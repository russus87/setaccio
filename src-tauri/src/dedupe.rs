//! Duplicati esatti (stesso hash) e near-duplicate (stesso testo, byte
//! diversi — il caso `accessibile.pdf` / `corretto.pdf`).

use anyhow::{Context, Result};
use rayon::prelude::*;
use regex::Regex;
use std::collections::{BTreeMap, HashSet};
use std::io::Read;
use std::path::Path;
use std::sync::OnceLock;

use crate::db::{riga_in_file, Db};
use crate::types::{FileRecord, GruppoDuplicati};

/// Le stesse colonne che `riga_in_file` si aspetta di trovare nella riga.
const COLONNE_FILE: &str = "id, path, nome, ext, size, mtime, hash, tipo, contesto, stato,
     motivo_tipo, motivo_contesto, sorgente_id, archivio_padre, lotto, testo_estratto, pagine";

/// Blocco di lettura per l'hashing. 256 KB tiene occupato il disco senza
/// costare RAM per thread quando rayon ne apre uno per core.
const BLOCCO: usize = 256 * 1024;

/// Sotto questa soglia di testo normalizzato il confronto per testo non è
/// affidabile: due PDF quasi vuoti (una copertina, un frontespizio) finirebbero
/// nello stesso gruppo pur non c'entrando niente.
const MINIMO_TESTO_CONFRONTABILE: usize = 64;

// ---------------------------------------------------------------------------
// Hash
// ---------------------------------------------------------------------------

/// Hash del contenuto di un file.
pub fn hash_file(path: &Path) -> Result<String> {
    let f = std::fs::File::open(path)
        .with_context(|| format!("apertura di {}", path.display()))?;
    let mut lettore = std::io::BufReader::with_capacity(BLOCCO, f);
    let mut hasher = blake3::Hasher::new();
    // A blocchi e mai `read_to_end`: negli archivi dell'utente ci sono zip da
    // centinaia di MB, caricarli in RAM per hasharli non ha senso.
    let mut buf = vec![0u8; BLOCCO];
    loop {
        let letti = lettore
            .read(&mut buf)
            .with_context(|| format!("lettura di {}", path.display()))?;
        if letti == 0 {
            break;
        }
        hasher.update(&buf[..letti]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

/// Calcola gli hash mancanti nell'indice, in parallelo.
pub fn calcola_hash_mancanti(db: &Db) -> Result<usize> {
    // Prima si legge la lista e **si rilascia il lock**: `Db` tiene la
    // connessione dietro un Mutex, e tenerlo mentre si hasha renderebbe il
    // parallelismo di rayon puramente decorativo.
    let da_fare: Vec<(i64, String)> = {
        let conn = db.conn();
        let mut st = conn.prepare(
            // Gli artefatti sono esclusi per contratto: non entrano nel
            // dedup e hasharli costerebbe la parte più grossa della scansione.
            // I file interni a un archivio hanno un path virtuale
            // (`x.zip!/dentro`) che sul filesystem non esiste: niente hash.
            "SELECT id, path FROM file
              WHERE hash IS NULL
                AND tipo <> 'artefatto'
                AND archivio_padre IS NULL",
        )?;
        let v = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        v
    };

    if da_fare.is_empty() {
        return Ok(0);
    }

    let calcolati: Vec<(i64, String)> = da_fare
        .par_iter()
        .filter_map(|(id, path)| {
            // Un file sparito o senza permessi non deve fermare il batch:
            // resterà con hash NULL e verrà ritentato alla prossima passata.
            hash_file(Path::new(path)).ok().map(|h| (*id, h))
        })
        .collect();

    // Riscrittura solo ora che il lavoro pesante è finito.
    for (id, h) in &calcolati {
        db.hash_salva(*id, h)?;
    }
    Ok(calcolati.len())
}

// ---------------------------------------------------------------------------
// Elezione del canonico
// ---------------------------------------------------------------------------

fn regex_copia() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        // Marcatori con cui i sistemi operativi e i browser rinominano una
        // copia: `BOOKWORM(1).zip`, `CV_Antonio_Russo-1.pdf`, `scheda-1.pdf`,
        // `documento copia.pdf`, `report - Copy.docx`.
        Regex::new(r"(?i)(\(\s*\d+\s*\)\s*$|[-_ ]\d{1,2}$|\bcopia\b|\bcopy\b|\bconflicted\b)")
            .expect("regex dei marcatori di copia valida")
    })
}

/// True se il nome porta i segni di essere una copia generata.
///
/// È un'euristica e serve solo come spareggio: `XXXXXX_01.pdf` la fa scattare
/// pur essendo il primo file di una serie, ma a parità di lunghezza del path
/// sbagliare qui costa al massimo la scelta di quale gemello chiamare canonico.
pub fn pare_copia(nome: &str) -> bool {
    let stelo = match nome.rsplit_once('.') {
        Some((s, _)) if !s.is_empty() => s,
        _ => nome,
    };
    regex_copia().is_match(stelo)
}

/// Chiave d'ordinamento per l'elezione del canonico, nell'ordine deciso in
/// analisi:
/// 1. il path più corto — sta più vicino alla radice, è quello "a posto";
/// 2. il nome che NON porta marcatori di copia;
/// 3. l'mtime più vecchio — l'originale precede la copia;
/// 4. il path in ordine alfabetico, solo per rendere l'esito deterministico.
fn chiave_canonico(f: &FileRecord) -> (usize, u8, i64, String) {
    (
        f.path.chars().count(),
        pare_copia(&f.nome) as u8,
        f.mtime,
        f.path.clone(),
    )
}

/// Sceglie il canonico del gruppo e restituisce `(canonico, duplicati)`.
fn eleggi(mut gruppo: Vec<FileRecord>) -> (FileRecord, Vec<FileRecord>) {
    gruppo.sort_by_key(chiave_canonico);
    let canonico = gruppo.remove(0);
    (canonico, gruppo)
}

// ---------------------------------------------------------------------------
// Testo normalizzato
// ---------------------------------------------------------------------------

/// Collassa ogni sequenza di spazi in uno solo. Due render dello stesso PDF
/// (`accessibile.pdf` / `corretto.pdf`) differiscono per spaziatura e a capo:
/// senza questa normalizzazione non si riconoscerebbero mai.
pub fn normalizza_testo(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    let mut spazio = false;
    for c in t.chars() {
        if c.is_whitespace() {
            spazio = true;
        } else {
            if spazio && !out.is_empty() {
                out.push(' ');
            }
            spazio = false;
            out.push(c);
        }
    }
    out
}

/// Impronta del testo normalizzato. Si confronta l'impronta e non il testo
/// intero: con migliaia di PDF il confronto a coppie sarebbe quadratico.
pub fn impronta_testo(t: &str) -> Option<String> {
    let n = normalizza_testo(t);
    if n.len() < MINIMO_TESTO_CONFRONTABILE {
        return None;
    }
    Some(blake3::hash(n.as_bytes()).to_hex().to_string())
}

// ---------------------------------------------------------------------------
// Gruppi
// ---------------------------------------------------------------------------

/// Raggruppa i duplicati e aggiorna lo `stato` dei file nell'indice.
pub fn gruppi(db: &Db) -> Result<Vec<GruppoDuplicati>> {
    let esatti = gruppi_esatti(db)?;

    // I file già dichiarati duplicati da un gruppo esatto non devono ricomparire
    // in un gruppo per testo, o lo spazio recuperabile verrebbe contato due volte.
    let gia_duplicati: HashSet<i64> = esatti
        .iter()
        .flat_map(|g| g.duplicati.iter().map(|f| f.id))
        .collect();

    let per_testo = gruppi_per_testo(db, &gia_duplicati)?;

    let mut tutti = esatti;
    tutti.extend(per_testo);
    // I gruppi che liberano più spazio per primi: è l'ordine in cui l'utente li
    // vuole vedere.
    tutti.sort_by(|a, b| b.spazio_recuperabile.cmp(&a.spazio_recuperabile));

    scrivi_stati(db, &tutti)?;
    Ok(tutti)
}

fn gruppi_esatti(db: &Db) -> Result<Vec<GruppoDuplicati>> {
    let conn = db.conn();
    let sql = format!(
        "SELECT {COLONNE_FILE} FROM file
          WHERE hash IS NOT NULL AND hash <> ''
            AND tipo <> 'artefatto'
            AND hash IN (SELECT hash FROM file
                          WHERE hash IS NOT NULL AND hash <> '' AND tipo <> 'artefatto'
                          GROUP BY hash HAVING COUNT(*) > 1)
          ORDER BY hash, path"
    );
    let mut st = conn.prepare(&sql)?;
    let righe = st
        .query_map([], riga_in_file)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(st);
    drop(conn);

    let mut per_hash: BTreeMap<String, Vec<FileRecord>> = BTreeMap::new();
    for r in righe {
        if let Some(h) = r.hash.clone() {
            per_hash.entry(h).or_default().push(r);
        }
    }

    Ok(per_hash
        .into_iter()
        .filter(|(_, v)| v.len() > 1)
        .map(|(chiave, v)| costruisci(chiave, "esatto", v))
        .collect())
}

fn gruppi_per_testo(db: &Db, gia_duplicati: &HashSet<i64>) -> Result<Vec<GruppoDuplicati>> {
    let conn = db.conn();
    let sql = format!(
        "SELECT {}, ft.testo AS testo
           FROM file f JOIN file_fts ft ON ft.rowid = f.id
          WHERE f.testo_estratto = 1
            AND f.tipo <> 'artefatto'
            AND lower(COALESCE(f.ext, '')) = 'pdf'",
        COLONNE_FILE
            .split(',')
            .map(|c| format!("f.{}", c.trim()))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let mut st = conn.prepare(&sql)?;
    let mut righe = st.query([])?;

    // Il testo si consuma riga per riga e non si accumula: con migliaia di PDF
    // da 2 MB di testo ciascuno tenerli tutti in RAM non è un'opzione.
    let mut per_impronta: BTreeMap<String, Vec<FileRecord>> = BTreeMap::new();
    while let Some(r) = righe.next()? {
        let rec = riga_in_file(r)?;
        if gia_duplicati.contains(&rec.id) {
            continue;
        }
        let testo: String = r.get("testo")?;
        if let Some(imp) = impronta_testo(&testo) {
            per_impronta.entry(imp).or_default().push(rec);
        }
    }
    drop(righe);
    drop(st);
    drop(conn);

    Ok(per_impronta
        .into_iter()
        .filter(|(_, v)| {
            // Almeno due file, e con hash diversi: se i byte coincidono il caso
            // è già coperto dal gruppo esatto e ripeterlo qui è solo rumore.
            if v.len() < 2 {
                return false;
            }
            let distinti: HashSet<&str> =
                v.iter().filter_map(|f| f.hash.as_deref()).collect();
            distinti.len() != 1 || v.iter().any(|f| f.hash.is_none())
        })
        .map(|(chiave, v)| costruisci(chiave, "testo", v))
        .collect())
}

fn costruisci(chiave: String, genere: &str, membri: Vec<FileRecord>) -> GruppoDuplicati {
    let (canonico, duplicati) = eleggi(membri);
    let spazio_recuperabile = duplicati.iter().map(|f| f.size).sum();
    GruppoDuplicati {
        chiave,
        genere: genere.to_string(),
        canonico,
        duplicati,
        spazio_recuperabile,
    }
}

/// Riporta lo `stato` dell'indice in linea con i gruppi appena calcolati, in
/// una transazione sola: uno stato a metà sarebbe peggio di nessuno stato.
fn scrivi_stati(db: &Db, gruppi: &[GruppoDuplicati]) -> Result<()> {
    let mut conn = db.conn();
    let tx = conn.transaction()?;
    // Chi non è più duplicato deve tornare canonico. Gli `orfano` non si
    // toccano: quell'asse lo decide un altro modulo.
    tx.execute("UPDATE file SET stato = 'canonico' WHERE stato = 'duplicato'", [])?;
    {
        let mut st = tx.prepare("UPDATE file SET stato = 'duplicato' WHERE id = ?1")?;
        for g in gruppi {
            for d in &g.duplicati {
                st.execute([d.id])?;
            }
        }
    }
    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use super::*;
    use crate::db::FileVisto;
    use crate::types::{Fascia, Stato, Tipo};

    fn dir_temp(etichetta: &str) -> std::path::PathBuf {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let d = std::env::temp_dir().join(format!("setaccio-{etichetta}-{n}"));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn hash_stabile_su_contenuto_noto() {
        let d = dir_temp("hash");
        let p = d.join("a.txt");
        let contenuto = b"CONTRATTO PER IL SERVIZIO DI CONSULENZA IN MATERIA DI INVESTIMENTI";
        std::fs::write(&p, contenuto).unwrap();

        let atteso = blake3::hash(contenuto).to_hex().to_string();
        assert_eq!(hash_file(&p).unwrap(), atteso);
        assert_eq!(atteso.len(), 64, "BLAKE3 in esadecimale");

        // Stesso contenuto, nome e cartella diversi: stesso hash.
        let q = d.join("b.txt");
        std::fs::write(&q, contenuto).unwrap();
        assert_eq!(hash_file(&p).unwrap(), hash_file(&q).unwrap());

        // Un byte diverso cambia tutto.
        std::fs::write(&q, b"CONTRATTO PER IL SERVIZIO DI CONSULENZA IN MATERIA DI INVESTIMENTO").unwrap();
        assert_ne!(hash_file(&p).unwrap(), hash_file(&q).unwrap());

        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn hash_a_blocchi_regge_file_piu_grandi_del_buffer() {
        let d = dir_temp("hashgrosso");
        let p = d.join("grosso.bin");
        // Più blocchi da 256 KB: se il loop fosse sbagliato l'hash non tornerebbe.
        let contenuto: Vec<u8> = (0..(BLOCCO * 3 + 7)).map(|i| (i % 251) as u8).collect();
        std::fs::write(&p, &contenuto).unwrap();
        assert_eq!(
            hash_file(&p).unwrap(),
            blake3::hash(&contenuto).to_hex().to_string()
        );
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn hash_di_file_inesistente_da_errore() {
        assert!(hash_file(Path::new("/non/esiste/mai.bin")).is_err());
    }

    #[test]
    fn marcatori_di_copia_sui_nomi_reali() {
        // Casi presi dalla cartella Scaricati dell'utente.
        assert!(pare_copia("BOOKWORM(1).zip"));
        assert!(pare_copia("BOOKWORM (1).zip"));
        assert!(pare_copia("CV_Antonio_Russo-1.pdf"));
        assert!(pare_copia("scheda-1.pdf"));
        assert!(pare_copia("relazione copia.docx"));
        assert!(pare_copia("report - Copy.docx"));

        assert!(!pare_copia("BOOKWORM.zip"));
        assert!(!pare_copia("bookworm_admin.zip"));
        assert!(!pare_copia("CV_Antonio_Russo.pdf"));
        assert!(!pare_copia("scheda.pdf"));
        assert!(
            !pare_copia("000000001.pdf"),
            "una serie numerata non è una copia"
        );
        assert!(
            !pare_copia("T1Q73KFP_0000001.pdf"),
            "i lotti hanno numeri lunghi, non marcatori di copia"
        );
    }

    fn rec(id: i64, path: &str, size: i64, mtime: i64) -> FileRecord {
        FileRecord {
            id,
            nome: path.rsplit('/').next().unwrap().to_string(),
            path: path.to_string(),
            ext: None,
            size,
            mtime,
            hash: Some("h".into()),
            tipo: Tipo::Documento,
            contesto: None,
            stato: Stato::Canonico,
            motivo_tipo: None,
            motivo_contesto: None,
            sorgente_id: 1,
            archivio_padre: None,
            lotto: None,
            testo_estratto: false,
            pagine: None,
        }
    }

    #[test]
    fn canonico_preferisce_il_path_piu_corto() {
        let (c, d) = eleggi(vec![
            rec(1, "/home/u/Scaricati/vecchi/2024/BOOKWORM.zip", 10, 100),
            rec(2, "/home/u/Scaricati/BOOKWORM.zip", 10, 900),
        ]);
        assert_eq!(c.id, 2, "vince il path più corto anche se più recente");
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn canonico_scarta_i_marcatori_di_copia_a_parita_di_lunghezza() {
        // Stessa lunghezza di path: decide il marcatore di copia.
        let (c, _) = eleggi(vec![
            rec(1, "/x/BOOKWORM(1).zip", 272_000_000, 100),
            rec(2, "/x/BOOKWORM_a.zip", 272_000_000, 900),
        ]);
        assert_eq!(c.id, 2, "«(1)» perde contro un nome senza marcatore");

        let (c, dup) = eleggi(vec![
            rec(1, "/x/CV_Antonio_Russo-1.pdf", 500, 100),
            rec(2, "/x/CV_Antonio_Russo_x.pdf", 500, 900),
        ]);
        assert_eq!(c.id, 2);
        assert_eq!(dup[0].nome, "CV_Antonio_Russo-1.pdf");
    }

    #[test]
    fn canonico_a_parita_di_nome_prende_il_piu_vecchio() {
        let (c, _) = eleggi(vec![
            rec(1, "/x/319313/scheda.pdf", 500, 1_700_000_000),
            rec(2, "/x/319312/scheda.pdf", 500, 1_600_000_000),
        ]);
        assert_eq!(c.id, 2, "a parità di path e nome vince l'mtime più vecchio");
    }

    /// Inserisce un file nell'indice e ritorna il suo id.
    fn inserisci(db: &Db, sorg: i64, path: &str, ext: &str, size: i64, mtime: i64) -> i64 {
        let nome = path.rsplit('/').next().unwrap().to_string();
        let f = FileVisto {
            path: path.into(),
            nome,
            ext: Some(ext.into()),
            size,
            mtime,
            tipo: Tipo::Documento,
            motivo_tipo: "test".into(),
            contesto: None,
            motivo_contesto: None,
            sorgente_id: sorg,
            archivio_padre: None,
            lotto: None,
        };
        db.file_upsert(&f).unwrap().0
    }

    #[test]
    fn gruppi_esatti_e_stato_aggiornato() {
        let db = Db::in_memoria().unwrap();
        let s = db.sorgente_aggiungi("/x", Fascia::Documenti).unwrap();

        let a = inserisci(&db, s, "/x/BOOKWORM.zip", "zip", 272_000_000, 100);
        let b = inserisci(&db, s, "/x/vecchi/BOOKWORM(1).zip", "zip", 272_000_000, 200);
        let c = inserisci(&db, s, "/x/altro.zip", "zip", 10, 300);
        db.hash_salva(a, "aaaa").unwrap();
        db.hash_salva(b, "aaaa").unwrap();
        db.hash_salva(c, "cccc").unwrap();

        let g = gruppi(&db).unwrap();
        assert_eq!(g.len(), 1, "un solo gruppo: «altro.zip» è unico");
        assert_eq!(g[0].genere, "esatto");
        assert_eq!(g[0].chiave, "aaaa");
        assert_eq!(g[0].canonico.id, a, "path più corto e senza «(1)»");
        assert_eq!(g[0].duplicati.len(), 1);
        assert_eq!(g[0].spazio_recuperabile, 272_000_000);

        assert_eq!(db.file_per_id(a).unwrap().unwrap().stato, Stato::Canonico);
        assert_eq!(db.file_per_id(b).unwrap().unwrap().stato, Stato::Duplicato);
        assert_eq!(db.file_per_id(c).unwrap().unwrap().stato, Stato::Canonico);
    }

    #[test]
    fn duplicato_che_sparisce_torna_canonico() {
        let db = Db::in_memoria().unwrap();
        let s = db.sorgente_aggiungi("/x", Fascia::Documenti).unwrap();
        let a = inserisci(&db, s, "/x/a.pdf", "pdf", 10, 100);
        let b = inserisci(&db, s, "/x/lungo/b.pdf", "pdf", 10, 200);
        db.hash_salva(a, "zz").unwrap();
        db.hash_salva(b, "zz").unwrap();
        gruppi(&db).unwrap();
        assert_eq!(db.file_per_id(b).unwrap().unwrap().stato, Stato::Duplicato);

        // Il contenuto di b cambia: non è più un duplicato.
        db.hash_salva(b, "ww").unwrap();
        let g = gruppi(&db).unwrap();
        assert!(g.is_empty());
        assert_eq!(
            db.file_per_id(b).unwrap().unwrap().stato,
            Stato::Canonico,
            "lo stato va ricalcolato, non solo accumulato"
        );
    }

    #[test]
    fn gruppo_per_testo_su_pdf_con_byte_diversi() {
        let db = Db::in_memoria().unwrap();
        let s = db.sorgente_aggiungi("/x", Fascia::Documenti).unwrap();

        // Il caso reale: stesso documento riesportato, testo identico a meno di
        // spaziatura, byte completamente diversi.
        let testo_a = "OGGETTO: CONTRATTO PER IL SERVIZIO DI CONSULENZA IN MATERIA DI INVESTIMENTI N. 667 - COMUNICAZIONE DI RECESSO";
        let testo_b = "OGGETTO:   CONTRATTO PER IL SERVIZIO   DI CONSULENZA IN MATERIA\nDI INVESTIMENTI N. 667 -\n\nCOMUNICAZIONE DI RECESSO";

        let a = inserisci(&db, s, "/x/corretto.pdf", "pdf", 90_000, 100);
        let b = inserisci(&db, s, "/x/accessibile.pdf", "pdf", 120_000, 200);
        db.hash_salva(a, "1111").unwrap();
        db.hash_salva(b, "2222").unwrap();
        db.testo_salva(a, "corretto.pdf", testo_a, Some(1)).unwrap();
        db.testo_salva(b, "accessibile.pdf", testo_b, Some(1)).unwrap();

        let g = gruppi(&db).unwrap();
        assert_eq!(g.len(), 1, "gruppi: {g:?}");
        assert_eq!(g[0].genere, "testo");
        assert_eq!(g[0].canonico.id, a, "«corretto.pdf» ha il path più corto");
        assert_eq!(g[0].duplicati[0].id, b);
        assert_eq!(g[0].spazio_recuperabile, 120_000);
        assert_eq!(db.file_per_id(b).unwrap().unwrap().stato, Stato::Duplicato);
    }

    #[test]
    fn stesso_testo_e_stessi_byte_resta_un_solo_gruppo() {
        let db = Db::in_memoria().unwrap();
        let s = db.sorgente_aggiungi("/x", Fascia::Documenti).unwrap();
        let testo = "OGGETTO: CONTRATTO PER IL SERVIZIO DI CONSULENZA IN MATERIA DI INVESTIMENTI N. 667";
        let a = inserisci(&db, s, "/x/a.pdf", "pdf", 100, 100);
        let b = inserisci(&db, s, "/x/dir/b.pdf", "pdf", 100, 200);
        db.hash_salva(a, "uguale").unwrap();
        db.hash_salva(b, "uguale").unwrap();
        db.testo_salva(a, "a.pdf", testo, Some(1)).unwrap();
        db.testo_salva(b, "b.pdf", testo, Some(1)).unwrap();

        let g = gruppi(&db).unwrap();
        assert_eq!(g.len(), 1, "niente doppio conteggio esatto+testo: {g:?}");
        assert_eq!(g[0].genere, "esatto");
        assert_eq!(g[0].spazio_recuperabile, 100);
    }

    #[test]
    fn pdf_con_testo_troppo_corto_non_fa_gruppo() {
        let db = Db::in_memoria().unwrap();
        let s = db.sorgente_aggiungi("/x", Fascia::Documenti).unwrap();
        let a = inserisci(&db, s, "/x/a.pdf", "pdf", 100, 100);
        let b = inserisci(&db, s, "/x/b.pdf", "pdf", 100, 200);
        db.hash_salva(a, "1").unwrap();
        db.hash_salva(b, "2").unwrap();
        db.testo_salva(a, "a.pdf", "Pagina 1", Some(1)).unwrap();
        db.testo_salva(b, "b.pdf", "Pagina 1", Some(1)).unwrap();
        assert!(
            gruppi(&db).unwrap().is_empty(),
            "due frontespizi vuoti non sono lo stesso documento"
        );
    }

    #[test]
    fn calcola_hash_mancanti_ignora_artefatti_e_voci_di_archivio() {
        let d = dir_temp("hashmancanti");
        let vero = d.join("vero.txt");
        std::fs::write(&vero, b"contenuto reale del file").unwrap();
        let artefatto = d.join("build.log");
        std::fs::write(&artefatto, b"output di build").unwrap();

        let db = Db::in_memoria().unwrap();
        let s = db
            .sorgente_aggiungi(&d.to_string_lossy(), Fascia::Documenti)
            .unwrap();

        let id_vero = inserisci(&db, s, &vero.to_string_lossy(), "txt", 24, 100);

        // Un artefatto: non va hashato.
        let f = FileVisto {
            path: artefatto.to_string_lossy().into_owned(),
            nome: "build.log".into(),
            ext: Some("log".into()),
            size: 15,
            mtime: 100,
            tipo: Tipo::Artefatto,
            motivo_tipo: "dentro un repo".into(),
            contesto: None,
            motivo_contesto: None,
            sorgente_id: s,
            archivio_padre: None,
            lotto: None,
        };
        let id_art = db.file_upsert(&f).unwrap().0;

        // Una voce interna a un archivio: il path non esiste sul filesystem.
        let f = FileVisto {
            path: format!("{}/pacco.zip!/dentro/x.txt", d.to_string_lossy()),
            nome: "x.txt".into(),
            ext: Some("txt".into()),
            size: 5,
            mtime: 100,
            tipo: Tipo::Documento,
            motivo_tipo: "dentro archivio".into(),
            contesto: None,
            motivo_contesto: None,
            sorgente_id: s,
            archivio_padre: Some(format!("{}/pacco.zip", d.to_string_lossy())),
            lotto: None,
        };
        let id_zip = db.file_upsert(&f).unwrap().0;

        let quanti = calcola_hash_mancanti(&db).unwrap();
        assert_eq!(quanti, 1, "solo il file vero va hashato");
        assert_eq!(
            db.file_per_id(id_vero).unwrap().unwrap().hash,
            Some(hash_file(&vero).unwrap())
        );
        assert!(db.file_per_id(id_art).unwrap().unwrap().hash.is_none());
        assert!(db.file_per_id(id_zip).unwrap().unwrap().hash.is_none());

        // Seconda passata: non c'è più niente da fare.
        assert_eq!(calcola_hash_mancanti(&db).unwrap(), 0);

        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn normalizzazione_del_testo() {
        assert_eq!(normalizza_testo("  a   b \n\n c  "), "a b c");
        assert_eq!(normalizza_testo(""), "");
        assert_eq!(
            impronta_testo("troppo corto"),
            None,
            "sotto la soglia non si produce impronta"
        );
    }
}
