//! Walker e orchestrazione della scansione.
//!
//! Il pezzo che dà valore a Setaccio sta qui: `dentro_repo()`. Sul filesystem
//! reale dell'autore, 2.951 documenti su 3.341 vivono dentro alberi di
//! progetti di codice e sono fixture o output di build. Le regole per nome di
//! cartella ne intercettano 33 su 2.951 — l'1%. Risalire gli antenati in cerca
//! di un marker di repository è l'unico criterio che funziona.

use anyhow::Result;
use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use walkdir::{DirEntry, WalkDir};

use crate::db::{Db, FileVisto};
use crate::types::{Fascia, Progresso, Regola, Sorgente, Tipo};

/// Marker che identificano la radice di un progetto di codice.
pub const MARKER_REPO: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "Cargo.toml",
    "package.json",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "pubspec.yaml",
    "go.mod",
    "pyproject.toml",
    "composer.json",
    "Gemfile",
];

/// Estensioni che, trovate nella cartella, la qualificano come progetto .NET.
pub const MARKER_REPO_EXT: &[&str] = &["csproj", "sln", "fsproj", "vbproj"];

/// Cartelle mai attraversate: pesano molto e non contengono documenti
/// dell'utente.
pub const CARTELLE_SALTATE: &[&str] = &[
    "node_modules",
    "target",
    "build",
    "dist",
    ".git",
    ".cache",
    ".venv",
    "venv",
    "__pycache__",
    ".gradle",
    ".dart_tool",
    ".nuget",
    ".cargo",
    "vendor",
    "Pods",
    ".next",
    ".svelte-kit",
];

/// La UI ridisegna a ogni evento: oltre una decina al secondo il thread di
/// scansione passerebbe più tempo a notificare che a leggere il disco.
const INTERVALLO_EVENTI: Duration = Duration::from_millis(100);

/// Quanti file al massimo si indicizzano dall'interno di un singolo archivio.
/// Serve a non far esplodere l'indice su uno zip di sorgenti da 40.000 voci.
const LIMITE_VOCI_ARCHIVIO: usize = 500;

/// Stato condiviso fra il thread di scansione e la UI.
pub struct Controllo {
    ferma: AtomicBool,
    stato: Mutex<Progresso>,
}

impl Controllo {
    pub fn nuovo() -> Self {
        Controllo {
            ferma: AtomicBool::new(false),
            stato: Mutex::new(Progresso::default()),
        }
    }

    pub fn in_corso(&self) -> bool {
        self.stato.lock().map(|s| s.in_corso).unwrap_or(false)
    }

    pub fn ferma(&self) {
        self.ferma.store(true, Ordering::Relaxed);
    }

    pub fn da_fermare(&self) -> bool {
        self.ferma.load(Ordering::Relaxed)
    }

    pub fn azzera(&self) {
        self.ferma.store(false, Ordering::Relaxed);
        if let Ok(mut s) = self.stato.lock() {
            *s = Progresso::default();
        }
    }

    pub fn progresso(&self) -> Progresso {
        self.stato.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn aggiorna<F: FnOnce(&mut Progresso)>(&self, f: F) -> Progresso {
        let mut s = self.stato.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut s);
        s.clone()
    }
}

/// Risale gli antenati di `path` (fermandosi a `limite` incluso) cercando un
/// marker di repository. Ritorna la radice del repo se ne trova uno.
///
/// Vince il repository **più esterno**: dentro `phoenix_old/` c'è un `.git` e
/// dentro `phoenix_old/PhoenixTest/` un `.csproj`, ma all'utente interessa
/// sapere che il file sta in `phoenix_old`, non in quale sottoprogetto.
pub fn dentro_repo(path: &Path, limite: Option<&Path>) -> Option<PathBuf> {
    let partenza = if path.is_dir() {
        path
    } else {
        path.parent()?
    };

    let mut catena: Vec<&Path> = Vec::new();
    let mut corrente = Some(partenza);
    while let Some(dir) = corrente {
        if let Some(l) = limite {
            if !dir.starts_with(l) {
                break;
            }
        }
        catena.push(dir);
        corrente = dir.parent();
    }

    catena
        .into_iter()
        .rev()
        .find(|d| ha_marker_repo(d))
        .map(|d| d.to_path_buf())
}

/// True se la cartella contiene uno dei marker di repository.
fn ha_marker_repo(dir: &Path) -> bool {
    if MARKER_REPO.iter().any(|m| dir.join(m).exists()) {
        return true;
    }
    // I progetti .NET non hanno un marker a nome fisso: si riconoscono solo
    // dall'estensione di un file qualsiasi dentro la cartella, e questo
    // costringe a una `read_dir` — per questo la si tenta solo come ripiego.
    let Ok(voci) = std::fs::read_dir(dir) else {
        return false;
    };
    voci.flatten().any(|v| {
        Path::new(&v.file_name())
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| {
                let e = e.to_ascii_lowercase();
                MARKER_REPO_EXT.contains(&e.as_str())
            })
            .unwrap_or(false)
    })
}

/// Memoizza l'esito per cartella: senza, risalire gli antenati dei 17.401 file
/// di `Scaricati` costerebbe una `stat` per marker per livello, per ogni file.
#[derive(Default)]
struct CacheRepo {
    visto: HashMap<PathBuf, Option<PathBuf>>,
}

impl CacheRepo {
    /// Radice del repository che contiene `dir`, cercata verso l'alto fino a
    /// `limite`. La ricorsione è su cartelle già viste quasi sempre, perché il
    /// walker scende in ordine e il padre è stato risolto un attimo prima.
    fn radice(&mut self, dir: &Path, limite: &Path) -> Option<PathBuf> {
        if !dir.starts_with(limite) {
            return None;
        }
        if let Some(esito) = self.visto.get(dir) {
            return esito.clone();
        }
        let da_sopra = match dir.parent() {
            Some(p) if p != dir => self.radice(p, limite),
            _ => None,
        };
        let esito = da_sopra.or_else(|| {
            if ha_marker_repo(dir) {
                Some(dir.to_path_buf())
            } else {
                None
            }
        });
        self.visto.insert(dir.to_path_buf(), esito.clone());
        esito
    }
}

/// Emette gli eventi di avanzamento a ritmo sostenibile per la UI.
struct Emettitore<F> {
    callback: F,
    ultimo: Option<Instant>,
}

impl<F: Fn(Progresso)> Emettitore<F> {
    fn nuovo(callback: F) -> Self {
        Emettitore {
            callback,
            ultimo: None,
        }
    }

    fn emetti(&mut self, p: Progresso, forza: bool) {
        let scaduto = self
            .ultimo
            .map(|t| t.elapsed() >= INTERVALLO_EVENTI)
            .unwrap_or(true);
        if forza || scaduto {
            self.ultimo = Some(Instant::now());
            (self.callback)(p);
        }
    }
}

/// Esegue `f` isolando sia gli errori sia i panic: un PDF malformato o un
/// parser di terze parti che sbaglia un indice non devono fermare una
/// scansione da 17.000 file a metà strada.
fn prova<T>(f: impl FnOnce() -> Result<T>) -> Option<T> {
    match std::panic::catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(v)) => Some(v),
        _ => None,
    }
}

/// Come `prova`, per le funzioni che non ritornano `Result`.
fn prova_valore<T>(f: impl FnOnce() -> T) -> Option<T> {
    std::panic::catch_unwind(AssertUnwindSafe(f)).ok()
}

/// Zittisce il messaggio di panic per la durata della scansione, ripristinando
/// il gestore precedente all'uscita.
///
/// Serve perché `pdf-extract` va in panic su PDF malformati: su una passata
/// reale in `Scaricati` sono stati 80 messaggi riversati su stderr. I panic
/// vengono già catturati e contati fra gli errori, ma il testo grezzo
/// (`panicked at .../lib.rs:286: deref`) non deve arrivare all'utente.
struct SilenzioPanic {
    precedente: Option<Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + Sync + Send + 'static>>,
}

impl SilenzioPanic {
    fn attiva() -> Self {
        let precedente = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        SilenzioPanic {
            precedente: Some(precedente),
        }
    }
}

impl Drop for SilenzioPanic {
    fn drop(&mut self) {
        if let Some(h) = self.precedente.take() {
            std::panic::set_hook(h);
        }
    }
}

/// Esegue una scansione completa di tutte le sorgenti attive.
pub fn esegui<F>(db: Arc<Db>, controllo: Arc<Controllo>, avanzamento: F) -> Result<()>
where
    F: Fn(Progresso) + Send + 'static,
{
    controllo.azzera();
    let mut em = Emettitore::nuovo(avanzamento);
    let p = controllo.aggiorna(|p| {
        p.in_corso = true;
        p.fase = "avvio".into();
    });
    em.emetti(p, true);

    let esito = {
        let _silenzio = SilenzioPanic::attiva();
        passata(&db, &controllo, &mut em)
    };

    // Qualunque cosa sia successa la UI deve ritrovare uno stato coerente,
    // altrimenti il pulsante "Scansiona" resta disabilitato per sempre.
    let fermata = controllo.da_fermare();
    let fallita = esito.is_err();
    let p = controllo.aggiorna(|p| {
        p.in_corso = false;
        p.finito = true;
        p.path_corrente.clear();
        p.fase = if fallita {
            "errore".into()
        } else if fermata {
            "fermata".into()
        } else {
            "finita".into()
        };
    });
    em.emetti(p, true);
    esito
}

fn passata<F: Fn(Progresso)>(
    db: &Db,
    controllo: &Controllo,
    em: &mut Emettitore<F>,
) -> Result<()> {
    let regole = db.regole()?;
    let sorgenti = db.sorgenti()?;

    for s in sorgenti.iter().filter(|s| s.attiva) {
        if controllo.da_fermare() {
            break;
        }
        let radice = PathBuf::from(&s.path);
        if !radice.is_dir() {
            // Una sorgente sparita (disco esterno staccato) non è un errore
            // fatale: si salta e si va avanti con le altre.
            continue;
        }
        let p = controllo.aggiorna(|p| {
            p.fase = format!("scansione di {}", s.path);
        });
        em.emetti(p, true);

        scansiona_sorgente(db, controllo, em, s, &radice, &regole);

        if controllo.da_fermare() {
            // Potare dopo un'interruzione cancellerebbe dall'indice i file che
            // esistono ancora ma che non abbiamo fatto in tempo a rivedere.
            break;
        }
        let p = controllo.aggiorna(|p| {
            p.fase = format!("potatura di {}", s.path);
        });
        em.emetti(p, true);
        if db.pota_spariti(s.id).is_err() {
            controllo.aggiorna(|p| p.errori += 1);
        }
    }

    let adesso = epoch_ora();
    db.imposta("ultima_scansione", &adesso.to_string())?;
    Ok(())
}

fn scansiona_sorgente<F: Fn(Progresso)>(
    db: &Db,
    controllo: &Controllo,
    em: &mut Emettitore<F>,
    s: &Sorgente,
    radice: &Path,
    regole: &[Regola],
) {
    let mut cache = CacheRepo::default();
    let profondita = if s.ricorsiva { usize::MAX } else { 1 };
    let camminata = WalkDir::new(radice)
        .follow_links(false)
        .min_depth(1)
        .max_depth(profondita)
        .into_iter()
        .filter_entry(|e| !da_saltare(e));

    for voce in camminata {
        if controllo.da_fermare() {
            return;
        }
        let voce = match voce {
            Ok(v) => v,
            Err(_) => {
                // Permesso negato su una cartella: contabilizza e prosegui.
                controllo.aggiorna(|p| p.errori += 1);
                continue;
            }
        };
        if !voce.file_type().is_file() {
            continue;
        }

        let path = voce.path();
        let p = controllo.aggiorna(|p| {
            p.visti += 1;
            p.path_corrente = path.display().to_string();
        });
        em.emetti(p, false);

        if let Err(_e) = indicizza_file(db, controllo, s, radice, regole, &mut cache, &voce) {
            controllo.aggiorna(|p| p.errori += 1);
        }
    }
}

/// Vero per ciò che il walker non deve nemmeno attraversare.
fn da_saltare(e: &DirEntry) -> bool {
    // I symlink non si seguono: una cartella che punta a un suo antenato manda
    // il walker in un ciclo infinito, ed è uno scenario comune nelle home.
    if e.file_type().is_symlink() {
        return true;
    }
    if !e.file_type().is_dir() {
        return false;
    }
    e.file_name()
        .to_str()
        .map(|n| CARTELLE_SALTATE.contains(&n))
        .unwrap_or(false)
}

fn indicizza_file(
    db: &Db,
    controllo: &Controllo,
    s: &Sorgente,
    radice: &Path,
    regole: &[Regola],
    cache: &mut CacheRepo,
    voce: &DirEntry,
) -> Result<()> {
    let path = voce.path();
    let meta = voce.metadata()?;
    let size = meta.len() as i64;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // La guardia si disattiva per le sorgenti che *stanno* dentro un repo ma
    // contengono documenti veri.
    let radice_repo = if s.ignora_repo_guard {
        None
    } else {
        path.parent().and_then(|d| cache.radice(d, radice))
    };

    let esito = crate::classify::classifica(path, radice_repo.as_deref(), regole);
    let artefatto = esito.tipo == Tipo::Artefatto;
    let ext = estensione(path);

    // `sembra_tracciato` legge il file: la si interroga solo sui candidati
    // plausibili, cioè quelli senza estensione riconosciuta.
    // Quando la classificazione ha già detto «tracciato» ha appena letto il
    // file per stabilirlo: rileggerlo qui sarebbe solo I/O doppio. La sonda
    // serve solo per le sorgenti di fascia `tracciati`, dove si accetta di
    // pagare un assaggio in più pur di non perdere un flusso.
    let tracciato = !artefatto
        && (esito.tipo == Tipo::Tracciato
            || (s.fascia == Fascia::Tracciati
                && ext.is_none()
                && prova_valore(|| crate::tracciati::sembra_tracciato(path)).unwrap_or(false)));
    // Il codice di lotto si ricava per OGNI file, non solo per i tracciati:
    // la correlazione serve proprio a mettere insieme il tracciato di partenza
    // con i PDF che ne derivano e con l'XML di accompagnamento. Limitarla ai
    // tracciati la renderebbe un'etichetta senza contenuto — un lotto fatto di
    // un file solo non correla niente. Gli artefatti restano fuori: un PDF di
    // test dentro un repo non appartiene a nessun lotto.
    let lotto = if artefatto {
        None
    } else {
        prova_valore(|| crate::tracciati::codice_lotto(path)).flatten()
    };

    let visto = FileVisto {
        path: path.display().to_string(),
        nome: nome_file(path),
        ext: ext.clone(),
        size,
        mtime,
        tipo: if tracciato { Tipo::Tracciato } else { esito.tipo },
        motivo_tipo: esito.motivo_tipo,
        contesto: esito.contesto,
        motivo_contesto: esito.motivo_contesto,
        sorgente_id: s.id,
        archivio_padre: None,
        lotto,
    };
    let (id, cambiato) = db.file_upsert(&visto)?;
    controllo.aggiorna(|p| {
        p.indicizzati += 1;
        if artefatto {
            p.saltati_repo += 1;
        }
    });

    // Gli artefatti restano indicizzati per nome ma non si aprono mai: sono il
    // al 88% dei documenti trovati e riempirebbero l'FTS di rumore.
    if !cambiato || artefatto {
        return Ok(());
    }

    if tracciato {
        match prova(|| crate::tracciati::indicizza_record(db, id, path)) {
            Some(_) => controllo.aggiorna(|p| p.estratti += 1),
            None => controllo.aggiorna(|p| p.errori += 1),
        };
        return Ok(());
    }

    if prova_valore(|| crate::extract::e_archivio(ext.as_deref())).unwrap_or(false) {
        indicizza_archivio(db, controllo, s, regole, path, mtime);
        return Ok(());
    }

    // Media e installer non hanno testo: aprirli sarebbe solo I/O sprecato.
    if matches!(esito.tipo, Tipo::Media | Tipo::Installer) {
        return Ok(());
    }
    if !prova_valore(|| crate::extract::estraibile(ext.as_deref())).unwrap_or(true) {
        return Ok(());
    }
    match prova(|| crate::extract::estrai(path)) {
        Some(e) => {
            if db.testo_salva(id, &visto.nome, &e.testo, e.pagine).is_ok() {
                // Senza gli offset di pagina la ricerca sa dire *in quale file*
                // cade un match, ma non *in quale pagina*: metà del valore.
                let _ = db.offset_pagine_salva(id, &e.testo, &e.inizio_pagina);
                controllo.aggiorna(|p| p.estratti += 1);
            } else {
                controllo.aggiorna(|p| p.errori += 1);
            }
        }
        None => {
            controllo.aggiorna(|p| p.errori += 1);
        }
    }
    Ok(())
}

/// Indicizza il contenuto di un archivio senza estrarlo su disco: i file
/// interni esistono solo nell'indice, con un path virtuale.
fn indicizza_archivio(
    db: &Db,
    controllo: &Controllo,
    s: &Sorgente,
    regole: &[Regola],
    path: &Path,
    mtime: i64,
) {
    let Some(voci) = prova(|| crate::extract::dentro_archivio(path, LIMITE_VOCI_ARCHIVIO)) else {
        controllo.aggiorna(|p| p.errori += 1);
        return;
    };
    let padre = path.display().to_string();
    for v in voci {
        let virtuale = PathBuf::from(&v.path_virtuale);
        // Dentro un archivio la guardia repo non si applica: il contenitore è
        // già stato giudicato, e i suoi antenati non sono quelli della voce.
        let esito = crate::classify::classifica(&virtuale, None, regole);
        let interno = FileVisto {
            path: v.path_virtuale.clone(),
            nome: v.nome.clone(),
            ext: estensione(&virtuale),
            size: v.size,
            mtime,
            tipo: esito.tipo,
            motivo_tipo: esito.motivo_tipo,
            contesto: esito.contesto,
            motivo_contesto: esito.motivo_contesto,
            sorgente_id: s.id,
            archivio_padre: Some(padre.clone()),
            lotto: None,
        };
        let Ok((id, _)) = db.file_upsert(&interno) else {
            controllo.aggiorna(|p| p.errori += 1);
            continue;
        };
        controllo.aggiorna(|p| p.indicizzati += 1);
        if let Some(e) = v.estratto {
            if db.testo_salva(id, &v.nome, &e.testo, e.pagine).is_ok() {
                let _ = db.offset_pagine_salva(id, &e.testo, &e.inizio_pagina);
                controllo.aggiorna(|p| p.estratti += 1);
            } else {
                controllo.aggiorna(|p| p.errori += 1);
            }
        }
    }
}

fn estensione(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
}

fn nome_file(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn epoch_ora() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Sorgenti proposte alla prima apertura: `(path, fascia)`.
///
/// Sono le cartelle emerse dall'analisi del filesystem reale; quelle che non
/// esistono non si propongono, per non far partire l'utente con una lista di
/// errori.
pub fn sorgenti_suggerite() -> Vec<(String, String)> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let candidate: &[(&str, Fascia)] = &[
        ("Scaricati", Fascia::Documenti),
        ("Documenti/russus_doc", Fascia::Documenti),
        ("Documenti/libri", Fascia::Documenti),
        ("Documenti/kindle", Fascia::Documenti),
        ("Documenti/RED", Fascia::Documenti),
        ("Documenti/DevOps", Fascia::Documenti),
        ("Documenti/PAM", Fascia::Documenti),
        ("Scaricati/T1Q73KFP", Fascia::Tracciati),
        ("Scaricati/T1Q73KFP_PDF", Fascia::Tracciati),
    ];
    candidate
        .iter()
        .map(|(rel, fascia)| (home.join(rel), *fascia))
        .filter(|(p, _)| p.is_dir())
        .map(|(p, fascia)| (p.display().to_string(), fascia.as_str().to_string()))
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::atomic::AtomicU32;

    static CONTATORE: AtomicU32 = AtomicU32::new(0);

    /// Cartella temporanea usa e getta. Niente crate esterni: `tempfile` non è
    /// fra le dipendenze e il contratto non si tocca.
    struct Temporanea(PathBuf);

    impl Temporanea {
        fn nuova() -> Self {
            let n = CONTATORE.fetch_add(1, Ordering::Relaxed);
            let p = std::env::temp_dir().join(format!(
                "setaccio-test-{}-{}",
                std::process::id(),
                n
            ));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            Temporanea(p)
        }

        fn cartella(&self, rel: &str) -> PathBuf {
            let p = self.0.join(rel);
            std::fs::create_dir_all(&p).unwrap();
            p
        }

        fn file(&self, rel: &str, contenuto: &str) -> PathBuf {
            let p = self.0.join(rel);
            if let Some(d) = p.parent() {
                std::fs::create_dir_all(d).unwrap();
            }
            std::fs::write(&p, contenuto).unwrap();
            p
        }
    }

    impl Drop for Temporanea {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn trova_il_repo_fra_gli_antenati() {
        let t = Temporanea::nuova();
        t.cartella("progetto/.git");
        let doc = t.file("progetto/PhoenixTest/TestOutput/report.pdf", "x");

        let trovato = dentro_repo(&doc, Some(&t.0)).unwrap();
        assert_eq!(trovato, t.0.join("progetto"));
    }

    #[test]
    fn fuori_da_un_repo_non_trova_niente() {
        let t = Temporanea::nuova();
        let doc = t.file("documenti/fattura.pdf", "x");
        assert!(dentro_repo(&doc, Some(&t.0)).is_none());
    }

    #[test]
    fn il_progetto_dotnet_si_riconosce_dall_estensione() {
        let t = Temporanea::nuova();
        t.file("Phoenix/Phoenix.sln", "");
        let doc = t.file("Phoenix/out/a.pdf", "x");
        assert_eq!(dentro_repo(&doc, Some(&t.0)).unwrap(), t.0.join("Phoenix"));
    }

    #[test]
    fn vince_il_repo_piu_esterno() {
        let t = Temporanea::nuova();
        t.cartella("esterno/.git");
        t.file("esterno/interno/Cargo.toml", "");
        let doc = t.file("esterno/interno/src/note.pdf", "x");
        assert_eq!(dentro_repo(&doc, Some(&t.0)).unwrap(), t.0.join("esterno"));
    }

    #[test]
    fn il_limite_ferma_la_risalita() {
        let t = Temporanea::nuova();
        t.cartella("progetto/.git");
        let doc = t.file("progetto/docs/manuale.pdf", "x");
        let limite = t.0.join("progetto/docs");
        // Il marker sta sopra il limite: da lì in su non si guarda.
        assert!(dentro_repo(&doc, Some(&limite)).is_none());
    }

    #[test]
    fn la_cache_da_lo_stesso_esito_della_risalita_diretta() {
        let t = Temporanea::nuova();
        t.cartella("progetto/.git");
        let dentro = t.file("progetto/a/b/x.pdf", "x");
        let fuori = t.file("altro/y.pdf", "x");

        let mut cache = CacheRepo::default();
        assert_eq!(
            cache.radice(dentro.parent().unwrap(), &t.0),
            dentro_repo(&dentro, Some(&t.0))
        );
        assert_eq!(cache.radice(fuori.parent().unwrap(), &t.0), None);
        // Seconda passata: stessa risposta arrivando dalla memoizzazione.
        assert_eq!(
            cache.radice(dentro.parent().unwrap(), &t.0),
            Some(t.0.join("progetto"))
        );
    }

    #[test]
    fn le_sorgenti_suggerite_esistono_tutte() {
        for (p, fascia) in sorgenti_suggerite() {
            assert!(Path::new(&p).is_dir(), "{p} proposta ma inesistente");
            assert!(matches!(fascia.as_str(), "documenti" | "tracciati"));
        }
    }

    /// Scansione end-to-end su un albero fatto di soli artefatti: verifica
    /// walker, guardia repo, upsert e potatura senza toccare `extract` e
    /// `tracciati`, che sono ancora stub di altri moduli.
    #[test]
    fn scansione_di_un_albero_di_soli_artefatti() {
        let t = Temporanea::nuova();
        t.cartella("progetto/.git");
        t.file("progetto/TestOutput/a.pdf", "x");
        t.file("progetto/TestOutput/b.docx", "x");
        t.cartella("progetto/node_modules/pacchetto");
        t.file("progetto/node_modules/pacchetto/c.pdf", "x");

        let db = Arc::new(Db::in_memoria().unwrap());
        let id = db
            .sorgente_aggiungi(&t.0.display().to_string(), Fascia::Documenti)
            .unwrap();
        let controllo = Arc::new(Controllo::nuovo());
        let eventi = Arc::new(AtomicU32::new(0));
        let contatore = eventi.clone();

        esegui(db.clone(), controllo.clone(), move |_| {
            contatore.fetch_add(1, Ordering::Relaxed);
        })
        .unwrap();

        let p = controllo.progresso();
        assert!(!p.in_corso && p.finito);
        assert_eq!(p.visti, 2, "node_modules non va attraversata");
        assert_eq!(p.indicizzati, 2);
        assert_eq!(p.saltati_repo, 2, "tutto sta dentro un repo");
        assert_eq!(p.errori, 0);
        assert!(eventi.load(Ordering::Relaxed) >= 2, "primo e ultimo evento");

        let rec = db
            .file_per_path(&t.0.join("progetto/TestOutput/a.pdf").display().to_string())
            .unwrap()
            .unwrap();
        assert_eq!(rec.tipo, Tipo::Artefatto);
        assert_eq!(rec.sorgente_id, id);
        assert!(rec
            .motivo_tipo
            .unwrap()
            .contains(&t.0.join("progetto").display().to_string()));

        // Seconda passata dopo aver cancellato un file: la potatura lo toglie.
        std::fs::remove_file(t.0.join("progetto/TestOutput/b.docx")).unwrap();
        esegui(db.clone(), controllo.clone(), |_| {}).unwrap();
        assert_eq!(controllo.progresso().visti, 1);
        assert!(db
            .file_per_path(&t.0.join("progetto/TestOutput/b.docx").display().to_string())
            .unwrap()
            .is_none());
    }

    #[test]
    fn la_sorgente_non_ricorsiva_resta_al_primo_livello() {
        let t = Temporanea::nuova();
        t.cartella(".git"); // tutto artefatto: niente estrazione negli stub
        t.file("radice.pdf", "x");
        t.file("sotto/annidato.pdf", "x");

        let db = Arc::new(Db::in_memoria().unwrap());
        let id = db
            .sorgente_aggiungi(&t.0.display().to_string(), Fascia::Documenti)
            .unwrap();
        let mut s = db.sorgenti().unwrap().pop().unwrap();
        s.ricorsiva = false;
        db.sorgente_aggiorna(&s).unwrap();
        assert_eq!(s.id, id);

        let controllo = Arc::new(Controllo::nuovo());
        esegui(db.clone(), controllo.clone(), |_| {}).unwrap();
        assert_eq!(controllo.progresso().visti, 1);
    }
}
