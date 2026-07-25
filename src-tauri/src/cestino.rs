//! Le due operazioni che tolgono un file dal disco davvero.
//!
//! Stanno qui e non in [`crate::organize`] apposta: quel modulo garantisce di
//! non usare mai una primitiva distruttiva, e quella garanzia vale qualcosa
//! solo se resta verificabile leggendolo. Tutto ciò che cancella è
//! concentrato in questo file, che è quindi l'unico da rileggere quando ci si
//! chiede «cosa può perdere dati».
//!
//! Le due operazioni non sono la stessa cosa e la differenza va tenuta
//! visibile fino alla UI:
//!
//! - **cestino** consegna il file al cestino del sistema operativo. Da
//!   Setaccio non si torna indietro, ma il file esiste ancora e si recupera
//!   dal gestore file. È l'operazione di tutti i giorni.
//! - **elimina** chiama `remove_file`. Non c'è recupero, da nessuna parte.
//!   Esiste perché su un disco pieno il cestino non libera niente, ma va
//!   chiesta esplicitamente ogni volta.
//!
//! Resta invece uguale a tutto il resto la struttura: si costruisce un piano,
//! l'utente lo vede, e solo dopo si esegue. Un piano non tocca mai il disco.

use anyhow::{bail, Context, Result};
use rusqlite::params;
use std::path::{Path, PathBuf};

use crate::db::Db;
use crate::organize::{assembla, nuovo_batch};
use crate::types::{FileRecord, Mossa, PianoOperazioni};

/// Genere di mossa che consegna il file al cestino di sistema.
pub const GENERE_CESTINO: &str = "cestino";

/// Genere di mossa che cancella il file senza passare dal cestino.
pub const GENERE_ELIMINA: &str = "elimina";

/// Vero per i generi da cui non si torna indietro con l'undo di Setaccio.
pub fn irreversibile(genere: &str) -> bool {
    genere == GENERE_CESTINO || genere == GENERE_ELIMINA
}

// ---------------------------------------------------------------------------
// Piani
// ---------------------------------------------------------------------------

/// Piano per mandare nel cestino di sistema i file indicati.
pub fn piano_cestino(db: &Db, file_ids: &[i64]) -> Result<PianoOperazioni> {
    piano(db, file_ids, GENERE_CESTINO)
}

/// Piano per cancellare definitivamente i file indicati.
pub fn piano_elimina(db: &Db, file_ids: &[i64]) -> Result<PianoOperazioni> {
    piano(db, file_ids, GENERE_ELIMINA)
}

fn piano(db: &Db, file_ids: &[i64], genere: &str) -> Result<PianoOperazioni> {
    let mut mosse = Vec::with_capacity(file_ids.len());
    let mut dimensioni = Vec::with_capacity(file_ids.len());
    // Lo stesso id due volte nella selezione produrrebbe due mosse, la seconda
    // delle quali fallirebbe a metà esecuzione su un file già sparito. Meglio
    // che si veda nel piano.
    let mut gia_viste: Vec<i64> = Vec::new();

    for &id in file_ids {
        let Some(f) = db.file_per_id(id)? else {
            mosse.push(non_eseguibile(
                id,
                String::new(),
                genere,
                "richiesto dall'utente",
                format!("il file {id} non è più nell'indice"),
            ));
            continue;
        };
        dimensioni.push((id, f.size));

        if gia_viste.contains(&id) {
            mosse.push(non_eseguibile(
                id,
                f.path.clone(),
                genere,
                &motivo(&f),
                "lo stesso file compare due volte nella selezione".into(),
            ));
            continue;
        }

        // Un file indicizzato dall'interno di uno zip non ha un percorso
        // proprio sul disco: cancellarlo vorrebbe dire riscrivere l'archivio,
        // che è un'altra operazione e non si fa di soppiatto dentro questa.
        // Il controllo viene prima di quello sull'esistenza perché quel
        // percorso non esisterà mai, e «non esiste più» sarebbe una risposta
        // esatta ma inutile.
        if let Some(archivio) = f.archivio_padre.as_deref() {
            mosse.push(non_eseguibile(
                id,
                f.path.clone(),
                genere,
                &motivo(&f),
                format!("sta dentro l'archivio «{archivio}»: va tolto da lì, non dal disco"),
            ));
            continue;
        }

        let origine = PathBuf::from(&f.path);
        if !origine.exists() {
            mosse.push(non_eseguibile(
                id,
                f.path.clone(),
                genere,
                &motivo(&f),
                "l'origine non esiste più sul disco".into(),
            ));
            continue;
        }
        if origine.is_dir() {
            mosse.push(non_eseguibile(
                id,
                f.path.clone(),
                genere,
                &motivo(&f),
                "è una cartella, non un file: qui si tolgono solo file".into(),
            ));
            continue;
        }

        gia_viste.push(id);
        mosse.push(Mossa {
            file_id: id,
            origine: f.path.clone(),
            destinazione: String::new(),
            genere: genere.to_string(),
            motivo: motivo(&f),
            eseguibile: true,
            avviso: None,
        });
    }

    Ok(assembla(nuovo_batch(db)?, mosse, &dimensioni))
}

fn non_eseguibile(
    file_id: i64,
    origine: String,
    genere: &str,
    motivo: &str,
    avviso: String,
) -> Mossa {
    Mossa {
        file_id,
        origine,
        destinazione: String::new(),
        genere: genere.to_string(),
        motivo: motivo.to_string(),
        eseguibile: false,
        avviso: Some(avviso),
    }
}

/// Perché questo file è nel piano. Il motivo si legge dall'indice: chi
/// rilegge il piano deve poter capire cosa sta approvando senza fidarsi.
fn motivo(f: &FileRecord) -> String {
    use crate::types::Stato;
    match f.stato {
        Stato::Duplicato => "duplicato di un file già presente".into(),
        Stato::Orfano => "orfano: il contenuto esiste già estratto accanto".into(),
        Stato::Canonico => format!(
            "{} · scelto dall'utente",
            crate::types::Tipo::as_str(&f.tipo)
        ),
    }
}

// ---------------------------------------------------------------------------
// Esecuzione
// ---------------------------------------------------------------------------

/// Consegna il file al cestino di sistema e aggiorna indice e journal.
///
/// Il percorso del file dentro il cestino non viene registrato: dipende dal
/// sistema, cambia se l'utente ci mette mano, e fingere di conoscerlo
/// vorrebbe dire offrire un undo che a volte non funziona. La riga di journal
/// serve a dire *cosa è stato tolto e quando*, non a rimetterlo a posto.
pub fn nel_cestino(db: &Db, batch: &str, m: &Mossa) -> Result<()> {
    let origine = Path::new(&m.origine);
    if !origine.exists() {
        bail!("non esiste più");
    }
    trash::delete(origine).with_context(|| format!("«{}» nel cestino", m.origine))?;
    registra(db, batch, m)
}

/// Cancella il file. Nessun recupero possibile, da nessuna parte.
pub fn elimina_definitivo(db: &Db, batch: &str, m: &Mossa) -> Result<()> {
    let origine = Path::new(&m.origine);
    if !origine.exists() {
        bail!("non esiste più");
    }
    // `remove_file` e non `remove_dir_all`: se il percorso indicizzato è
    // diventato una cartella nel frattempo, questa chiamata fallisce invece
    // di portarsi via l'albero che c'è sotto.
    std::fs::remove_file(origine).with_context(|| format!("cancellazione di «{}»", m.origine))?;
    registra(db, batch, m)
}

/// Scrive la riga di journal e toglie il file dall'indice.
///
/// Nell'ordine: prima il journal, poi l'indice. Il file a questo punto non c'è
/// già più, e se qualcosa andasse storto fra le due scritture è meglio avere
/// una traccia di troppo che un'operazione senza traccia.
fn registra(db: &Db, batch: &str, m: &Mossa) -> Result<()> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO operazione (batch, genere, origine, destinazione, file_id)
         VALUES (?1, ?2, ?3, '', ?4)",
        params![batch, m.genere, m.origine, m.file_id],
    )?;
    if m.file_id > 0 {
        // Il file non è più sul disco: lasciarlo nell'indice lo farebbe
        // ricomparire nelle ricerche e nei conti dello spazio occupato.
        //
        // Le tabelle FTS non hanno chiavi esterne, quindi vanno svuotate a
        // mano e **prima** di `file`: `record_tracciato` scende per
        // `ON DELETE CASCADE`, e dopo non ci sarebbe più modo di sapere quali
        // righe di `record_fts` appartenevano a questo file.
        conn.execute(
            "DELETE FROM record_fts
              WHERE rowid IN (SELECT id FROM record_tracciato WHERE file_id = ?1)",
            params![m.file_id],
        )?;
        conn.execute("DELETE FROM file_fts WHERE rowid = ?1", params![m.file_id])?;
        conn.execute("DELETE FROM file WHERE id = ?1", params![m.file_id])?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use super::*;
    use crate::db::FileVisto;
    use crate::types::{Fascia, Tipo};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static CONTATORE: AtomicUsize = AtomicUsize::new(0);

    fn temp(nome: &str) -> PathBuf {
        let n = CONTATORE.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!(
            "setaccio-cestino-{}-{}-{}",
            std::process::id(),
            nome,
            n
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn indicizza(db: &Db, sid: i64, path: &Path) -> i64 {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"contenuto di prova").unwrap();
        let (id, _) = db
            .file_upsert(&FileVisto {
                path: path.to_string_lossy().into_owned(),
                nome: path.file_name().unwrap().to_string_lossy().into_owned(),
                ext: path.extension().map(|e| e.to_string_lossy().into_owned()),
                size: 18,
                mtime: 1,
                tipo: Tipo::Documento,
                motivo_tipo: "test".into(),
                contesto: None,
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
    fn il_piano_non_tocca_niente_e_l_esecuzione_cancella() {
        let base = temp("elimina");
        let file = base.join("dentro/vecchio.iso");
        let (db, sid) = db_e_sorgente(&base);
        let id = indicizza(&db, sid, &file);

        let piano = piano_elimina(&db, &[id]).unwrap();
        assert_eq!(piano.eseguibili, 1);
        assert_eq!(piano.spazio_liberato, 18);
        assert_eq!(piano.mosse[0].genere, GENERE_ELIMINA);
        // Il piano da solo non fa niente.
        assert!(file.exists(), "il piano non deve toccare il disco");

        let esito = crate::organize::esegui(&db, &piano).unwrap();
        assert_eq!(esito.eseguite, 1, "errori: {:?}", esito.errori);
        assert!(!file.exists());
        // Il file sparisce anche dall'indice: non deve tornare nelle ricerche.
        assert!(db.file_per_id(id).unwrap().is_none());
    }

    /// Il giro vero contro il cestino del sistema su cui gira il test.
    ///
    /// Non è un test del crate `trash`: è la verifica che la consegna
    /// funzioni *qui*, che la riga di journal resti come traccia e che
    /// l'annulla dica di no invece di provarci e lasciare un batch marcato
    /// come disfatto quando non lo è.
    #[test]
    fn il_cestino_toglie_dal_disco_e_l_annulla_dice_dove_cercare() {
        let base = temp("cestino");
        let file = base.join("da-buttare.txt");
        let (db, sid) = db_e_sorgente(&base);
        let id = indicizza(&db, sid, &file);

        let piano = piano_cestino(&db, &[id]).unwrap();
        assert_eq!(piano.eseguibili, 1);
        assert!(file.exists(), "il piano non deve toccare il disco");

        let esito = crate::organize::esegui(&db, &piano).unwrap();
        assert_eq!(esito.eseguite, 1, "errori: {:?}", esito.errori);
        assert!(!file.exists(), "il file deve aver lasciato il suo posto");
        assert!(db.file_per_id(id).unwrap().is_none());

        // Il batch resta come traccia, ma dichiarato non annullabile.
        let batch = crate::organize::batch_recenti(&db).unwrap();
        assert_eq!(batch[0].genere, GENERE_CESTINO);
        assert!(!batch[0].annullabile);
        assert!(!batch[0].annullato);

        // E chi ci prova lo stesso si sente dire dove andare a cercare.
        let undo = crate::organize::annulla(&db, &esito.batch).unwrap();
        assert_eq!(undo.eseguite, 0);
        assert_eq!(undo.fallite, 1);
        assert!(undo.errori[0].contains("cestino di sistema"));
        // Il rifiuto non deve marcare la riga come annullata.
        assert!(!crate::organize::batch_recenti(&db).unwrap()[0].annullato);
    }

    #[test]
    fn un_file_gia_sparito_e_una_mossa_saltata_non_un_errore() {
        let base = temp("sparito");
        let file = base.join("mai-esistito.bin");
        let (db, sid) = db_e_sorgente(&base);
        let id = indicizza(&db, sid, &file);
        std::fs::remove_file(&file).unwrap();

        let piano = piano_elimina(&db, &[id]).unwrap();
        assert_eq!(piano.eseguibili, 0);
        assert_eq!(piano.saltate, 1);
        assert!(piano.mosse[0].avviso.as_deref().unwrap().contains("non esiste più"));
    }

    #[test]
    fn lo_stesso_file_due_volte_produce_una_sola_mossa_eseguibile() {
        let base = temp("doppio");
        let file = base.join("uno.pdf");
        let (db, sid) = db_e_sorgente(&base);
        let id = indicizza(&db, sid, &file);

        let piano = piano_cestino(&db, &[id, id]).unwrap();
        assert_eq!(piano.eseguibili, 1);
        assert_eq!(piano.saltate, 1);
    }

    #[test]
    fn quello_che_sta_dentro_un_archivio_non_si_cancella_dal_disco() {
        let base = temp("archivio");
        let zip = base.join("pacco.zip");
        std::fs::write(&zip, b"finto zip").unwrap();
        let (db, sid) = db_e_sorgente(&base);
        let (id, _) = db
            .file_upsert(&FileVisto {
                path: zip.join("dentro/nota.txt").to_string_lossy().into_owned(),
                nome: "nota.txt".into(),
                ext: Some("txt".into()),
                size: 10,
                mtime: 1,
                tipo: Tipo::Documento,
                motivo_tipo: "test".into(),
                contesto: None,
                motivo_contesto: None,
                sorgente_id: sid,
                archivio_padre: Some(zip.to_string_lossy().into_owned()),
                lotto: None,
            })
            .unwrap();

        let piano = piano_elimina(&db, &[id]).unwrap();
        assert_eq!(piano.eseguibili, 0);
        assert!(piano.mosse[0].avviso.as_deref().unwrap().contains("archivio"));
        assert!(zip.exists(), "l'archivio contenitore non si tocca");
    }
}
