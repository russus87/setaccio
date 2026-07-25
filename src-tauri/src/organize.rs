//! Operazioni sul filesystem — l'unico modulo che tocca il disco.
//!
//! Regole non negoziabili, in ordine di importanza:
//! 1. Mai `rm`. I file vanno in quarantena, non nel nulla.
//! 2. Mai sovrascrivere: se la destinazione è occupata la mossa viene saltata,
//!    non risolta d'ufficio.
//! 3. Ogni mossa finisce nel journal, così l'undo è sempre possibile.
//! 4. Il piano si vede prima. Nessuna operazione parte senza che l'utente
//!    abbia visto l'elenco completo di cosa sta per succedere.
//!
//! Conseguenza diretta della regola 1: qui dentro non compare **mai** una
//! `std::fs::remove_*`, nemmeno nei test. L'unica primitiva usata è
//! `std::fs::rename`, che è atomica e reversibile. È anche il motivo per cui
//! una mossa fra filesystem diversi non viene eseguita: `rename` fallirebbe
//! con `EXDEV` e l'alternativa (copia + cancellazione) violerebbe la regola 1.

use anyhow::{bail, Context, Result};
use rusqlite::{params, OptionalExtension};
use std::path::{Component, Path, PathBuf};

use crate::db::Db;
use crate::types::{Batch, EsitoOperazioni, Mossa, PianoOperazioni};

/// Batch mostrati dalla schermata di undo.
const BATCH_RECENTI_MAX: i64 = 100;

// ---------------------------------------------------------------------------
// Quarantena
// ---------------------------------------------------------------------------

/// Cartella di quarantena, dentro la data dir dell'applicazione.
pub fn cartella_quarantena() -> Result<std::path::PathBuf> {
    let dir = dirs::data_dir()
        .context("impossibile determinare la data dir dell'utente")?
        .join("setaccio")
        .join("_da-eliminare");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("impossibile creare «{}»", dir.display()))?;
    Ok(dir)
}

/// Destinazione in quarantena di un file: sotto la radice si ricostruisce
/// l'intero path di origine privato della radice del filesystem.
///
/// Perché tutto il path e non solo un pezzo: due `report.pdf` di cartelle
/// diverse non possono collidere in nessun caso, e la provenienza resta
/// leggibile a occhio dentro la quarantena — che è la cosa che serve quando si
/// deve decidere se buttare davvero o rimettere a posto.
fn destinazione_quarantena(radice: &Path, origine: &Path) -> PathBuf {
    let mut p = radice.to_path_buf();
    for c in origine.components() {
        match c {
            Component::Normal(s) => p.push(s),
            Component::Prefix(pre) => {
                // Su Windows la lettera di unità diventa un livello di
                // cartella (`C:\x\y` → `<quarantena>\C\x\y`).
                let s = pre.as_os_str().to_string_lossy().replace([':', '\\'], "");
                if !s.is_empty() {
                    p.push(s);
                }
            }
            // RootDir / CurDir / ParentDir non producono livelli.
            _ => {}
        }
    }
    p
}

// ---------------------------------------------------------------------------
// Filesystem diversi
// ---------------------------------------------------------------------------

/// Primo antenato esistente di un path: su una destinazione ancora da creare
/// il device si può leggere solo dalla cartella che la conterrà.
fn primo_esistente(p: &Path) -> Option<PathBuf> {
    let mut cur = Some(p);
    while let Some(c) = cur {
        if c.exists() {
            return Some(c.to_path_buf());
        }
        cur = c.parent();
    }
    None
}

#[cfg(unix)]
fn stesso_filesystem(a: &Path, b: &Path) -> Option<bool> {
    use std::os::unix::fs::MetadataExt;
    let da = std::fs::metadata(primo_esistente(a)?).ok()?.dev();
    let db = std::fs::metadata(primo_esistente(b)?).ok()?.dev();
    Some(da == db)
}

#[cfg(windows)]
fn stesso_filesystem(a: &Path, b: &Path) -> Option<bool> {
    // Su Windows non c'è un `dev()`: ci si limita a confrontare la lettera di
    // unità, che è quanto basta per distinguere `C:` da un disco esterno.
    fn unita(p: &Path) -> Option<String> {
        let s = p.to_string_lossy().to_ascii_uppercase();
        let b = s.as_bytes();
        if b.len() >= 2 && b[1] == b':' {
            Some((b[0] as char).to_string())
        } else if s.starts_with("\\\\") {
            // UNC: si usa `\\server\share` come identificativo di volume.
            Some(s.split('\\').filter(|x| !x.is_empty()).take(2).collect::<Vec<_>>().join("\\"))
        } else {
            None
        }
    }
    let ua = unita(&primo_esistente(a)?)?;
    let ub = unita(&primo_esistente(b)?)?;
    Some(ua == ub)
}

#[cfg(not(any(unix, windows)))]
fn stesso_filesystem(_a: &Path, _b: &Path) -> Option<bool> {
    None
}

/// Avviso da mettere sulla mossa quando i due path non stanno sullo stesso
/// filesystem. `None` se stanno insieme o se non si è potuto stabilire.
fn avviso_filesystem(origine: &Path, destinazione: &Path) -> Option<String> {
    match stesso_filesystem(origine, destinazione) {
        Some(false) => Some(
            "origine e destinazione stanno su filesystem diversi: lo spostamento \
             diventerebbe copia + cancellazione, che ha tutt'altro rischio e tutt'altra \
             durata. La mossa non viene eseguita."
                .into(),
        ),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Costruzione dei piani
// ---------------------------------------------------------------------------

/// Identificativo di batch leggibile e ordinabile: `AAAAMMGG-HHMMSS-NNN`.
/// Niente valori casuali — il contatore si ricava dal journal, così due batch
/// nello stesso secondo restano distinti e l'ordine alfabetico coincide con
/// l'ordine cronologico.
fn nuovo_batch(db: &Db) -> Result<String> {
    let prefisso = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let conn = db.conn();
    let usati: i64 = conn.query_row(
        "SELECT COUNT(*) FROM (SELECT DISTINCT batch FROM operazione WHERE batch LIKE ?1)",
        params![format!("{prefisso}-%")],
        |r| r.get(0),
    )?;
    Ok(format!("{prefisso}-{:03}", usati + 1))
}

/// Assembla il piano a partire dalle mosse, contando eseguibili, saltate e
/// spazio liberato. `dimensioni` associa a ogni `file_id` la sua dimensione.
fn assembla(batch: String, mosse: Vec<Mossa>, dimensioni: &[(i64, i64)]) -> PianoOperazioni {
    let eseguibili = mosse.iter().filter(|m| m.eseguibile).count();
    let saltate = mosse.len() - eseguibili;
    let spazio_liberato = mosse
        .iter()
        .filter(|m| m.eseguibile)
        .filter_map(|m| {
            dimensioni
                .iter()
                .find(|(id, _)| *id == m.file_id)
                .map(|(_, s)| *s)
        })
        .sum();
    PianoOperazioni {
        batch,
        mosse,
        spazio_liberato,
        eseguibili,
        saltate,
    }
}

/// Prepara il piano per mandare in quarantena i file indicati.
pub fn piano_quarantena(db: &Db, file_ids: &[i64]) -> Result<PianoOperazioni> {
    let radice = cartella_quarantena()?;
    piano_quarantena_in(db, file_ids, &radice)
}

/// Variante con radice esplicita: serve ai test per non sporcare la data dir
/// vera dell'utente.
pub fn piano_quarantena_in(db: &Db, file_ids: &[i64], radice: &Path) -> Result<PianoOperazioni> {
    let mut mosse = Vec::with_capacity(file_ids.len());
    let mut dimensioni = Vec::with_capacity(file_ids.len());
    let mut gia_destinate: Vec<String> = Vec::new();

    for &id in file_ids {
        let Some(f) = db.file_per_id(id)? else {
            mosse.push(Mossa {
                file_id: id,
                origine: String::new(),
                destinazione: String::new(),
                genere: "quarantena".into(),
                motivo: "richiesto dall'utente".into(),
                eseguibile: false,
                avviso: Some(format!("il file {id} non è più nell'indice")),
            });
            continue;
        };
        dimensioni.push((id, f.size));

        let origine = PathBuf::from(&f.path);
        let destinazione = destinazione_quarantena(radice, &origine);
        let mut eseguibile = true;
        let mut avviso = None;

        if !origine.exists() {
            eseguibile = false;
            avviso = Some("l'origine non esiste più sul disco".into());
        } else if destinazione.exists() {
            // Regola 2: mai sovrascrivere, e mai inventare un suffisso per
            // aggirare il problema. L'utente deve vedere che c'è già qualcosa.
            eseguibile = false;
            avviso = Some(format!(
                "in quarantena esiste già «{}»: la mossa va risolta a mano",
                destinazione.display()
            ));
        } else if gia_destinate.contains(&destinazione.to_string_lossy().into_owned()) {
            eseguibile = false;
            avviso = Some("due mosse dello stesso piano puntano allo stesso file".into());
        } else if let Some(a) = avviso_filesystem(&origine, &destinazione) {
            eseguibile = false;
            avviso = Some(a);
        }

        if eseguibile {
            gia_destinate.push(destinazione.to_string_lossy().into_owned());
        }

        mosse.push(Mossa {
            file_id: id,
            origine: f.path.clone(),
            destinazione: destinazione.to_string_lossy().into_owned(),
            genere: "quarantena".into(),
            motivo: motivo_quarantena(db, &f)?,
            eseguibile,
            avviso,
        });
    }

    Ok(assembla(nuovo_batch(db)?, mosse, &dimensioni))
}

/// Perché questo file è candidato alla quarantena. Il motivo si legge
/// dall'indice, non lo si inventa: se è un duplicato si nomina il canonico,
/// altrimenti è una scelta esplicita dell'utente e va detto.
fn motivo_quarantena(db: &Db, f: &crate::types::FileRecord) -> Result<String> {
    use crate::types::Stato;
    match f.stato {
        Stato::Duplicato => {
            let conn = db.conn();
            let canonico: Option<String> = match f.hash.as_deref() {
                Some(h) => conn
                    .query_row(
                        "SELECT path FROM file
                          WHERE hash = ?1 AND stato = 'canonico' AND id <> ?2
                          ORDER BY id LIMIT 1",
                        params![h, f.id],
                        |r| r.get(0),
                    )
                    .optional()?,
                None => None,
            };
            Ok(match canonico {
                Some(p) => format!("duplicato di {p}"),
                None => "duplicato (canonico non più nell'indice)".into(),
            })
        }
        Stato::Orfano => Ok("orfano: il contenuto esiste già estratto accanto".into()),
        Stato::Canonico => Ok("scelto dall'utente".into()),
    }
}

/// Traduce un contesto in un percorso relativo sicuro.
///
/// I contesti sono gerarchici (`lavoro/PAM` diventa `lavoro/` con dentro
/// `PAM/`), quindi il separatore va onorato — ma proprio per questo il valore
/// non può essere concatenato alla cieca: `join` con un path assoluto scarta
/// la radice (`radice.join("/etc")` è `/etc`), e un `..` risalirebbe fuori
/// dalla cartella scelta. I contesti li scrive l'utente nelle regole, quindi
/// qui si accettano solo segmenti normali e non vuoti.
pub fn contesto_sicuro(contesto: &str) -> Option<PathBuf> {
    let grezzo = contesto.trim().replace('\\', "/");
    if grezzo.is_empty() {
        return None;
    }
    let mut p = PathBuf::new();
    let mut segmenti = 0usize;
    for seg in grezzo.split('/') {
        let seg = seg.trim();
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            return None;
        }
        // Un segmento non deve poter reintrodurre un separatore o un prefisso
        // di unità per vie traverse.
        if seg.contains(':') || seg.contains('\0') {
            return None;
        }
        p.push(seg);
        segmenti += 1;
        // Un contesto profondo dieci livelli è un errore di scrittura, non
        // un'intenzione.
        if segmenti > 8 {
            return None;
        }
    }
    if segmenti == 0 {
        None
    } else {
        Some(p)
    }
}

/// Cartella proposta come destinazione per i contesti indicati: la sorgente
/// che contiene già la maggior parte di quei file.
///
/// Chiedere all'utente di cercare a mano una cartella che nel 99% dei casi è
/// la sorgente stessa è attrito inutile: la si propone, resta modificabile.
pub fn destinazione_suggerita(db: &Db, contesti: &[String]) -> Result<Option<String>> {
    let conn = db.conn();
    let (filtro, args): (String, Vec<String>) = if contesti.is_empty() {
        (String::new(), Vec::new())
    } else {
        let segnaposto = std::iter::repeat("?")
            .take(contesti.len())
            .collect::<Vec<_>>()
            .join(",");
        (
            format!("AND f.contesto IN ({segnaposto})"),
            contesti.to_vec(),
        )
    };
    let sql = format!(
        "SELECT s.path, COUNT(*) AS quanti
           FROM file f JOIN sorgenti s ON s.id = f.sorgente_id
          WHERE f.tipo <> 'artefatto' {filtro}
          GROUP BY s.path
          ORDER BY quanti DESC, s.path
          LIMIT 1"
    );
    let p: Vec<&dyn rusqlite::ToSql> = args.iter().map(|a| a as &dyn rusqlite::ToSql).collect();
    let mut st = conn.prepare(&sql)?;
    let v = st
        .query_row(p.as_slice(), |r| r.get::<_, String>(0))
        .optional()?;
    Ok(v)
}

// ---------------------------------------------------------------------------
// Cartelle vuote
// ---------------------------------------------------------------------------

/// Genere di mossa che rimuove una cartella rimasta vuota.
pub const GENERE_CARTELLA: &str = "rimuovi_cartella";

/// Trova le cartelle rimaste vuote sotto le radici indicate.
///
/// È l'unica eccezione al divieto di cancellare, e regge perché è una
/// cancellazione che non può distruggere dati: si usa `remove_dir`, **mai**
/// `remove_dir_all`, quindi è il sistema operativo stesso a rifiutare se
/// dentro c'è ancora qualcosa. E resta annullabile, perché ricreare una
/// cartella vuota è un'operazione esatta: nel journal finisce come le altre.
///
/// Le radici vuote significano «tutte le sorgenti più la quarantena». La
/// radice stessa non viene mai rimossa: svuotare `~/Scaricati` non deve far
/// sparire `~/Scaricati`.
pub fn piano_cartelle_vuote(db: &Db, radici: &[String]) -> Result<PianoOperazioni> {
    let mut basi: Vec<PathBuf> = if radici.is_empty() {
        db.sorgenti()?
            .into_iter()
            .filter(|s| s.attiva)
            .map(|s| PathBuf::from(s.path))
            .collect()
    } else {
        radici.iter().map(PathBuf::from).collect()
    };
    if radici.is_empty() {
        if let Ok(q) = cartella_quarantena() {
            basi.push(q);
        }
    }

    let mut candidate: Vec<(usize, PathBuf)> = Vec::new();
    for base in &basi {
        if !base.is_dir() {
            continue;
        }
        for voce in walkdir::WalkDir::new(base)
            .min_depth(1)
            .follow_links(false)
            .into_iter()
            .filter_map(|v| v.ok())
        {
            // Mai seguire un link: rimuoverebbe il collegamento, non la
            // cartella, e il conteggio del vuoto sarebbe quello del bersaglio.
            if voce.file_type().is_dir() && !voce.path_is_symlink() {
                candidate.push((voce.depth(), voce.path().to_path_buf()));
            }
        }
    }

    // Dalla più profonda: così una cartella che conteneva solo cartelle vuote
    // risulta a sua volta vuota quando arriva il suo turno, e la pulizia
    // scende a cascata in una passata sola.
    candidate.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

    let mut rimosse: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    let mut mosse = Vec::new();

    for (_, dir) in candidate {
        match vuota_dopo(&dir, &rimosse) {
            Ok(true) => {
                rimosse.insert(dir.clone());
                mosse.push(Mossa {
                    file_id: 0,
                    origine: dir.to_string_lossy().into_owned(),
                    destinazione: String::new(),
                    genere: GENERE_CARTELLA.into(),
                    motivo: "cartella vuota".into(),
                    eseguibile: true,
                    avviso: None,
                });
            }
            Ok(false) => {}
            Err(e) => mosse.push(Mossa {
                file_id: 0,
                origine: dir.to_string_lossy().into_owned(),
                destinazione: String::new(),
                genere: GENERE_CARTELLA.into(),
                motivo: "cartella vuota".into(),
                eseguibile: false,
                avviso: Some(format!("impossibile leggerne il contenuto: {e}")),
            }),
        }
    }

    Ok(assembla(nuovo_batch(db)?, mosse, &[]))
}

fn marca_annullata(db: &Db, id: i64) -> Result<()> {
    let conn = db.conn();
    conn.execute(
        "UPDATE operazione SET annullata = 1 WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

/// Rimuove una cartella vuota e la registra nel journal.
///
/// `remove_dir` e non `remove_dir_all`: se nel frattempo qualcuno ci ha messo
/// dentro qualcosa, è il sistema operativo a rifiutare. La garanzia «non si
/// perdono dati» non dipende quindi da un controllo nostro, che sarebbe
/// comunque soggetto a una corsa fra il controllo e la rimozione.
fn rimuovi_cartella(db: &Db, batch: &str, m: &Mossa) -> Result<()> {
    let dir = Path::new(&m.origine);
    if !dir.exists() {
        bail!("non esiste più");
    }
    std::fs::remove_dir(dir).with_context(|| format!("rimozione di «{}»", m.origine))?;
    let conn = db.conn();
    conn.execute(
        "INSERT INTO operazione (batch, genere, origine, destinazione, file_id)
         VALUES (?1, ?2, ?3, '', NULL)",
        params![batch, GENERE_CARTELLA, m.origine],
    )?;
    Ok(())
}

/// True se la cartella è vuota, o se contiene soltanto cartelle già destinate
/// alla rimozione in questo stesso piano.
fn vuota_dopo(dir: &Path, rimosse: &std::collections::HashSet<PathBuf>) -> std::io::Result<bool> {
    for voce in std::fs::read_dir(dir)? {
        let voce = voce?;
        let p = voce.path();
        let e_dir = voce.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if !e_dir || !rimosse.contains(&p) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Prepara il piano della modalità Organizza: i file dei contesti indicati
/// vengono spostati sotto `destinazione/<contesto>/`.
///
/// Se `destinazione` è vuota si usa la sorgente che contiene già la maggior
/// parte di quei file.
pub fn piano_organizza(db: &Db, destinazione: &str, contesti: &[String]) -> Result<PianoOperazioni> {
    if contesti.is_empty() {
        return Ok(assembla(nuovo_batch(db)?, Vec::new(), &[]));
    }
    let scelta = if destinazione.trim().is_empty() {
        destinazione_suggerita(db, contesti)?.unwrap_or_default()
    } else {
        destinazione.to_string()
    };
    if scelta.trim().is_empty() {
        bail!("nessuna destinazione indicata e nessuna sorgente da cui dedurne una");
    }
    let radice = PathBuf::from(&scelta);

    let file = {
        let conn = db.conn();
        let segnaposto = std::iter::repeat("?")
            .take(contesti.len())
            .collect::<Vec<_>>()
            .join(",");
        // Gli artefatti restano dove sono: sono output di build dentro
        // repository di codice, spostarli romperebbe il progetto che li
        // contiene.
        let sql = format!(
            "SELECT id, path, nome, size, contesto FROM file
              WHERE contesto IN ({segnaposto}) AND tipo <> 'artefatto'
              ORDER BY path"
        );
        let mut st = conn.prepare(&sql)?;
        let p: Vec<&dyn rusqlite::ToSql> =
            contesti.iter().map(|c| c as &dyn rusqlite::ToSql).collect();
        let v = st
            .query_map(p.as_slice(), |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        v
    };

    let mut mosse = Vec::with_capacity(file.len());
    let mut dimensioni = Vec::with_capacity(file.len());
    let mut gia_destinate: Vec<String> = Vec::new();

    for (id, path, nome, size, contesto) in file {
        dimensioni.push((id, size));
        let origine = PathBuf::from(&path);
        let Some(relativo) = contesto_sicuro(&contesto) else {
            mosse.push(Mossa {
                file_id: id,
                origine: path,
                destinazione: String::new(),
                genere: "sposta".into(),
                motivo: format!("contesto «{contesto}»"),
                eseguibile: false,
                avviso: Some(format!(
                    "il contesto «{contesto}» non è un percorso valido: userebbe «..», \
                     un path assoluto o un carattere non ammesso"
                )),
            });
            continue;
        };
        let destinazione = radice.join(&relativo).join(&nome);
        let dest_txt = destinazione.to_string_lossy().into_owned();

        let mut eseguibile = true;
        let mut avviso = None;
        if !origine.exists() {
            eseguibile = false;
            avviso = Some("l'origine non esiste più sul disco".into());
        } else if origine == destinazione {
            eseguibile = false;
            avviso = Some("il file è già al suo posto".into());
        } else if destinazione.exists() {
            eseguibile = false;
            avviso = Some(format!("«{dest_txt}» esiste già: la mossa va risolta a mano"));
        } else if gia_destinate.contains(&dest_txt) {
            // Due file omonimi da cartelle diverse nello stesso contesto: la
            // collisione va vista nel piano, non scoperta a metà esecuzione.
            eseguibile = false;
            avviso = Some(format!(
                "un'altra mossa di questo piano punta già a «{dest_txt}»"
            ));
        } else if let Some(a) = avviso_filesystem(&origine, &destinazione) {
            eseguibile = false;
            avviso = Some(a);
        }

        if eseguibile {
            gia_destinate.push(dest_txt.clone());
        }

        mosse.push(Mossa {
            file_id: id,
            origine: path,
            destinazione: dest_txt,
            genere: "sposta".into(),
            motivo: format!("contesto «{contesto}»"),
            eseguibile,
            avviso,
        });
    }

    Ok(assembla(nuovo_batch(db)?, mosse, &dimensioni))
}

// ---------------------------------------------------------------------------
// Esecuzione
// ---------------------------------------------------------------------------

/// Sposta un file e aggiorna indice e journal. Isolata perché il chiamante
/// possa continuare con le altre mosse quando questa fallisce.
fn sposta(db: &Db, batch: &str, m: &Mossa) -> Result<()> {
    let origine = Path::new(&m.origine);
    let destinazione = Path::new(&m.destinazione);

    if !origine.exists() {
        bail!("l'origine non esiste più");
    }
    // Ricontrollo dopo il piano: fra la costruzione e l'esecuzione può essere
    // passato del tempo, e sovrascrivere non è mai un'opzione.
    if destinazione.exists() {
        bail!("«{}» è occupata", m.destinazione);
    }
    if let Some(p) = destinazione.parent() {
        std::fs::create_dir_all(p)
            .with_context(|| format!("impossibile creare «{}»", p.display()))?;
    }
    std::fs::rename(origine, destinazione)
        .with_context(|| format!("spostamento verso «{}» fallito", m.destinazione))?;

    // Da qui in poi il file è già stato mosso: il journal va scritto per
    // primo, altrimenti un errore successivo lascerebbe una mossa senza undo.
    let conn = db.conn();
    conn.execute(
        "INSERT INTO operazione (batch, genere, origine, destinazione, file_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![batch, m.genere, m.origine, m.destinazione, m.file_id],
    )?;
    if m.file_id > 0 {
        // `file.path` è UNIQUE: una riga rimasta appesa alla destinazione si
        // riferisce a un file che lì non c'è più (l'abbiamo appena verificato),
        // quindi toglierla è corretto e sblocca l'update.
        conn.execute(
            "DELETE FROM file WHERE path = ?1 AND id <> ?2",
            params![m.destinazione, m.file_id],
        )?;
        conn.execute(
            "UPDATE file SET path = ?2 WHERE id = ?1",
            params![m.file_id, m.destinazione],
        )?;
    }
    Ok(())
}

/// Esegue un piano già mostrato all'utente, registrando ogni mossa nel journal.
pub fn esegui(db: &Db, piano: &PianoOperazioni) -> Result<EsitoOperazioni> {
    // L'identificativo si genera qui e non si riusa quello del piano: lo stesso
    // piano potrebbe essere eseguito due volte, e due esecuzioni distinte
    // devono restare due undo distinti.
    let batch = nuovo_batch(db)?;
    let mut eseguite = 0usize;
    let mut fallite = 0usize;
    let mut errori = Vec::new();

    for m in piano.mosse.iter().filter(|m| m.eseguibile) {
        let esito = if m.genere == GENERE_CARTELLA {
            rimuovi_cartella(db, &batch, m)
        } else {
            sposta(db, &batch, m)
        };
        match esito {
            Ok(()) => eseguite += 1,
            Err(e) => {
                // Un errore su una mossa non ferma le altre: l'utente ha
                // approvato un piano, non una transazione tutto-o-niente.
                fallite += 1;
                errori.push(format!("{}: {e}", m.origine));
            }
        }
    }

    Ok(EsitoOperazioni {
        batch,
        eseguite,
        fallite,
        errori,
    })
}

// ---------------------------------------------------------------------------
// Undo
// ---------------------------------------------------------------------------

/// I batch di operazioni più recenti, per la schermata di undo.
pub fn batch_recenti(db: &Db) -> Result<Vec<Batch>> {
    let conn = db.conn();
    let mut st = conn.prepare(
        "SELECT batch,
                MIN(genere)        AS genere,
                COUNT(*)           AS quante,
                MAX(eseguita_il)   AS eseguito_il,
                MIN(annullata)     AS tutte_annullate
           FROM operazione
          GROUP BY batch
          ORDER BY MAX(eseguita_il) DESC, batch DESC
          LIMIT ?1",
    )?;
    let v = st
        .query_map(params![BATCH_RECENTI_MAX], |r| {
            Ok(Batch {
                batch: r.get(0)?,
                genere: r.get(1)?,
                quante: r.get(2)?,
                eseguito_il: r.get(3)?,
                // Il batch si dice annullato solo quando *tutte* le sue mosse
                // lo sono: un undo parziale deve restare visibile come da
                // completare.
                annullato: r.get::<_, i64>(4)? != 0,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(v)
}

/// Riporta indietro tutte le mosse di un batch.
pub fn annulla(db: &Db, batch: &str) -> Result<EsitoOperazioni> {
    let da_annullare: Vec<(i64, String, String, Option<i64>, String)> = {
        let conn = db.conn();
        let mut st = conn.prepare(
            "SELECT id, origine, destinazione, file_id, genere
               FROM operazione
              WHERE batch = ?1 AND annullata = 0
              ORDER BY id DESC",
        )?;
        let v = st
            .query_map(params![batch], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        v
    };

    let mut eseguite = 0usize;
    let mut fallite = 0usize;
    let mut errori = Vec::new();

    // Ordine INVERSO: se il batch ha creato cartelle annidate o ha spostato
    // più file nello stesso posto, disfare a ritroso è l'unico ordine che
    // ripercorre esattamente ciò che è successo.
    for (id, origine, destinazione, file_id, genere) in da_annullare {
        let orig = Path::new(&origine);
        let dest = Path::new(&destinazione);

        // Disfare la rimozione di una cartella vuota vuole dire ricrearla: è
        // un'operazione esatta, perché di quella cartella non c'era altro da
        // conservare che la sua esistenza.
        if genere == GENERE_CARTELLA {
            if orig.exists() {
                marca_annullata(db, id)?;
                eseguite += 1;
                continue;
            }
            match std::fs::create_dir_all(orig) {
                Ok(()) => {
                    marca_annullata(db, id)?;
                    eseguite += 1;
                }
                Err(e) => {
                    fallite += 1;
                    errori.push(format!("{origine}: impossibile ricrearla ({e})"));
                }
            }
            continue;
        }

        if orig.exists() {
            // Nel frattempo qualcosa ha rioccupato l'origine: si salta e lo si
            // dice, non si sovrascrive (regola 2, vale anche all'indietro).
            fallite += 1;
            errori.push(format!("{origine}: l'origine è di nuovo occupata, mossa saltata"));
            continue;
        }
        if !dest.exists() {
            fallite += 1;
            errori.push(format!("{destinazione}: non è più là, niente da riportare indietro"));
            continue;
        }
        if let Some(p) = orig.parent() {
            if let Err(e) = std::fs::create_dir_all(p) {
                fallite += 1;
                errori.push(format!("{origine}: impossibile ricreare la cartella ({e})"));
                continue;
            }
        }
        if let Err(e) = std::fs::rename(dest, orig) {
            fallite += 1;
            errori.push(format!("{origine}: ritorno fallito ({e})"));
            continue;
        }

        let conn = db.conn();
        conn.execute("UPDATE operazione SET annullata = 1 WHERE id = ?1", params![id])?;
        if let Some(fid) = file_id.filter(|f| *f > 0) {
            conn.execute(
                "DELETE FROM file WHERE path = ?1 AND id <> ?2",
                params![origine, fid],
            )?;
            conn.execute(
                "UPDATE file SET path = ?2 WHERE id = ?1",
                params![fid, origine],
            )?;
        }
        eseguite += 1;
    }

    Ok(EsitoOperazioni {
        batch: batch.to_string(),
        eseguite,
        fallite,
        errori,
    })
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use super::*;
    use crate::db::FileVisto;
    use crate::types::{Fascia, Tipo};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static CONTATORE: AtomicUsize = AtomicUsize::new(0);

    /// Cartella temporanea fatta a mano: `tempfile` non è fra le dipendenze e
    /// non è lecito aggiungerne. Non si ripulisce a fine test — regola 1: in
    /// questo file non compare nessuna `remove_*`, nemmeno nei test.
    fn temp(nome: &str) -> PathBuf {
        let n = CONTATORE.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!(
            "setaccio-organize-{}-{}-{}",
            std::process::id(),
            nome,
            n
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn indicizza(db: &Db, sid: i64, path: &Path, contesto: Option<&str>, tipo: Tipo) -> i64 {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        if !path.exists() {
            std::fs::write(path, b"contenuto di prova").unwrap();
        }
        let (id, _) = db
            .file_upsert(&FileVisto {
                path: path.to_string_lossy().into_owned(),
                nome: path.file_name().unwrap().to_string_lossy().into_owned(),
                ext: path
                    .extension()
                    .map(|e| e.to_string_lossy().into_owned()),
                size: std::fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0),
                mtime: 1,
                tipo,
                motivo_tipo: "test".into(),
                contesto: contesto.map(|c| c.to_string()),
                motivo_contesto: None,
                sorgente_id: sid,
                archivio_padre: None,
                lotto: None,
            })
            .unwrap();
        id
    }

    fn db_e_sorgente(radice: &Path) -> (Db, i64) {
        let db = Db::in_memoria().unwrap();
        let sid = db
            .sorgente_aggiungi(radice.to_str().unwrap(), Fascia::Documenti)
            .unwrap();
        (db, sid)
    }

    #[test]
    fn quarantena_e_undo_giro_completo() {
        let base = temp("giro");
        let sorgente = base.join("origine/sotto");
        let quarantena = base.join("quarantena");
        std::fs::create_dir_all(&quarantena).unwrap();
        let file = sorgente.join("report.pdf");

        let (db, sid) = db_e_sorgente(&base);
        let id = indicizza(&db, sid, &file, None, Tipo::Documento);

        let piano = piano_quarantena_in(&db, &[id], &quarantena).unwrap();
        assert_eq!(piano.eseguibili, 1);
        assert_eq!(piano.saltate, 0);
        assert_eq!(piano.spazio_liberato, 18);
        assert_eq!(piano.mosse[0].genere, "quarantena");
        // Il piano non ha ancora toccato niente.
        assert!(file.exists());

        let esito = esegui(&db, &piano).unwrap();
        assert_eq!(esito.eseguite, 1, "errori: {:?}", esito.errori);
        assert_eq!(esito.fallite, 0);
        assert!(!file.exists(), "l'origine deve essere sparita");
        let dest = PathBuf::from(&piano.mosse[0].destinazione);
        assert!(dest.exists(), "il file deve stare in quarantena");
        // Il path in quarantena conserva la provenienza.
        assert!(dest.to_string_lossy().contains("origine"));
        assert_eq!(
            db.file_per_id(id).unwrap().unwrap().path,
            dest.to_string_lossy()
        );

        // Il journal vede il batch, non ancora annullato.
        let batch = batch_recenti(&db).unwrap();
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].quante, 1);
        assert_eq!(batch[0].genere, "quarantena");
        assert!(!batch[0].annullato);

        // Giro di ritorno.
        let undo = annulla(&db, &esito.batch).unwrap();
        assert_eq!(undo.eseguite, 1, "errori: {:?}", undo.errori);
        assert!(file.exists(), "il file deve essere tornato al suo posto");
        assert!(!dest.exists());
        assert_eq!(
            db.file_per_id(id).unwrap().unwrap().path,
            file.to_string_lossy()
        );
        assert!(batch_recenti(&db).unwrap()[0].annullato);

        // Annullare due volte non fa danni: non c'è più niente da disfare.
        let undo2 = annulla(&db, &esito.batch).unwrap();
        assert_eq!(undo2.eseguite, 0);
        assert_eq!(undo2.fallite, 0);
        assert!(file.exists());
    }

    #[test]
    fn destinazione_occupata_rende_la_mossa_non_eseguibile() {
        let base = temp("occupata");
        let sorgente = base.join("origine");
        let quarantena = base.join("quarantena");
        std::fs::create_dir_all(&quarantena).unwrap();
        let file = sorgente.join("report.pdf");

        let (db, sid) = db_e_sorgente(&base);
        let id = indicizza(&db, sid, &file, None, Tipo::Documento);

        // Si occupa in anticipo la destinazione che il piano sceglierebbe.
        let dest = destinazione_quarantena(&quarantena, &file);
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::write(&dest, b"c'ero prima io").unwrap();

        let piano = piano_quarantena_in(&db, &[id], &quarantena).unwrap();
        assert_eq!(piano.eseguibili, 0);
        assert_eq!(piano.saltate, 1);
        assert_eq!(piano.spazio_liberato, 0);
        assert!(!piano.mosse[0].eseguibile);
        assert!(piano.mosse[0].avviso.is_some(), "l'avviso va compilato");

        let esito = esegui(&db, &piano).unwrap();
        assert_eq!(esito.eseguite, 0);
        // Regola 2: il file preesistente non è stato toccato.
        assert_eq!(std::fs::read(&dest).unwrap(), b"c'ero prima io");
        assert!(file.exists());
    }

    #[test]
    fn due_omonimi_non_collidono_in_quarantena() {
        let base = temp("omonimi");
        let quarantena = base.join("quarantena");
        std::fs::create_dir_all(&quarantena).unwrap();
        let a = base.join("uno/report.pdf");
        let b = base.join("due/report.pdf");

        let (db, sid) = db_e_sorgente(&base);
        let ia = indicizza(&db, sid, &a, None, Tipo::Documento);
        let ib = indicizza(&db, sid, &b, None, Tipo::Documento);

        let piano = piano_quarantena_in(&db, &[ia, ib], &quarantena).unwrap();
        assert_eq!(piano.eseguibili, 2);
        assert_ne!(piano.mosse[0].destinazione, piano.mosse[1].destinazione);

        let esito = esegui(&db, &piano).unwrap();
        assert_eq!(esito.eseguite, 2, "errori: {:?}", esito.errori);
        assert!(PathBuf::from(&piano.mosse[0].destinazione).exists());
        assert!(PathBuf::from(&piano.mosse[1].destinazione).exists());
    }

    #[test]
    fn motivo_cita_il_canonico_del_duplicato() {
        let base = temp("motivo");
        let quarantena = base.join("quarantena");
        std::fs::create_dir_all(&quarantena).unwrap();
        let can = base.join("uno/report.pdf");
        let dup = base.join("due/report.pdf");

        let (db, sid) = db_e_sorgente(&base);
        let ic = indicizza(&db, sid, &can, None, Tipo::Documento);
        let id = indicizza(&db, sid, &dup, None, Tipo::Documento);
        db.hash_salva(ic, "abc").unwrap();
        db.hash_salva(id, "abc").unwrap();
        db.conn()
            .execute("UPDATE file SET stato='duplicato' WHERE id=?1", params![id])
            .unwrap();

        let piano = piano_quarantena_in(&db, &[id], &quarantena).unwrap();
        assert_eq!(
            piano.mosse[0].motivo,
            format!("duplicato di {}", can.to_string_lossy())
        );
    }

    #[test]
    fn organizza_salta_artefatti_e_collisioni() {
        let base = temp("organizza");
        let destinazione = base.join("Archivio");
        std::fs::create_dir_all(&destinazione).unwrap();

        let (db, sid) = db_e_sorgente(&base);
        let a = base.join("scaricati/nota.txt");
        let b = base.join("altro/nota.txt"); // stesso nome, stesso contesto
        let c = base.join("repo/target/nota.txt"); // artefatto
        let d = base.join("scaricati/solo.txt");
        indicizza(&db, sid, &a, Some("studio"), Tipo::Documento);
        indicizza(&db, sid, &b, Some("studio"), Tipo::Documento);
        indicizza(&db, sid, &c, Some("studio"), Tipo::Artefatto);
        indicizza(&db, sid, &d, Some("personale"), Tipo::Documento);

        let piano = piano_organizza(
            &db,
            destinazione.to_str().unwrap(),
            &["studio".to_string()],
        )
        .unwrap();

        // L'artefatto non compare proprio nel piano.
        assert_eq!(piano.mosse.len(), 2, "mosse: {:?}", piano.mosse);
        assert!(piano.mosse.iter().all(|m| !m.destinazione.contains("target")));
        // Il secondo omonimo è saltato con avviso, non risolto con un suffisso.
        assert_eq!(piano.eseguibili, 1);
        assert_eq!(piano.saltate, 1);
        let saltata = piano.mosse.iter().find(|m| !m.eseguibile).unwrap();
        assert!(saltata.avviso.as_deref().unwrap().contains("punta già"));

        let esito = esegui(&db, &piano).unwrap();
        assert_eq!(esito.eseguite, 1, "errori: {:?}", esito.errori);
        assert!(destinazione.join("studio/nota.txt").exists());
        // Il file di un contesto non richiesto non si muove.
        assert!(d.exists());
    }

    #[test]
    fn undo_salta_se_l_origine_e_stata_rioccupata() {
        let base = temp("rioccupata");
        let quarantena = base.join("quarantena");
        std::fs::create_dir_all(&quarantena).unwrap();
        let file = base.join("origine/report.pdf");

        let (db, sid) = db_e_sorgente(&base);
        let id = indicizza(&db, sid, &file, None, Tipo::Documento);
        let piano = piano_quarantena_in(&db, &[id], &quarantena).unwrap();
        let esito = esegui(&db, &piano).unwrap();
        assert_eq!(esito.eseguite, 1);

        // Qualcuno rimette un file con lo stesso nome all'origine.
        std::fs::write(&file, b"un altro file").unwrap();
        let undo = annulla(&db, &esito.batch).unwrap();
        assert_eq!(undo.eseguite, 0);
        assert_eq!(undo.fallite, 1);
        assert!(undo.errori[0].contains("occupata"));
        // Niente è stato sovrascritto.
        assert_eq!(std::fs::read(&file).unwrap(), b"un altro file");
        assert!(PathBuf::from(&piano.mosse[0].destinazione).exists());
    }

    #[test]
    fn batch_leggibile_e_ordinabile() {
        let db = Db::in_memoria().unwrap();
        let b1 = nuovo_batch(&db).unwrap();
        // Formato AAAAMMGG-HHMMSS-NNN.
        assert_eq!(b1.len(), 8 + 1 + 6 + 1 + 3);
        assert!(b1.ends_with("-001"));
        assert!(b1.chars().filter(|c| *c == '-').count() == 2);

        // Registrato un batch, il successivo nello stesso secondo incrementa.
        db.conn()
            .execute(
                "INSERT INTO operazione (batch, genere, origine, destinazione) VALUES (?1,'x','a','b')",
                params![b1],
            )
            .unwrap();
        let b2 = nuovo_batch(&db).unwrap();
        assert_ne!(b1, b2);
    }

    #[test]
    fn la_pulizia_toglie_le_vuote_a_cascata_e_risparmia_le_piene() {
        let db = Db::in_memoria().unwrap();
        let base = std::env::temp_dir().join(format!("setaccio-vuote-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&base);

        // vuota/  → va via
        // annidata/dentro/ → vanno via entrambe, a cascata
        // piena/ con un file → resta
        // piena2/sotto/ con un file dentro sotto → restano entrambe
        std::fs::create_dir_all(base.join("vuota")).unwrap();
        std::fs::create_dir_all(base.join("annidata/dentro")).unwrap();
        std::fs::create_dir_all(base.join("piena")).unwrap();
        std::fs::write(base.join("piena/f.txt"), b"x").unwrap();
        std::fs::create_dir_all(base.join("piena2/sotto")).unwrap();
        std::fs::write(base.join("piena2/sotto/f.txt"), b"x").unwrap();

        let radici = vec![base.to_string_lossy().into_owned()];
        let piano = piano_cartelle_vuote(&db, &radici).unwrap();
        let previste: std::collections::HashSet<String> = piano
            .mosse
            .iter()
            .filter(|m| m.eseguibile)
            .map(|m| m.origine.clone())
            .collect();

        assert!(previste.contains(&base.join("vuota").to_string_lossy().into_owned()));
        assert!(previste.contains(&base.join("annidata").to_string_lossy().into_owned()));
        assert!(previste.contains(&base.join("annidata/dentro").to_string_lossy().into_owned()));
        assert!(
            !previste.contains(&base.join("piena").to_string_lossy().into_owned()),
            "una cartella con dentro un file non deve mai finire nel piano"
        );
        assert!(!previste.contains(&base.join("piena2").to_string_lossy().into_owned()));
        assert!(!previste.contains(&base.join("piena2/sotto").to_string_lossy().into_owned()));
        // La radice non si tocca: svuotare Scaricati non deve far sparire Scaricati.
        assert!(!previste.contains(&base.to_string_lossy().into_owned()));

        let esito = esegui(&db, &piano).unwrap();
        assert_eq!(esito.fallite, 0, "errori: {:?}", esito.errori);
        assert!(!base.join("vuota").exists());
        assert!(!base.join("annidata").exists());
        assert!(base.join("piena/f.txt").exists(), "il file non va perso");
        assert!(base.join("piena2/sotto/f.txt").exists());

        // E si può disfare: le cartelle tornano.
        let ann = annulla(&db, &esito.batch).unwrap();
        assert_eq!(ann.fallite, 0, "errori: {:?}", ann.errori);
        assert!(base.join("vuota").is_dir());
        assert!(base.join("annidata/dentro").is_dir());
    }

    #[test]
    fn il_contesto_gerarchico_diventa_cartelle_annidate() {
        assert_eq!(
            contesto_sicuro("lavoro/PAM"),
            Some(PathBuf::from("lavoro").join("PAM"))
        );
        assert_eq!(contesto_sicuro("studio"), Some(PathBuf::from("studio")));
        // Separatori ripetuti o spazi attorno non devono generare segmenti vuoti.
        assert_eq!(
            contesto_sicuro(" lavoro // DevOps "),
            Some(PathBuf::from("lavoro").join("DevOps"))
        );
    }

    #[test]
    fn il_contesto_non_puo_uscire_dalla_destinazione() {
        // `radice.join("/etc")` in Rust vale `/etc`: senza guardia un contesto
        // assoluto scriverebbe fuori dalla cartella scelta dall'utente.
        assert_eq!(contesto_sicuro("/etc"), Some(PathBuf::from("etc")));
        assert_eq!(contesto_sicuro("../../fuori"), None);
        assert_eq!(contesto_sicuro("lavoro/../../fuori"), None);
        assert_eq!(contesto_sicuro("C:/Windows"), None);
        assert_eq!(contesto_sicuro(""), None);
        assert_eq!(contesto_sicuro("   "), None);
        assert_eq!(contesto_sicuro("a/b/c/d/e/f/g/h/i/j"), None);
    }

    #[test]
    fn la_destinazione_proposta_e_la_sorgente_piu_popolata() {
        let db = Db::in_memoria().unwrap();
        let poco = db
            .sorgente_aggiungi("/tmp/poco", crate::types::Fascia::Documenti)
            .unwrap();
        let molto = db
            .sorgente_aggiungi("/tmp/molto", crate::types::Fascia::Documenti)
            .unwrap();

        let inserisci = |sorgente: i64, n: usize, contesto: &str| {
            for i in 0..n {
                let f = crate::db::FileVisto {
                    path: format!("/tmp/s{sorgente}/f{i}.pdf"),
                    nome: format!("f{i}.pdf"),
                    ext: Some("pdf".into()),
                    size: 10,
                    mtime: 1,
                    tipo: crate::types::Tipo::Documento,
                    motivo_tipo: "test".into(),
                    contesto: Some(contesto.into()),
                    motivo_contesto: None,
                    sorgente_id: sorgente,
                    archivio_padre: None,
                    lotto: None,
                };
                db.file_upsert(&f).unwrap();
            }
        };
        inserisci(poco, 1, "lavoro/PAM");
        inserisci(molto, 5, "lavoro/PAM");

        let suggerita = destinazione_suggerita(&db, &["lavoro/PAM".to_string()]).unwrap();
        assert_eq!(suggerita.as_deref(), Some("/tmp/molto"));
    }

    #[test]
    fn la_migrazione_rende_gerarchici_i_contesti_esistenti() {
        let db = Db::in_memoria().unwrap();
        let regole = db.regole().unwrap();
        // Nessuna regola di serie deve più usare il trattino.
        assert!(
            regole.iter().all(|r| !r.valore.starts_with("lavoro-")),
            "le regole builtin devono usare la forma gerarchica"
        );
        assert!(
            regole.iter().any(|r| r.valore == "lavoro/PAM"),
            "atteso il contesto lavoro/PAM fra le regole di serie"
        );
    }
}
