//! Categorizzazione su tre assi ortogonali: Tipo, Contesto, Stato.
//!
//! Regole prima, contenuto dopo. E ogni classificazione porta con sé il
//! motivo: senza il "perché è finito qui" l'utente non si fida del risultato e
//! smette di usare lo strumento.

use anyhow::Result;
use globset::{Glob, GlobMatcher};
use rusqlite::params;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

use crate::db::{riga_in_file, Db};
use crate::types::{DaRevisionare, Regola, Tipo};

/// Esito della classificazione di un singolo file.
#[derive(Debug, Clone)]
pub struct Esito {
    pub tipo: Tipo,
    pub motivo_tipo: String,
    pub contesto: Option<String>,
    pub motivo_contesto: Option<String>,
}

const EST_DOCUMENTO: &[&str] = &[
    "pdf", "docx", "doc", "xlsx", "xls", "pptx", "odt", "ods", "epub", "rtf", "md", "txt", "csv",
    "tsv",
];
const EST_ARCHIVIO: &[&str] = &["zip", "tar", "gz", "7z", "rar", "zst", "xz", "bz2"];
const EST_MEDIA: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "svg", "mp3", "mp4", "mkv", "wav", "avif",
];
const EST_INSTALLER: &[&str] = &[
    "exe", "msi", "deb", "rpm", "appimage", "apk", "ipa", "dmg", "pkg",
];

/// Estensioni che possono nascondere un flusso a record fissi. La stringa
/// vuota rappresenta l'assenza di estensione, che è il caso più comune nei
/// lotti reali (`Scaricati/T1Q73KFP/328102`).
const PROBABILI_TRACCIATI: &[&str] = &["", "dat", "bol"];

/// Decide Tipo e Contesto di un file. `radice_repo` è valorizzato quando il
/// walker ha trovato un marker di repository fra gli antenati: in quel caso il
/// tipo è `artefatto` e il motivo cita la radice trovata.
pub fn classifica(path: &Path, radice_repo: Option<&Path>, regole: &[Regola]) -> Esito {
    let nome = nome_file(path);
    let (contesto, motivo_contesto) = match regola_vincente(path, &nome, regole, "contesto") {
        Some(r) => (Some(r.valore.clone()), Some(motivo_regola(r))),
        None => (None, None),
    };

    // La guardia repo batte qualsiasi altra considerazione: un PDF dentro
    // `TestOutput/` resta una fixture di test anche se si chiama `fattura.pdf`.
    if let Some(radice) = radice_repo {
        return Esito {
            tipo: Tipo::Artefatto,
            motivo_tipo: format!("dentro il repository {}", radice.display()),
            contesto,
            motivo_contesto,
        };
    }

    let (tipo, motivo_tipo) = match regola_vincente(path, &nome, regole, "tipo") {
        Some(r) => (Tipo::from_str(&r.valore), motivo_regola(r)),
        None => tipo_dal_file(path),
    };

    Esito {
        tipo,
        motivo_tipo,
        contesto,
        motivo_contesto,
    }
}

fn motivo_regola(r: &Regola) -> String {
    format!("regola «{}» ({})", r.nome, r.pattern)
}

fn nome_file(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn estensione(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
}

fn tipo_per_estensione(ext: &str) -> Option<Tipo> {
    if EST_DOCUMENTO.contains(&ext) {
        Some(Tipo::Documento)
    } else if EST_ARCHIVIO.contains(&ext) {
        Some(Tipo::Archivio)
    } else if EST_MEDIA.contains(&ext) {
        Some(Tipo::Media)
    } else if EST_INSTALLER.contains(&ext) {
        Some(Tipo::Installer)
    } else {
        None
    }
}

/// Tipo ricavato dal filesystem: estensione prima, contenuto dopo.
fn tipo_dal_file(path: &Path) -> (Tipo, String) {
    let ext = estensione(path);
    if let Some(e) = &ext {
        if let Some(t) = tipo_per_estensione(e) {
            return (t, format!("estensione .{e}"));
        }
    }

    // I magic bytes costano una `open` più una lettura: si pagano solo quando
    // l'estensione non ha detto niente, che è il caso raro.
    if let Ok(Some(k)) = infer::get_from_path(path) {
        if let Some(t) = tipo_per_estensione(k.extension()) {
            return (t, format!("magic bytes {}", k.mime_type()));
        }
    }

    // Il riconoscimento dei tracciati sta tutto in `tracciati::sembra_tracciato`
    // e non va duplicato qui. Averlo fatto è costato caro: questa funzione
    // usava «tutte le righe della stessa lunghezza», mentre il tracciato reale
    // `328102` è un flusso multi-tipo con 16 lunghezze distinte su 144 righe.
    // Il risultato era che su una sorgente di fascia `documenti` la
    // classificazione scartava i tracciati prima ancora che il modulo dedicato
    // potesse vederli: un censimento su 17.414 file ne trovava zero.
    if PROBABILI_TRACCIATI.contains(&ext.as_deref().unwrap_or_default())
        && crate::tracciati::sembra_tracciato(path)
    {
        return (
            Tipo::Tracciato,
            match &ext {
                Some(e) => format!("flusso a record fissi (.{e})"),
                None => "nessuna estensione, flusso a record fissi".into(),
            },
        );
    }

    match ext {
        Some(e) => (Tipo::Altro, format!("estensione .{e} non riconosciuta")),
        None => (Tipo::Altro, "nessuna estensione né firma nota".into()),
    }
}

// ---------------------------------------------------------------------------
// Regole
// ---------------------------------------------------------------------------

thread_local! {
    /// Compilare un glob costa più del match stesso: con 3.000 file e una
    /// dozzina di regole senza cache si ricompilerebbe 36.000 volte.
    static GLOB: RefCell<HashMap<String, Option<GlobMatcher>>> = RefCell::new(HashMap::new());
}

fn matcher(pattern: &str) -> Option<GlobMatcher> {
    GLOB.with(|c| {
        let mut c = c.borrow_mut();
        if let Some(m) = c.get(pattern) {
            return m.clone();
        }
        // Un pattern sbagliato scritto dall'utente non deve far saltare la
        // scansione: la regola semplicemente non matcha mai.
        let m = Glob::new(pattern).ok().map(|g| g.compile_matcher());
        c.insert(pattern.to_string(), m.clone());
        m
    })
}

/// Un pattern che nomina cartelle (`**/T1Q*`, `**/libri/**`) si confronta col
/// path intero; uno senza separatori (`CV_*`) col solo nome del file.
fn combacia(r: &Regola, path: &Path, nome: &str) -> bool {
    let Some(m) = matcher(&r.pattern) else {
        return false;
    };
    if r.pattern.contains('/') || r.pattern.contains("**") {
        m.is_match(path)
    } else {
        m.is_match(nome)
    }
}

/// La regola attiva di priorità più bassa che matcha, sull'asse richiesto. A
/// parità di priorità vince quella incontrata per prima.
fn regola_vincente<'a>(
    path: &Path,
    nome: &str,
    regole: &'a [Regola],
    asse: &str,
) -> Option<&'a Regola> {
    let mut migliore: Option<&Regola> = None;
    for r in regole {
        if !r.attiva || r.asse != asse {
            continue;
        }
        if migliore.map(|m| r.priorita >= m.priorita).unwrap_or(false) {
            continue;
        }
        if combacia(r, path, nome) {
            migliore = Some(r);
        }
    }
    migliore
}

/// Riapplica le regole a tutto l'indice, senza rileggere i file dal disco.
///
/// Tocca solo il contesto: il tipo dipende dal filesystem (estensione, magic
/// bytes, presenza di un repo fra gli antenati) e reinventarlo qui darebbe un
/// risultato diverso da quello della scansione.
pub fn riapplica(db: &Db) -> Result<()> {
    let regole = db.regole()?;
    let conn = db.conn();
    // Una transazione sola: 3.000 commit separati metterebbero un minuto.
    let tx = conn.unchecked_transaction()?;

    let righe: Vec<(i64, String, String)> = {
        let mut st = tx.prepare("SELECT id, path, nome FROM file")?;
        let v = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        v
    };

    {
        let mut up = tx.prepare("UPDATE file SET contesto = ?2, motivo_contesto = ?3 WHERE id = ?1")?;
        for (id, path, nome) in righe {
            let p = Path::new(&path);
            match regola_vincente(p, &nome, &regole, "contesto") {
                Some(r) => up.execute(params![id, r.valore, motivo_regola(r)])?,
                None => up.execute(params![id, None::<String>, None::<String>])?,
            };
        }
    }

    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Coda di revisione
// ---------------------------------------------------------------------------

const COLONNE: &str = "id, path, nome, ext, size, mtime, hash, tipo, contesto, stato,
     motivo_tipo, motivo_contesto, sorgente_id, archivio_padre, lotto, testo_estratto, pagine";

/// I file che nessuna regola ha saputo collocare, con i pattern proposti.
///
/// L'ordine è per dimensione decrescente: un'ora spesa a scrivere regole rende
/// di più se si parte dai file che occupano davvero spazio.
pub fn coda_revisione(db: &Db, limite: i64) -> Result<Vec<DaRevisionare>> {
    let conn = db.conn();
    let sql = format!(
        "SELECT {COLONNE} FROM file
          WHERE contesto IS NULL AND tipo <> 'artefatto'
          ORDER BY size DESC LIMIT ?1"
    );
    let file = {
        let mut st = conn.prepare(&sql)?;
        let v = st
            .query_map(params![limite], riga_in_file)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        v
    };

    let mut st_vicini = conn.prepare(
        "SELECT contesto, COUNT(*) AS quanti FROM file
          WHERE contesto IS NOT NULL
            AND substr(path, 1, ?1) = ?2
            AND instr(substr(path, ?1 + 1), '/') = 0
          GROUP BY contesto ORDER BY quanti DESC LIMIT 5",
    )?;

    let mut out = Vec::with_capacity(file.len());
    for f in file {
        let p = Path::new(&f.path);
        let vicini = match p.parent() {
            Some(dir) => {
                let prefisso = format!("{}/", dir.display());
                let n = prefisso.chars().count() as i64;
                st_vicini
                    .query_map(params![n, prefisso], |r| r.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?
            }
            None => Vec::new(),
        };
        out.push(DaRevisionare {
            pattern_suggeriti: pattern_suggeriti(p),
            contesti_vicini: vicini,
            file: f,
        });
    }
    Ok(out)
}

/// Pattern proposti per un file non classificato, dal più specifico al più
/// generale: il nome esatto, la famiglia di nomi, la cartella.
fn pattern_suggeriti(path: &Path) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    let nome = nome_file(path);
    if !nome.is_empty() {
        v.push(nome.clone());
        if let Some(p) = prefisso_alfabetico(&nome) {
            v.push(format!("{p}*"));
        }
    }
    if let Some(cartella) = path
        .parent()
        .and_then(|d| d.file_name())
        .and_then(|n| n.to_str())
    {
        v.push(format!("**/{cartella}/**"));
    }
    v.dedup();
    v
}

/// Prefisso di lettere iniziale, col separatore che lo segue se c'è:
/// `CV_Rossi.pdf` → `CV_`, così il pattern proposto è `CV_*` e non `CV*`.
fn prefisso_alfabetico(nome: &str) -> Option<String> {
    let lettere: String = nome.chars().take_while(|c| c.is_alphabetic()).collect();
    if lettere.chars().count() < 2 || lettere.chars().count() == nome.chars().count() {
        return None;
    }
    let resto = &nome[lettere.len()..];
    let sep = resto.chars().next().filter(|c| *c == '_' || *c == '-');
    Some(match sep {
        Some(c) => format!("{lettere}{c}"),
        None => lettere,
    })
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::db::FileVisto;
    use crate::types::Fascia;

    fn regola(nome: &str, asse: &str, pattern: &str, valore: &str, priorita: i64) -> Regola {
        Regola {
            id: 0,
            nome: nome.into(),
            asse: asse.into(),
            pattern: pattern.into(),
            valore: valore.into(),
            priorita,
            attiva: true,
            builtin: false,
        }
    }

    #[test]
    fn tipo_dalle_estensioni() {
        let r: Vec<Regola> = vec![];
        let casi = [
            ("/x/a.pdf", Tipo::Documento),
            ("/x/a.EPUB", Tipo::Documento),
            ("/x/a.zip", Tipo::Archivio),
            ("/x/a.jpg", Tipo::Media),
            ("/x/a.AppImage", Tipo::Installer),
            ("/x/a.qwerty", Tipo::Altro),
        ];
        for (p, atteso) in casi {
            let e = classifica(Path::new(p), None, &r);
            assert_eq!(e.tipo, atteso, "{p}");
            assert!(!e.motivo_tipo.is_empty());
        }
    }

    #[test]
    fn artefatto_batte_tutto_il_resto() {
        let r = vec![regola("Curriculum", "contesto", "CV_*", "personale", 20)];
        let p = Path::new("/home/u/RiderProjects/phoenix_old/Test/CV_Rossi.pdf");
        let e = classifica(p, Some(Path::new("/home/u/RiderProjects/phoenix_old")), &r);
        assert_eq!(e.tipo, Tipo::Artefatto);
        assert_eq!(
            e.motivo_tipo,
            "dentro il repository /home/u/RiderProjects/phoenix_old"
        );
        // Il contesto continua a valere: serve a ritrovarlo se lo si cerca.
        assert_eq!(e.contesto.as_deref(), Some("personale"));
    }

    #[test]
    fn glob_sul_nome_e_glob_sul_path() {
        let r = vec![
            regola("Curriculum", "contesto", "CV_*", "personale", 20),
            regola("Lotti ILC", "contesto", "**/T1Q*", "lavoro-ILC", 10),
            regola("Libri", "contesto", "**/libri/**", "studio", 30),
        ];

        // Pattern senza separatori: si guarda solo il nome del file.
        let e = classifica(Path::new("/home/u/Scaricati/CV_Rossi.pdf"), None, &r);
        assert_eq!(e.contesto.as_deref(), Some("personale"));
        assert_eq!(
            e.motivo_contesto.as_deref(),
            Some("regola «Curriculum» (CV_*)")
        );

        // Lo stesso pattern non deve pescare una cartella che si chiama così.
        let e = classifica(Path::new("/home/u/CV_vecchi/nota.pdf"), None, &r);
        assert_eq!(e.contesto, None);

        // Pattern con separatori: si guarda il path intero.
        let e = classifica(Path::new("/home/u/Scaricati/T1Q73KFP/328102"), None, &r);
        assert_eq!(e.contesto.as_deref(), Some("lavoro-ILC"));

        let e = classifica(Path::new("/home/u/Documenti/libri/rust.epub"), None, &r);
        assert_eq!(e.contesto.as_deref(), Some("studio"));
    }

    #[test]
    fn vince_la_priorita_piu_bassa_anche_se_dichiarata_dopo() {
        let r = vec![
            regola("Generica", "contesto", "**/Scaricati/**", "scaricati", 90),
            regola("Lotti ILC", "contesto", "**/T1Q*", "lavoro-ILC", 10),
        ];
        let e = classifica(Path::new("/home/u/Scaricati/T1Q73KFP/328102"), None, &r);
        assert_eq!(e.contesto.as_deref(), Some("lavoro-ILC"));
    }

    #[test]
    fn le_regole_disattivate_non_contano() {
        let mut r = vec![regola("Curriculum", "contesto", "CV_*", "personale", 20)];
        r[0].attiva = false;
        let e = classifica(Path::new("/x/CV_Rossi.pdf"), None, &r);
        assert_eq!(e.contesto, None);
    }

    #[test]
    fn una_regola_di_tipo_sovrascrive_lestensione() {
        let r = vec![regola("Tracciati ILC", "tipo", "**/T1Q*", "tracciato", 5)];
        let e = classifica(Path::new("/home/u/Scaricati/T1Q73KFP/328102.txt"), None, &r);
        assert_eq!(e.tipo, Tipo::Tracciato);
    }

    #[test]
    fn pattern_proposti_dal_piu_specifico_al_piu_generale() {
        let v = pattern_suggeriti(Path::new("/home/u/Scaricati/BDOC_2024_policy.pdf"));
        assert_eq!(
            v,
            vec![
                "BDOC_2024_policy.pdf".to_string(),
                "BDOC_*".to_string(),
                "**/Scaricati/**".to_string(),
            ]
        );

        // Nome tutto lettere: il prefisso coinciderebbe col nome, si omette.
        let v = pattern_suggeriti(Path::new("/home/u/note/appunti"));
        assert_eq!(v, vec!["appunti".to_string(), "**/note/**".to_string()]);

        // Nome che inizia con una cifra: nessuna famiglia da proporre.
        let v = pattern_suggeriti(Path::new("/home/u/note/328102.pdf"));
        assert_eq!(v, vec!["328102.pdf".to_string(), "**/note/**".to_string()]);
    }

    fn inserisci(db: &Db, sorgente: i64, path: &str, tipo: Tipo, contesto: Option<&str>) -> i64 {
        let nome = path.rsplit('/').next().unwrap().to_string();
        let f = FileVisto {
            path: path.into(),
            nome,
            ext: None,
            size: 100,
            mtime: 1,
            tipo,
            motivo_tipo: "test".into(),
            contesto: contesto.map(|c| c.to_string()),
            motivo_contesto: None,
            sorgente_id: sorgente,
            archivio_padre: None,
            lotto: None,
        };
        db.file_upsert(&f).unwrap().0
    }

    #[test]
    fn riapplica_aggiorna_solo_il_contesto() {
        let db = Db::in_memoria().unwrap();
        let s = db.sorgente_aggiungi("/home/u", Fascia::Documenti).unwrap();
        let id = inserisci(&db, s, "/home/u/Documenti/libri/rust.epub", Tipo::Documento, None);

        riapplica(&db).unwrap();
        let r = db.file_per_id(id).unwrap().unwrap();
        assert_eq!(r.contesto.as_deref(), Some("studio"), "regola builtin Libri");
        assert_eq!(r.tipo, Tipo::Documento, "il tipo non si tocca");

        // Spenta la regola, il contesto deve tornare vuoto.
        let libri = db
            .regole()
            .unwrap()
            .into_iter()
            .find(|x| x.pattern == "**/libri/**")
            .unwrap();
        db.regola_attiva(libri.id, false).unwrap();
        riapplica(&db).unwrap();
        assert_eq!(db.file_per_id(id).unwrap().unwrap().contesto, None);
    }

    #[test]
    fn la_coda_ignora_gli_artefatti_e_ordina_per_dimensione() {
        let db = Db::in_memoria().unwrap();
        let s = db.sorgente_aggiungi("/home/u", Fascia::Documenti).unwrap();
        inserisci(&db, s, "/home/u/vari/piccolo.pdf", Tipo::Documento, None);
        inserisci(&db, s, "/home/u/vari/vicino.pdf", Tipo::Documento, Some("studio"));
        inserisci(&db, s, "/home/u/repo/fixture.pdf", Tipo::Artefatto, None);
        // Un file grande, per verificare l'ordinamento.
        {
            let conn = db.conn();
            conn.execute(
                "UPDATE file SET size = 9000 WHERE path = '/home/u/vari/piccolo.pdf'",
                [],
            )
            .unwrap();
        }

        let coda = coda_revisione(&db, 10).unwrap();
        assert_eq!(coda.len(), 1, "artefatti e già classificati restano fuori");
        let r = &coda[0];
        assert_eq!(r.file.path, "/home/u/vari/piccolo.pdf");
        assert_eq!(r.contesti_vicini, vec!["studio".to_string()]);
        assert!(r.pattern_suggeriti.contains(&"**/vari/**".to_string()));
    }
}
