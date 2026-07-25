//! Tracciati a record fissi e correlazione di lotto.
//!
//! È il modulo verticale di Setaccio. I file dei lotti di composizione
//! documentale (`Scaricati/T1Q73KFP/328102`) non hanno estensione, sono UTF-8
//! con record di lunghezza costante e nessun indexer generalista li apre.
//! L'obiettivo finale non è leggerli, è **correlarli**: dato un testo, dire in
//! quale riga del tracciato e in quale pagina del PDF generato si trova.
//!
//! Nota sui dati reali, perché ha guidato tutte le euristiche qui sotto: il
//! tracciato `328102` non ha *una* lunghezza di record, ne ha sedici. È un
//! flusso multi-tipo: ogni riga comincia con una chiave di record
//! (`ILCM0121…`, `ILCM0101…`) e ogni tipo ha la sua lunghezza. Perciò
//! "lunghezza costante" non è il criterio giusto per riconoscerlo — lo è
//! invece "poche lunghezze distinte su tante righe", che separa nettamente un
//! flusso a record fissi da un file di prosa senza estensione (README,
//! LICENSE) dove quasi ogni riga ha una lunghezza diversa.

use anyhow::{Context, Result};
use rusqlite::{params, OptionalExtension};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use crate::db::{riga_in_file, Db};
use crate::types::{Anteprima, Campo, FileRecord, Layout, LayoutCandidato, Lotto, Tipo};

// ---------------------------------------------------------------------------
// Tetti: nessuna funzione qui dentro deve poter leggere un file intero senza
// un limite, altrimenti un tracciato da qualche GB blocca la UI.
// ---------------------------------------------------------------------------

/// Byte letti dall'euristica di riconoscimento. 64 KB sono ~80 record da 807
/// caratteri: più che sufficienti per decidere, e costano una sola read.
const CAMPIONE_EURISTICA: usize = 64 * 1024;

/// Byte letti dall'auto-detect. Più generoso dell'euristica perché qui si
/// misurano le colonne stabili e servono record di tutti i tipi presenti.
const CAMPIONE_DETECT: usize = 4 * 1024 * 1024;

/// Record su cui si calcolano le colonne stabili. Oltre questa soglia il
/// risultato non cambia più: se una colonna è instabile lo si scopre nelle
/// prime migliaia di righe.
const RECORD_PER_ANALISI: usize = 5_000;

/// **Tetto ai record indicizzati per singolo file.** Un tracciato da milioni
/// di righe farebbe esplodere `record_tracciato` e la sua FTS (a ~800 caratteri
/// per record, 50.000 righe sono già ~40 MB di indice per un solo file). Oltre
/// la soglia il file resta cercabile per nome e leggibile in anteprima
/// direttamente dal disco: si perde solo la ricerca full-text sulle righe in
/// coda, che è il compromesso meno doloroso.
pub const MAX_RECORD_INDICIZZATI: usize = 50_000;

/// Record restituiti da `record()` quando il chiamante non specifica nulla.
const RECORD_PAGINA_DEFAULT: usize = 50;
/// Tetto per pagina: protegge il canale Tauri da payload assurdi.
const RECORD_PAGINA_MAX: usize = 1_000;

/// Estensioni che, oltre all'assenza di estensione, possono nascondere un
/// tracciato: sui lotti reali `.dat` e `.bol` sono flussi a record fissi in
/// latin-1, non file binari.
const ESTENSIONI_TRACCIATO: &[&str] = &["dat", "bol"];

// ---------------------------------------------------------------------------
// Lettura e decodifica
// ---------------------------------------------------------------------------

/// Decodifica un blocco di byte come testo. I tracciati reali sono in due
/// codifiche diverse (`328102` è UTF-8, `T1Q73KFP.dat` è latin-1): provare
/// prima UTF-8 e ripiegare su windows-1252 li copre entrambi senza mai
/// fallire, e windows-1252 non ha byte invalidi per costruzione.
fn decodifica(buf: &[u8]) -> String {
    match std::str::from_utf8(buf) {
        Ok(s) => s.to_string(),
        Err(e) if e.error_len().is_none() => {
            // Errore solo in coda: è il campione tagliato a metà di un
            // carattere multibyte, non un file non-UTF-8. Si scarta la coda.
            String::from_utf8_lossy(&buf[..e.valid_up_to()]).into_owned()
        }
        Err(_) => encoding_rs::WINDOWS_1252.decode(buf).0.into_owned(),
    }
}

/// Legge al massimo `massimo` byte. Ritorna il testo decodificato e se il file
/// è stato letto per intero (serve al detect per sapere se può fidarsi della
/// dimensione totale).
fn campione(path: &Path, massimo: usize) -> Result<(String, bool)> {
    let f = std::fs::File::open(path)
        .with_context(|| format!("impossibile aprire «{}»", path.display()))?;
    let mut buf = Vec::with_capacity(massimo.min(1 << 20));
    // `massimo + 1` per accorgersi che il file continua oltre il campione.
    let letti = f.take(massimo as u64 + 1).read_to_end(&mut buf)?;
    let intero = letti <= massimo;
    buf.truncate(massimo.min(letti));
    Ok((decodifica(&buf), intero))
}

/// Righe complete contenute nel campione. L'ultima riga viene scartata quando
/// il campione è troncato: sarebbe mozzata e falserebbe ogni misura di
/// lunghezza.
fn righe_complete(testo: &str, intero: bool) -> Vec<&str> {
    let mut righe: Vec<&str> = testo.split('\n').map(|r| r.trim_end_matches('\r')).collect();
    // `split` produce sempre un elemento finale: se il file finisce con \n è
    // vuoto e va tolto, se non finisce con \n ed è troncato è mozzato.
    match righe.last() {
        Some(u) if u.is_empty() => {
            righe.pop();
        }
        Some(_) if !intero => {
            righe.pop();
        }
        _ => {}
    }
    righe
}

// ---------------------------------------------------------------------------
// Euristica di riconoscimento
// ---------------------------------------------------------------------------

/// True se il file sembra un tracciato a record fissi: righe di lunghezza
/// costante, contenuto testuale, nessuna estensione riconosciuta.
pub fn sembra_tracciato(path: &Path) -> bool {
    // Primo filtro, gratis: l'estensione. Senza questo ogni .txt lungo del
    // disco finirebbe nell'analisi costosa.
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase());
    match ext.as_deref() {
        None => {}
        Some(e) if ESTENSIONI_TRACCIATO.contains(&e) => {}
        Some(_) => return false,
    }

    let Ok((testo, intero)) = campione(path, CAMPIONE_EURISTICA) else {
        return false;
    };
    if testo.is_empty() {
        return false;
    }
    // Un NUL o troppi caratteri di controllo: è un binario, non un tracciato.
    let mut controlli = 0usize;
    let mut totali = 0usize;
    for c in testo.chars() {
        totali += 1;
        if c == '\0' {
            return false;
        }
        if c.is_control() && c != '\n' && c != '\r' && c != '\t' {
            controlli += 1;
        }
    }
    if totali == 0 || (controlli * 100) / totali > 1 {
        return false;
    }

    let righe = righe_complete(&testo, intero);
    if righe.is_empty() {
        // Nessun a capo: può essere un blocco di record fissi concatenati.
        // Si accetta solo se la dimensione è divisibile per una lunghezza di
        // record plausibile, altrimenti è un file qualsiasi.
        let Ok(meta) = std::fs::metadata(path) else {
            return false;
        };
        return !divisori_plausibili(meta.len() as usize).is_empty();
    }
    if righe.len() < 4 {
        return false;
    }

    let lunghezze: Vec<usize> = righe.iter().map(|r| r.chars().count()).collect();
    let media = lunghezze.iter().sum::<usize>() as f64 / lunghezze.len() as f64;
    let mut distinte: Vec<usize> = lunghezze.clone();
    distinte.sort_unstable();
    distinte.dedup();

    // Il criterio vero: *poche forme diverse*. Un flusso a record fissi ha una
    // manciata di lunghezze anche quando è multi-tipo (`328102`: 16 lunghezze
    // su 143 righe = 0.11). Un file di prosa ne ha quasi una per riga.
    let varieta = distinte.len() as f64 / righe.len() as f64;
    media >= 16.0 && (distinte.len() <= 2 || varieta <= 0.35)
}

/// Lunghezze di record plausibili che dividono esattamente `dimensione`.
/// Serve solo ai file senza a capo, dove i record sono blocchi concatenati.
fn divisori_plausibili(dimensione: usize) -> Vec<usize> {
    if dimensione == 0 {
        return Vec::new();
    }
    let mut v = Vec::new();
    for l in 8..=8192usize {
        if l > dimensione {
            break;
        }
        if dimensione % l == 0 && dimensione / l >= 2 {
            v.push(l);
        }
    }
    v
}

// ---------------------------------------------------------------------------
// Auto-detect del layout
// ---------------------------------------------------------------------------

/// Genere di carattere di una colonna. Solo questi tre generi contano come
/// "stabile": una colonna sempre di punteggiatura è comoda ma non è un
/// confine di campo, è un separatore, e il chiamante non saprebbe che farsene.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Genere {
    Spazio,
    Cifra,
    Lettera,
    Altro,
}

fn genere(c: char) -> Genere {
    if c == ' ' {
        Genere::Spazio
    } else if c.is_ascii_digit() {
        Genere::Cifra
    } else if c.is_alphabetic() {
        Genere::Lettera
    } else {
        Genere::Altro
    }
}

/// Intervalli `[inizio, fine)` di colonne che su tutti i record passati
/// contengono sempre lo stesso genere di carattere, più il numero totale di
/// colonne stabili (che è quello che serve per la confidenza).
fn colonne_stabili(record: &[&str], lunghezza: usize) -> (Vec<(usize, usize)>, usize) {
    if lunghezza == 0 || record.is_empty() {
        return (Vec::new(), 0);
    }
    let mut visto: Vec<Option<Genere>> = vec![None; lunghezza];
    let mut instabile: Vec<bool> = vec![false; lunghezza];

    for r in record {
        for (i, c) in r.chars().enumerate() {
            if i >= lunghezza {
                break;
            }
            if instabile[i] {
                continue;
            }
            let g = genere(c);
            match visto[i] {
                None => visto[i] = Some(g),
                Some(p) if p == g => {}
                _ => instabile[i] = true,
            }
        }
    }

    let stabile = |i: usize| -> Option<Genere> {
        if instabile[i] {
            return None;
        }
        match visto[i] {
            Some(Genere::Altro) | None => None,
            Some(g) => Some(g),
        }
    };

    let mut intervalli: Vec<(usize, usize)> = Vec::new();
    let mut quante = 0usize;
    let mut i = 0usize;
    while i < lunghezza {
        match stabile(i) {
            None => i += 1,
            Some(g) => {
                let inizio = i;
                while i < lunghezza && stabile(i) == Some(g) {
                    i += 1;
                }
                quante += i - inizio;
                intervalli.push((inizio, i));
            }
        }
    }
    (intervalli, quante)
}

/// Auto-detect del layout: lunghezza di record dominante, numero di record e
/// intervalli di colonne che restano stabili su tutte le righe.
///
/// La confidenza pesa due cose diverse:
/// * quanto il file si concentra sulla lunghezza dominante (60%) — su un
///   flusso multi-tipo questa quota è bassa *per costruzione*, ed è giusto che
///   la confidenza lo dica;
/// * quanto quelle righe sono incolonnate (40%) — è il segnale che il layout
///   proposto ha davvero dei confini di campo dentro.
pub fn detect(path: &Path) -> Result<LayoutCandidato> {
    let (testo, intero) = campione(path, CAMPIONE_DETECT)?;
    let righe = righe_complete(&testo, intero);

    if righe.is_empty() {
        return detect_a_blocchi(path, &testo);
    }

    let mut conteggio: HashMap<usize, usize> = HashMap::new();
    for r in &righe {
        *conteggio.entry(r.chars().count()).or_insert(0) += 1;
    }
    // A parità di occorrenze vince la lunghezza maggiore: un record più lungo
    // porta più campi, ed è la scelta più utile da mostrare per prima.
    let (lunghezza_record, dominanti) = conteggio
        .iter()
        .max_by_key(|(l, n)| (**n, **l))
        .map(|(l, n)| (*l, *n))
        .unwrap_or((0, 0));

    let campione_record: Vec<&str> = righe
        .iter()
        .copied()
        .filter(|r| r.chars().count() == lunghezza_record)
        .take(RECORD_PER_ANALISI)
        .collect();
    let (intervalli, stabili) = colonne_stabili(&campione_record, lunghezza_record);

    let quota_dominante = dominanti as f64 / righe.len() as f64;
    let quota_stabile = if lunghezza_record > 0 {
        stabili as f64 / lunghezza_record as f64
    } else {
        0.0
    };
    // Con meno di due record non si è misurato niente: la confidenza è nulla,
    // non alta per caso.
    let confidenza = if campione_record.len() < 2 {
        0.0
    } else {
        (0.6 * quota_dominante + 0.4 * quota_stabile).clamp(0.0, 1.0)
    };

    Ok(LayoutCandidato {
        lunghezza_record,
        numero_record: righe.len(),
        colonne_stabili: intervalli,
        confidenza: (confidenza * 1000.0).round() / 1000.0,
    })
}

/// Variante per i file senza a capo: i record sono blocchi concatenati, quindi
/// la lunghezza va cercata fra i divisori esatti della dimensione totale e
/// scelta per quanto incolonna bene i dati.
fn detect_a_blocchi(path: &Path, testo: &str) -> Result<LayoutCandidato> {
    let dimensione = std::fs::metadata(path)?.len() as usize;
    let candidati = divisori_plausibili(dimensione);
    if candidati.is_empty() {
        return Ok(LayoutCandidato {
            lunghezza_record: testo.chars().count(),
            numero_record: if testo.is_empty() { 0 } else { 1 },
            colonne_stabili: Vec::new(),
            confidenza: 0.0,
        });
    }

    let caratteri: Vec<char> = testo.chars().collect();
    let mut migliore = (candidati[0], 0.0f64, Vec::new(), 0usize);
    for l in candidati {
        let quanti = (caratteri.len() / l).min(RECORD_PER_ANALISI);
        if quanti < 2 {
            continue;
        }
        let pezzi: Vec<String> = (0..quanti)
            .map(|i| caratteri[i * l..(i + 1) * l].iter().collect())
            .collect();
        let rif: Vec<&str> = pezzi.iter().map(|s| s.as_str()).collect();
        let (intervalli, stabili) = colonne_stabili(&rif, l);
        let punteggio = stabili as f64 / l as f64;
        if punteggio > migliore.1 {
            migliore = (l, punteggio, intervalli, stabili);
        }
    }

    Ok(LayoutCandidato {
        lunghezza_record: migliore.0,
        numero_record: dimensione / migliore.0.max(1),
        colonne_stabili: migliore.2,
        // Senza a capo l'unica prova che la lunghezza sia quella giusta è
        // l'incolonnamento: la confidenza è tutta lì, e si tiene sotto 0.9
        // perché un divisore può sempre essere una coincidenza.
        confidenza: ((migliore.1 * 0.9) * 1000.0).round() / 1000.0,
    })
}

// ---------------------------------------------------------------------------
// Codice di lotto
// ---------------------------------------------------------------------------

/// True se il nome è un codice di lotto in forma `T` + alfanumerici maiuscoli
/// (`T1Q73KFP`). Si pretende almeno una cifra per non promuovere a lotto ogni
/// cartella che comincia per T (`Test`, `Tesi`, `TODO`).
fn codice_t(nome: &str) -> bool {
    let b = nome.as_bytes();
    if b.len() < 6 || b.len() > 12 || b[0] != b'T' {
        return false;
    }
    nome.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        && nome.chars().any(|c| c.is_ascii_digit())
}

/// Estrae il codice da un singolo segmento di path, tagliando il suffisso dopo
/// il primo `_`: copre sia `T1Q73KFP_PDF` (cartella) sia `T1Q73KFP_0000001`
/// (PDF generato).
fn codice_da_segmento(nome: &str) -> Option<String> {
    if codice_t(nome) {
        return Some(nome.to_string());
    }
    let testa = nome.split('_').next().unwrap_or("");
    if codice_t(testa) {
        return Some(testa.to_string());
    }
    None
}

/// Quante cartelle si risalgono in cerca del codice. Oltre questa profondità
/// un match sarebbe quasi certamente casuale.
const RISALITA_MAX: usize = 4;

/// Ricava il codice di lotto da un path (`.../T1Q73KFP/328102` → `T1Q73KFP`).
pub fn codice_lotto(path: &Path) -> Option<String> {
    // Prima il nome del file, ma solo per la forma `T…`: il nome del tracciato
    // dentro un lotto è tutto cifre (`328102`) e NON è il codice del lotto —
    // il codice è la cartella che lo contiene.
    if let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) {
        if let Some(c) = codice_da_segmento(&stem) {
            return Some(c);
        }
    }

    // Poi le cartelle, dalla più vicina alla più lontana.
    let mut risalite = 0usize;
    let mut cur = path.parent();
    while let Some(dir) = cur {
        let nome = dir.file_name().map(|s| s.to_string_lossy().into_owned());
        if let Some(nome) = nome {
            if let Some(c) = codice_da_segmento(&nome) {
                return Some(c);
            }
            // Le cartelle-lotto numeriche (`319312`): tutte cifre e abbastanza
            // lunghe da non confondersi con un `2024` di archiviazione.
            if nome.len() >= 5 && nome.chars().all(|c| c.is_ascii_digit()) {
                return Some(nome);
            }
        }
        risalite += 1;
        if risalite >= RISALITA_MAX {
            break;
        }
        cur = dir.parent();
    }
    None
}

// ---------------------------------------------------------------------------
// Correlazione di lotto — il valore vero del modulo
// ---------------------------------------------------------------------------

fn e_pdf(f: &FileRecord) -> bool {
    f.ext
        .as_deref()
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

fn e_tracciato(f: &FileRecord) -> bool {
    if f.tipo == Tipo::Tracciato {
        return true;
    }
    match f.ext.as_deref() {
        // Dentro una cartella-lotto un file senza estensione è il tracciato:
        // è così che è fatto `T1Q73KFP/328102`.
        None => true,
        Some(e) => ESTENSIONI_TRACCIATO.contains(&e.to_ascii_lowercase().as_str()),
    }
}

/// Cartella rappresentativa del lotto. Un lotto reale si sparpaglia su più
/// cartelle (`T1Q73KFP/` e `T1Q73KFP_PDF/`): l'antenato comune sarebbe
/// `Scaricati/`, inutile. Vince la cartella del tracciato, che è l'origine di
/// tutto; se non c'è, la più popolata.
fn cartella_del_lotto(file: &[FileRecord]) -> String {
    let padre = |p: &str| {
        Path::new(p)
            .parent()
            .map(|d| d.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    if let Some(t) = file.iter().find(|f| e_tracciato(f)) {
        return padre(&t.path);
    }
    let mut conteggio: HashMap<String, usize> = HashMap::new();
    for f in file {
        *conteggio.entry(padre(&f.path)).or_insert(0) += 1;
    }
    conteggio
        .into_iter()
        // A parità di conteggio vince il path più corto: sta più in alto ed è
        // il contenitore più probabile.
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.len().cmp(&a.0.len())))
        .map(|(k, _)| k)
        .unwrap_or_default()
}

fn in_lotto(codice: String, file: Vec<FileRecord>) -> Lotto {
    let cartella = cartella_del_lotto(&file);
    let mut tracciati = Vec::new();
    let mut pdf = Vec::new();
    let mut altri = Vec::new();
    for f in file {
        // L'ordine conta: un PDF non è mai un tracciato, anche se il tipo
        // nell'indice fosse sbagliato.
        if e_pdf(&f) {
            pdf.push(f);
        } else if e_tracciato(&f) {
            tracciati.push(f);
        } else {
            altri.push(f);
        }
    }
    Lotto {
        codice,
        cartella,
        tracciati,
        pdf,
        altri,
    }
}

/// Legge dall'indice i file di uno o tutti i lotti, già ordinati per codice.
fn file_dei_lotti(db: &Db, codice: Option<&str>) -> Result<Vec<(String, FileRecord)>> {
    let conn = db.conn();
    let sql = match codice {
        Some(_) => "SELECT * FROM file WHERE lotto = ?1 ORDER BY path",
        None => "SELECT * FROM file WHERE lotto IS NOT NULL AND lotto <> '' ORDER BY lotto, path",
    };
    let mut st = conn.prepare(sql)?;
    let mappa = |r: &rusqlite::Row| -> rusqlite::Result<(String, FileRecord)> {
        let f = riga_in_file(r)?;
        Ok((f.lotto.clone().unwrap_or_default(), f))
    };
    let righe: Vec<(String, FileRecord)> = match codice {
        Some(c) => st
            .query_map(params![c], mappa)?
            .collect::<rusqlite::Result<Vec<_>>>()?,
        None => st.query_map([], mappa)?.collect::<rusqlite::Result<Vec<_>>>()?,
    };
    Ok(righe)
}

/// Tutti i lotti presenti nell'indice.
pub fn lotti(db: &Db) -> Result<Vec<Lotto>> {
    let righe = file_dei_lotti(db, None)?;
    // Raggruppamento in ordine di arrivo: la query ordina per codice, quindi
    // basta accumulare finché il codice non cambia — niente HashMap, l'ordine
    // alfabetico arriva gratis alla UI.
    let mut out: Vec<Lotto> = Vec::new();
    let mut corrente: Option<(String, Vec<FileRecord>)> = None;
    for (codice, f) in righe {
        match corrente.as_mut() {
            Some((c, v)) if *c == codice => v.push(f),
            _ => {
                if let Some((c, v)) = corrente.take() {
                    out.push(in_lotto(c, v));
                }
                corrente = Some((codice, vec![f]));
            }
        }
    }
    if let Some((c, v)) = corrente.take() {
        out.push(in_lotto(c, v));
    }
    Ok(out)
}

/// Un lotto con tracciati, PDF generati e file correlati.
pub fn lotto(db: &Db, codice: &str) -> Result<Option<Lotto>> {
    let righe = file_dei_lotti(db, Some(codice))?;
    if righe.is_empty() {
        return Ok(None);
    }
    let file: Vec<FileRecord> = righe.into_iter().map(|(_, f)| f).collect();
    Ok(Some(in_lotto(codice.to_string(), file)))
}

// ---------------------------------------------------------------------------
// Layout: CRUD
// ---------------------------------------------------------------------------

pub fn layout_lista(db: &Db) -> Result<Vec<Layout>> {
    let conn = db.conn();
    let mut st = conn.prepare("SELECT id, nome, lunghezza_record, campi FROM layout ORDER BY nome")?;
    let righe = st
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut out = Vec::with_capacity(righe.len());
    for (id, nome, lung, campi) in righe {
        out.push(Layout {
            id,
            nome,
            lunghezza_record: lung.max(0) as usize,
            // Un layout con JSON corrotto non deve far sparire tutta la lista:
            // si degrada a zero campi e resta modificabile dalla UI.
            campi: serde_json::from_str(&campi).unwrap_or_default(),
        });
    }
    Ok(out)
}

pub fn layout_salva(db: &Db, nome: &str, lunghezza_record: usize, campi: &[Campo]) -> Result<i64> {
    let json = serde_json::to_string(campi)?;
    let conn = db.conn();
    // `nome` è UNIQUE: salvare due volte lo stesso nome è un aggiornamento,
    // non un errore da mostrare all'utente.
    conn.execute(
        "INSERT INTO layout (nome, lunghezza_record, campi) VALUES (?1, ?2, ?3)
         ON CONFLICT(nome) DO UPDATE SET lunghezza_record = excluded.lunghezza_record,
                                         campi = excluded.campi",
        params![nome, lunghezza_record as i64, json],
    )?;
    let id: i64 = conn.query_row("SELECT id FROM layout WHERE nome = ?1", params![nome], |r| {
        r.get(0)
    })?;
    Ok(id)
}

pub fn layout_elimina(db: &Db, id: i64) -> Result<()> {
    let conn = db.conn();
    conn.execute("DELETE FROM layout WHERE id = ?1", params![id])?;
    Ok(())
}

fn layout_per_id(db: &Db, id: i64) -> Result<Option<Layout>> {
    let conn = db.conn();
    let r = conn
        .query_row(
            "SELECT id, nome, lunghezza_record, campi FROM layout WHERE id = ?1",
            params![id],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?;
    Ok(r.map(|(id, nome, lung, campi)| Layout {
        id,
        nome,
        lunghezza_record: lung.max(0) as usize,
        campi: serde_json::from_str(&campi).unwrap_or_default(),
    }))
}

// ---------------------------------------------------------------------------
// Lettura dei record
// ---------------------------------------------------------------------------

/// Spezza una riga secondo i campi del layout.
///
/// **Gli offset sono in CARATTERI, non in byte.** Sui tracciati reali compaiono
/// indirizzi con accenti (`VIA S. NICOLÒ`): tagliare su `&str[a..b]` darebbe
/// campi sfalsati dopo il primo carattere multibyte, o un panic in mezzo a una
/// sequenza UTF-8.
pub fn spezza(riga: &str, campi: &[Campo]) -> Vec<String> {
    let caratteri: Vec<char> = riga.chars().collect();
    campi
        .iter()
        .map(|c| {
            let da = c.offset.min(caratteri.len());
            let a = c.offset.saturating_add(c.lunghezza).min(caratteri.len());
            // `trim_end`: il riempimento a destra è padding del formato, non
            // dato. Gli spazi a sinistra invece restano, perché su un campo
            // numerico allineato a destra dicono qualcosa.
            caratteri[da..a]
                .iter()
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

/// Layout implicito a colonna unica, usato quando il chiamante non ne indica
/// nessuno: mostra la riga cruda invece di rifiutarsi di mostrare qualcosa.
fn layout_intero(lunghezza: usize) -> Vec<Campo> {
    vec![Campo {
        nome: "record".into(),
        offset: 0,
        lunghezza: lunghezza.max(1),
        tipo: "testo".into(),
    }]
}

/// Righe di un file saltandone `salta` e prendendone al massimo `quante`.
/// Legge a blocchi: un tracciato grosso non viene mai caricato tutto.
fn righe_da_file(path: &Path, salta: usize, quante: usize) -> Result<Vec<String>> {
    let f = std::fs::File::open(path)
        .with_context(|| format!("impossibile aprire «{}»", path.display()))?;
    let mut lettore = BufReader::new(f);
    let mut out = Vec::new();
    let mut buf: Vec<u8> = Vec::new();
    let mut n = 0usize;
    loop {
        buf.clear();
        if lettore.read_until(b'\n', &mut buf)? == 0 {
            break;
        }
        if n >= salta {
            while matches!(buf.last(), Some(b'\n') | Some(b'\r')) {
                buf.pop();
            }
            out.push(decodifica(&buf));
            if out.len() >= quante {
                break;
            }
        }
        n += 1;
    }
    Ok(out)
}

/// Applica un layout a un tracciato e restituisce i record già spezzati nei
/// campi, pronti per la tabella dell'anteprima.
pub fn record(
    db: &Db,
    file_id: i64,
    layout_id: Option<i64>,
    da: usize,
    quanti: usize,
) -> Result<Anteprima> {
    let quanti = if quanti == 0 {
        RECORD_PAGINA_DEFAULT
    } else {
        quanti.min(RECORD_PAGINA_MAX)
    };

    let Some(file) = db.file_per_id(file_id)? else {
        return Ok(Anteprima {
            genere: "nessuna".into(),
            testo: None,
            record: None,
            intestazioni: None,
            pagine: None,
            pagina: None,
            messaggio: Some(format!("file {file_id} non presente nell'indice")),
        });
    };

    // Prima l'indice: è più veloce e funziona anche se il file nel frattempo è
    // stato spostato. Il disco è il ripiego.
    let (righe, totale, fonte) = {
        let conn = db.conn();
        let totale: i64 = conn.query_row(
            "SELECT COUNT(*) FROM record_tracciato WHERE file_id = ?1",
            params![file_id],
            |r| r.get(0),
        )?;
        if totale > 0 {
            let mut st = conn.prepare(
                "SELECT contenuto FROM record_tracciato WHERE file_id = ?1
                 ORDER BY numero_riga LIMIT ?2 OFFSET ?3",
            )?;
            let v: Vec<String> = st
                .query_map(params![file_id, quanti as i64, da as i64], |r| r.get(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            (v, totale as usize, "indice")
        } else {
            (Vec::new(), 0usize, "disco")
        }
    };

    let (righe, totale) = if fonte == "indice" {
        (righe, totale)
    } else {
        let p = Path::new(&file.path);
        if !p.exists() {
            return Ok(Anteprima {
                genere: "nessuna".into(),
                testo: None,
                record: None,
                intestazioni: None,
                pagine: None,
                pagina: None,
                messaggio: Some(format!(
                    "«{}» non è più sul disco e non è indicizzato per record",
                    file.path
                )),
            });
        }
        let v = righe_da_file(p, da, quanti)?;
        // Totale ignoto senza scorrere tutto il file: si dichiara almeno
        // quanto si è visto, meglio che mentire con un conteggio finto.
        let t = da + v.len();
        (v, t)
    };

    let layout = match layout_id {
        Some(id) => layout_per_id(db, id)?,
        None => None,
    };
    let (campi, nome_layout) = match layout {
        Some(l) if !l.campi.is_empty() => (l.campi, l.nome),
        _ => {
            let larghezza = righe.iter().map(|r| r.chars().count()).max().unwrap_or(1);
            (layout_intero(larghezza), "colonna unica".to_string())
        }
    };

    let intestazioni: Vec<String> = campi.iter().map(|c| c.nome.clone()).collect();
    let spezzate: Vec<Vec<String>> = righe.iter().map(|r| spezza(r, &campi)).collect();
    let fine = da + spezzate.len();

    Ok(Anteprima {
        genere: "record".into(),
        testo: None,
        record: Some(spezzate),
        intestazioni: Some(intestazioni),
        pagine: None,
        pagina: None,
        messaggio: Some(format!(
            "record {}-{} di {} — layout «{}», letti da {}",
            if fine == 0 { 0 } else { da + 1 },
            fine,
            totale,
            nome_layout,
            fonte
        )),
    })
}

/// Indicizza le righe di un tracciato per la ricerca per campo.
///
/// Ritorna quante righe sono finite nell'indice; se il file ne ha di più si
/// ferma a [`MAX_RECORD_INDICIZZATI`].
pub fn indicizza_record(db: &Db, file_id: i64, path: &Path) -> Result<usize> {
    let f = std::fs::File::open(path)
        .with_context(|| format!("impossibile aprire «{}»", path.display()))?;
    let mut lettore = BufReader::new(f);

    let mut conn = db.conn();
    let tx = conn.transaction()?;
    // Reindicizzare deve essere idempotente: prima si toglie il vecchio, sia
    // dalla tabella sia dalla FTS (che non ha foreign key e non si pulisce da
    // sola).
    tx.execute(
        "DELETE FROM record_fts WHERE rowid IN (SELECT id FROM record_tracciato WHERE file_id = ?1)",
        params![file_id],
    )?;
    tx.execute(
        "DELETE FROM record_tracciato WHERE file_id = ?1",
        params![file_id],
    )?;

    let mut buf: Vec<u8> = Vec::new();
    let mut n = 0usize;
    {
        let mut ins = tx.prepare(
            "INSERT INTO record_tracciato (file_id, numero_riga, contenuto) VALUES (?1, ?2, ?3)",
        )?;
        let mut ins_fts = tx.prepare("INSERT INTO record_fts (rowid, contenuto) VALUES (?1, ?2)")?;
        while n < MAX_RECORD_INDICIZZATI {
            buf.clear();
            if lettore.read_until(b'\n', &mut buf)? == 0 {
                break;
            }
            while matches!(buf.last(), Some(b'\n') | Some(b'\r')) {
                buf.pop();
            }
            let contenuto = decodifica(&buf);
            // Le righe vuote non aiutano nessuna ricerca ma spostano la
            // numerazione: si indicizzano comunque, il numero di riga deve
            // corrispondere a quello che l'utente vede in un editor.
            ins.execute(params![file_id, (n + 1) as i64, contenuto])?;
            let id = tx.last_insert_rowid();
            ins_fts.execute(params![id, contenuto])?;
            n += 1;
        }
    }
    tx.commit()?;
    Ok(n)
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use super::*;
    use crate::db::FileVisto;
    use crate::types::Fascia;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static CONTATORE: AtomicUsize = AtomicUsize::new(0);

    /// Cartella temporanea senza dipendenze esterne (`tempfile` non è fra le
    /// dipendenze del progetto e non è lecito aggiungerla).
    fn temp(nome: &str) -> PathBuf {
        let n = CONTATORE.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!(
            "setaccio-tracciati-{}-{}-{}",
            std::process::id(),
            nome,
            n
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn db_con_file(path: &Path) -> (Db, i64) {
        let db = Db::in_memoria().unwrap();
        let sid = db
            .sorgente_aggiungi(
                path.parent().unwrap().to_str().unwrap(),
                Fascia::Tracciati,
            )
            .unwrap();
        let (id, _) = db
            .file_upsert(&FileVisto {
                path: path.to_string_lossy().into_owned(),
                nome: path.file_name().unwrap().to_string_lossy().into_owned(),
                ext: None,
                size: std::fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0),
                mtime: 1,
                tipo: Tipo::Tracciato,
                motivo_tipo: "test".into(),
                contesto: None,
                motivo_contesto: None,
                sorgente_id: sid,
                archivio_padre: None,
                lotto: Some("T1Q73KFP".into()),
            })
            .unwrap();
        (db, id)
    }

    /// Tre record da 30 caratteri: chiave (6) + nome (14) + importo (10).
    fn tracciato_finto() -> String {
        let mut s = String::new();
        for (i, nome) in ["ROSSI MARIO  ", "BIANCHI LUCIA", "VERDI ANNA   "]
            .iter()
            .enumerate()
        {
            s.push_str(&format!("REC{:03} {}0000{:06}\n", i + 1, nome, (i + 1) * 100));
        }
        s
    }

    fn campi_finti() -> Vec<Campo> {
        vec![
            Campo { nome: "chiave".into(), offset: 0, lunghezza: 6, tipo: "testo".into() },
            Campo { nome: "nome".into(), offset: 7, lunghezza: 13, tipo: "testo".into() },
            Campo { nome: "importo".into(), offset: 20, lunghezza: 10, tipo: "numero".into() },
        ]
    }

    #[test]
    fn detect_su_record_regolari() {
        let dir = temp("detect");
        let p = dir.join("328999");
        std::fs::write(&p, tracciato_finto()).unwrap();

        let c = detect(&p).unwrap();
        assert_eq!(c.lunghezza_record, 30);
        assert_eq!(c.numero_record, 3);
        // Tutte le righe hanno la lunghezza dominante: la quota vale 1.0 e la
        // confidenza non può scendere sotto il peso di quel termine.
        assert!(c.confidenza >= 0.6, "confidenza troppo bassa: {}", c.confidenza);
        // La colonna 6 è sempre spazio: è un confine di campo, e deve comparire
        // come intervallo a sé.
        assert!(
            c.colonne_stabili.iter().any(|(a, b)| *a == 6 && *b == 7),
            "colonne rilevate: {:?}",
            c.colonne_stabili
        );
        // Le prime tre colonne sono sempre lettere (REC).
        assert!(c.colonne_stabili.iter().any(|(a, b)| *a == 0 && *b == 3));
    }

    #[test]
    fn sembra_tracciato_distingue_flusso_da_prosa() {
        let dir = temp("euristica");
        let flusso = dir.join("328999");
        std::fs::write(&flusso, tracciato_finto().repeat(4)).unwrap();
        assert!(sembra_tracciato(&flusso));

        // Prosa senza estensione: lunghezze tutte diverse.
        let prosa = dir.join("LICENSE");
        std::fs::write(
            &prosa,
            "Questa e una licenza di prova.\nLa seconda riga e piu lunga della prima riga.\n\
             Terza.\nUna quarta riga di lunghezza ancora differente dalle altre.\n\
             E una quinta, diversa pure lei, per non lasciare dubbi al riguardo.\n",
        )
        .unwrap();
        assert!(!sembra_tracciato(&prosa));

        // Estensione non da tracciato: si esce subito.
        let pdf = dir.join("x.pdf");
        std::fs::write(&pdf, tracciato_finto()).unwrap();
        assert!(!sembra_tracciato(&pdf));

        // Binario travestito da file senza estensione.
        let bin = dir.join("blob");
        std::fs::write(&bin, [0u8, 1, 2, 3, 0, 9, 9, 9]).unwrap();
        assert!(!sembra_tracciato(&bin));
    }

    #[test]
    fn codice_lotto_dai_path_reali() {
        assert_eq!(
            codice_lotto(Path::new("/home/u/Scaricati/T1Q73KFP/328102")).as_deref(),
            Some("T1Q73KFP")
        );
        assert_eq!(
            codice_lotto(Path::new("/home/u/Scaricati/T1Q73KFP_PDF/cust_data (1).xml")).as_deref(),
            Some("T1Q73KFP")
        );
        assert_eq!(
            codice_lotto(Path::new("/home/u/Scaricati/T1Q73KFP_0000001.pdf")).as_deref(),
            Some("T1Q73KFP")
        );
        assert_eq!(
            codice_lotto(Path::new("/home/u/Scaricati/T1Q73KFP/T1Q73KFP.bol")).as_deref(),
            Some("T1Q73KFP")
        );
        // Cartella-lotto tutta cifre.
        assert_eq!(
            codice_lotto(Path::new("/home/u/Scaricati/319312/000000001.pdf")).as_deref(),
            Some("319312")
        );
        // Nessun lotto: e giusto che non inventi niente.
        assert_eq!(codice_lotto(Path::new("/home/u/Documenti/CV_2026.pdf")), None);
        assert_eq!(codice_lotto(Path::new("/home/u/Test/appunti.txt")), None);
    }

    #[test]
    fn record_spezzati_secondo_il_layout() {
        let dir = temp("record");
        let p = dir.join("328999");
        std::fs::write(&p, tracciato_finto()).unwrap();
        let (db, file_id) = db_con_file(&p);

        let quanti = indicizza_record(&db, file_id, &p).unwrap();
        assert_eq!(quanti, 3);

        let lid = layout_salva(&db, "prova", 30, &campi_finti()).unwrap();
        let a = record(&db, file_id, Some(lid), 0, 10).unwrap();
        assert_eq!(a.genere, "record");
        assert_eq!(
            a.intestazioni.as_deref(),
            Some(&["chiave".to_string(), "nome".to_string(), "importo".to_string()][..])
        );
        let righe = a.record.unwrap();
        assert_eq!(righe.len(), 3);
        assert_eq!(righe[0], vec!["REC001", "ROSSI MARIO", "0000000100"]);
        assert_eq!(righe[1], vec!["REC002", "BIANCHI LUCIA", "0000000200"]);

        // Paginazione.
        let a2 = record(&db, file_id, Some(lid), 2, 10).unwrap();
        assert_eq!(a2.record.unwrap().len(), 1);

        // Senza layout: colonna unica con la riga intera.
        let a3 = record(&db, file_id, None, 0, 1).unwrap();
        assert_eq!(a3.intestazioni.as_deref(), Some(&["record".to_string()][..]));
        assert_eq!(a3.record.unwrap()[0][0], "REC001 ROSSI MARIO  0000000100");

        // Reindicizzare non duplica.
        assert_eq!(indicizza_record(&db, file_id, &p).unwrap(), 3);
        let a4 = record(&db, file_id, Some(lid), 0, 10).unwrap();
        assert_eq!(a4.record.unwrap().len(), 3);
    }

    #[test]
    fn taglio_corretto_con_caratteri_accentati() {
        // "NICOLÒ" ha un carattere da 2 byte: tagliando in byte il campo
        // successivo slitterebbe di uno.
        let riga = "REC001 VIA S. NICOLÒ 12  0000000100";
        let campi = vec![
            Campo { nome: "chiave".into(), offset: 0, lunghezza: 6, tipo: "testo".into() },
            Campo { nome: "indirizzo".into(), offset: 7, lunghezza: 18, tipo: "testo".into() },
            Campo { nome: "importo".into(), offset: 25, lunghezza: 10, tipo: "numero".into() },
        ];
        let v = spezza(riga, &campi);
        assert_eq!(v[0], "REC001");
        assert_eq!(v[1], "VIA S. NICOLÒ 12");
        assert_eq!(v[2], "0000000100");

        // Taglio che cade esattamente sul carattere accentato (colonna 19).
        let corto = vec![Campo {
            nome: "x".into(),
            offset: 19,
            lunghezza: 1,
            tipo: "testo".into(),
        }];
        assert_eq!(spezza(riga, &corto)[0], "Ò");

        // Campo oltre la fine della riga: si tronca, non si va in panic.
        let oltre = vec![Campo {
            nome: "y".into(),
            offset: 100,
            lunghezza: 10,
            tipo: "testo".into(),
        }];
        assert_eq!(spezza(riga, &oltre)[0], "");
    }

    #[test]
    fn tetto_ai_record_indicizzati() {
        // Non si costruisce un file da 50.000 righe nel test: si verifica che
        // la costante resti in un intervallo sensato. Valutata a compile time,
        // così una modifica sbagliata al tetto non arriva nemmeno a girare.
        const { assert!(MAX_RECORD_INDICIZZATI >= 1_000) };
        const { assert!(MAX_RECORD_INDICIZZATI <= 500_000) };
    }

    #[test]
    fn lotti_correlano_tracciato_e_pdf() {
        let db = Db::in_memoria().unwrap();
        let sid = db.sorgente_aggiungi("/s", Fascia::Documenti).unwrap();
        let agg = |path: &str, ext: Option<&str>, tipo: Tipo| {
            db.file_upsert(&FileVisto {
                path: path.into(),
                nome: Path::new(path)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                ext: ext.map(|e| e.to_string()),
                size: 1,
                mtime: 1,
                tipo,
                motivo_tipo: "test".into(),
                contesto: None,
                motivo_contesto: None,
                sorgente_id: sid,
                archivio_padre: None,
                lotto: codice_lotto(Path::new(path)),
            })
            .unwrap();
        };
        agg("/s/T1Q73KFP/328102", None, Tipo::Tracciato);
        agg("/s/T1Q73KFP/T1Q73KFP.xml", Some("xml"), Tipo::Documento);
        agg("/s/T1Q73KFP/T1Q73KFP_0000001.pdf", Some("pdf"), Tipo::Documento);
        agg("/s/T1Q73KFP_PDF/T1Q73KFP_0000002.pdf", Some("pdf"), Tipo::Documento);
        agg("/s/319312/319312.dat", Some("dat"), Tipo::Tracciato);
        agg("/s/319312/000000001.pdf", Some("pdf"), Tipo::Documento);

        let tutti = lotti(&db).unwrap();
        assert_eq!(tutti.len(), 2, "lotti: {:?}", tutti.iter().map(|l| &l.codice).collect::<Vec<_>>());

        let t = lotto(&db, "T1Q73KFP").unwrap().unwrap();
        assert_eq!(t.tracciati.len(), 1);
        // I PDF del lotto stanno in due cartelle diverse: vanno correlati lo
        // stesso, e' esattamente il punto del modulo.
        assert_eq!(t.pdf.len(), 2);
        assert_eq!(t.altri.len(), 1);
        assert_eq!(t.cartella, "/s/T1Q73KFP");

        let n = lotto(&db, "319312").unwrap().unwrap();
        assert_eq!(n.tracciati.len(), 1);
        assert_eq!(n.pdf.len(), 1);

        assert!(lotto(&db, "INESISTENTE").unwrap().is_none());
    }

    #[test]
    fn crud_layout() {
        let db = Db::in_memoria().unwrap();
        let id = layout_salva(&db, "ILC-MAIL", 807, &campi_finti()).unwrap();
        let lista = layout_lista(&db).unwrap();
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].lunghezza_record, 807);
        assert_eq!(lista[0].campi.len(), 3);
        assert_eq!(lista[0].campi[1].nome, "nome");

        // Stesso nome: aggiornamento, non duplicato.
        let id2 = layout_salva(&db, "ILC-MAIL", 768, &campi_finti()[..1]).unwrap();
        assert_eq!(id, id2);
        let lista = layout_lista(&db).unwrap();
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].lunghezza_record, 768);
        assert_eq!(lista[0].campi.len(), 1);

        layout_elimina(&db, id).unwrap();
        assert!(layout_lista(&db).unwrap().is_empty());
    }

    /// Verifica sui dati veri. Salta in silenzio dove il file non c'è, così la
    /// suite resta verde su un'altra macchina.
    #[test]
    fn detect_sul_tracciato_reale() {
        let p = Path::new("/home/russus/Scaricati/T1Q73KFP/328102");
        if !p.exists() {
            return;
        }
        assert!(sembra_tracciato(p), "328102 deve essere riconosciuto");
        let c = detect(p).unwrap();
        println!(
            "328102 → lunghezza_record={} numero_record={} intervalli_stabili={} confidenza={}\n  {:?}",
            c.lunghezza_record,
            c.numero_record,
            c.colonne_stabili.len(),
            c.confidenza,
            c.colonne_stabili
        );

        // Il .dat dello stesso lotto è latin-1, non UTF-8: deve essere
        // riconosciuto lo stesso, altrimenti la decodifica è sbagliata.
        let dat = Path::new("/home/russus/Scaricati/T1Q73KFP/T1Q73KFP.dat");
        if dat.exists() {
            assert!(sembra_tracciato(dat), "il .dat latin-1 va riconosciuto");
            let d = detect(dat).unwrap();
            println!(
                "T1Q73KFP.dat → lunghezza_record={} numero_record={} confidenza={}",
                d.lunghezza_record, d.numero_record, d.confidenza
            );
            assert!(d.confidenza > 0.5);
        }
        // 144 record: il file non finisce con un a capo, quindi `wc -l` ne
        // conta 143 — l'ultima riga esiste comunque ed è un record vero.
        assert_eq!(c.numero_record, 144);
        // Flusso multi-tipo: la lunghezza dominante è quella dei record
        // `ILCM0121…` (20 righe su 144), non l'unica presente.
        assert_eq!(c.lunghezza_record, 768);
        assert!(!c.colonne_stabili.is_empty());
        assert_eq!(codice_lotto(p).as_deref(), Some("T1Q73KFP"));
    }
}
