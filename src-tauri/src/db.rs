//! Indice SQLite + FTS5.
//!
//! Tutto vive in un solo file (`setaccio.db` nella data dir dell'utente) così
//! che l'indice sia ispezionabile a mano con `sqlite3` e cancellabile senza
//! lasciare residui. Nessuna categoria viene mai scritta sui file: l'ordine
//! vive qui dentro.

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use crate::types::*;

pub struct Db {
    conn: Mutex<Connection>,
}

/// Percorso del database: `~/.local/share/setaccio/setaccio.db` su Linux.
pub fn percorso_db() -> Result<PathBuf> {
    let dir = dirs::data_dir()
        .context("impossibile determinare la data dir dell'utente")?
        .join("setaccio");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("setaccio.db"))
}

impl Db {
    pub fn apri(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        // WAL: la scansione scrive mentre la UI legge senza bloccarsi.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let db = Db {
            conn: Mutex::new(conn),
        };
        db.migra()?;
        Ok(db)
    }

    pub fn in_memoria() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let db = Db {
            conn: Mutex::new(conn),
        };
        db.migra()?;
        Ok(db)
    }

    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        // Un mutex avvelenato significa che un thread di scansione è andato in
        // panic: preferiamo continuare con la connessione piuttosto che far
        // cadere tutta la UI.
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn migra(&self) -> Result<()> {
        let conn = self.conn();
        conn.execute_batch(SCHEMA)?;
        let versione: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap_or(0);
        if versione < 1 {
            conn.execute_batch(REGOLE_DI_SERIE)?;
            conn.pragma_update(None, "user_version", 1)?;
        }
        Ok(())
    }
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sorgenti (
  id                INTEGER PRIMARY KEY,
  path              TEXT NOT NULL UNIQUE,
  fascia            TEXT NOT NULL DEFAULT 'documenti',
  attiva            INTEGER NOT NULL DEFAULT 1,
  ricorsiva         INTEGER NOT NULL DEFAULT 1,
  ignora_repo_guard INTEGER NOT NULL DEFAULT 0,
  creata_il         INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);

CREATE TABLE IF NOT EXISTS file (
  id              INTEGER PRIMARY KEY,
  path            TEXT NOT NULL UNIQUE,
  nome            TEXT NOT NULL,
  ext             TEXT,
  size            INTEGER NOT NULL DEFAULT 0,
  mtime           INTEGER NOT NULL DEFAULT 0,
  hash            TEXT,
  tipo            TEXT NOT NULL DEFAULT 'altro',
  contesto        TEXT,
  stato           TEXT NOT NULL DEFAULT 'canonico',
  motivo_tipo     TEXT,
  motivo_contesto TEXT,
  sorgente_id     INTEGER REFERENCES sorgenti(id) ON DELETE CASCADE,
  archivio_padre  TEXT,
  lotto           TEXT,
  testo_estratto  INTEGER NOT NULL DEFAULT 0,
  pagine          INTEGER,
  -- Offset di inizio di ogni pagina dentro il testo estratto, in JSON. Senza
  -- questo un match si può solo attribuire al file, non alla pagina.
  offset_pagine   TEXT,
  visto_il        INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);

CREATE INDEX IF NOT EXISTS idx_file_tipo      ON file(tipo);
CREATE INDEX IF NOT EXISTS idx_file_contesto  ON file(contesto);
CREATE INDEX IF NOT EXISTS idx_file_stato     ON file(stato);
CREATE INDEX IF NOT EXISTS idx_file_hash      ON file(hash);
CREATE INDEX IF NOT EXISTS idx_file_lotto     ON file(lotto);
CREATE INDEX IF NOT EXISTS idx_file_sorgente  ON file(sorgente_id);
CREATE INDEX IF NOT EXISTS idx_file_mtime     ON file(mtime);

-- Il testo sta dentro la tabella FTS (non contentless) perché serve a
-- snippet() per restituire il frammento evidenziato.
CREATE VIRTUAL TABLE IF NOT EXISTS file_fts USING fts5(
  nome,
  testo,
  tokenize = "unicode61 remove_diacritics 2"
);

-- Record dei tracciati a lunghezza fissa. Popolata solo sotto la soglia di
-- dimensione configurata, per non far esplodere l'indice.
CREATE TABLE IF NOT EXISTS record_tracciato (
  id          INTEGER PRIMARY KEY,
  file_id     INTEGER NOT NULL REFERENCES file(id) ON DELETE CASCADE,
  numero_riga INTEGER NOT NULL,
  contenuto   TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_record_file ON record_tracciato(file_id);

CREATE VIRTUAL TABLE IF NOT EXISTS record_fts USING fts5(
  contenuto,
  tokenize = "unicode61 remove_diacritics 2"
);

CREATE TABLE IF NOT EXISTS layout (
  id                INTEGER PRIMARY KEY,
  nome              TEXT NOT NULL UNIQUE,
  lunghezza_record  INTEGER NOT NULL,
  campi             TEXT NOT NULL,
  creato_il         INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);

CREATE TABLE IF NOT EXISTS lotto (
  codice    TEXT PRIMARY KEY,
  cartella  TEXT NOT NULL,
  layout_id INTEGER REFERENCES layout(id) ON DELETE SET NULL,
  visto_il  INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);

CREATE TABLE IF NOT EXISTS regola (
  id        INTEGER PRIMARY KEY,
  nome      TEXT NOT NULL,
  asse      TEXT NOT NULL,
  pattern   TEXT NOT NULL,
  valore    TEXT NOT NULL,
  priorita  INTEGER NOT NULL DEFAULT 100,
  attiva    INTEGER NOT NULL DEFAULT 1,
  builtin   INTEGER NOT NULL DEFAULT 0
);

-- Journal delle operazioni sul filesystem: senza questo non esiste undo, e
-- senza undo la modalita Organizza non e attivabile in sicurezza.
CREATE TABLE IF NOT EXISTS operazione (
  id            INTEGER PRIMARY KEY,
  batch         TEXT NOT NULL,
  genere        TEXT NOT NULL,
  origine       TEXT NOT NULL,
  destinazione  TEXT NOT NULL,
  file_id       INTEGER,
  eseguita_il   INTEGER NOT NULL DEFAULT (strftime('%s','now')),
  annullata     INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_operazione_batch ON operazione(batch);

CREATE TABLE IF NOT EXISTS impostazioni (
  chiave  TEXT PRIMARY KEY,
  valore  TEXT NOT NULL
);
"#;

/// Regole di serie ricavate dall'analisi del filesystem reale. Sono `builtin`:
/// disattivabili ma non cancellabili, così un utente non si ritrova senza
/// nessuna classificazione dopo una pulizia distratta.
const REGOLE_DI_SERIE: &str = r#"
INSERT INTO regola (nome, asse, pattern, valore, priorita, builtin) VALUES
  ('Lotti ILC',            'contesto', '**/T1Q*',                  'lavoro-ILC',        10, 1),
  ('Lotti numerici',       'contesto', '**/3[0-9][0-9][0-9][0-9][0-9]*', 'lavoro-ILC',  15, 1),
  ('Policy aziendali',     'contesto', 'BDOC_*',                   'lavoro-policy',     20, 1),
  ('Documenti BeyonDoc',   'contesto', 'BeyonDoc*',                'lavoro-BeyonDoc',   20, 1),
  ('Curriculum',           'contesto', 'CV_*',                     'personale',         20, 1),
  ('Libri e studio',       'contesto', '**/libri/**',              'studio',            30, 1),
  ('Kindle',               'contesto', '**/kindle/**',             'studio',            30, 1),
  ('Documenti personali',  'contesto', '**/russus_doc/**',         'personale',         30, 1),
  ('RED',                  'contesto', '**/RED/**',                'lavoro-RED',        30, 1),
  ('DevOps',               'contesto', '**/DevOps/**',             'lavoro-DevOps',     30, 1),
  ('PAM',                  'contesto', '**/PAM/**',                'lavoro-PAM',        30, 1);
"#;

// ---------------------------------------------------------------------------
// Sorgenti
// ---------------------------------------------------------------------------

impl Db {
    pub fn sorgenti(&self) -> Result<Vec<Sorgente>> {
        let conn = self.conn();
        let mut st = conn.prepare(
            "SELECT id, path, fascia, attiva, ricorsiva, ignora_repo_guard
               FROM sorgenti ORDER BY path",
        )?;
        let righe = st
            .query_map([], |r| {
                Ok(Sorgente {
                    id: r.get(0)?,
                    path: r.get(1)?,
                    fascia: Fascia::from_str(&r.get::<_, String>(2)?),
                    attiva: r.get::<_, i64>(3)? != 0,
                    ricorsiva: r.get::<_, i64>(4)? != 0,
                    ignora_repo_guard: r.get::<_, i64>(5)? != 0,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(righe)
    }

    pub fn sorgente_aggiungi(&self, path: &str, fascia: Fascia) -> Result<i64> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO sorgenti (path, fascia) VALUES (?1, ?2)
             ON CONFLICT(path) DO UPDATE SET fascia = excluded.fascia, attiva = 1",
            params![path, fascia.as_str()],
        )?;
        let id = conn.query_row(
            "SELECT id FROM sorgenti WHERE path = ?1",
            params![path],
            |r| r.get(0),
        )?;
        Ok(id)
    }

    pub fn sorgente_rimuovi(&self, id: i64) -> Result<()> {
        let conn = self.conn();
        // I file vanno via in cascata; l'FTS non ha foreign key, va ripulito a mano.
        conn.execute(
            "DELETE FROM file_fts WHERE rowid IN (SELECT id FROM file WHERE sorgente_id = ?1)",
            params![id],
        )?;
        conn.execute("DELETE FROM sorgenti WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn sorgente_aggiorna(&self, s: &Sorgente) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE sorgenti SET fascia = ?2, attiva = ?3, ricorsiva = ?4, ignora_repo_guard = ?5
             WHERE id = ?1",
            params![
                s.id,
                s.fascia.as_str(),
                s.attiva as i64,
                s.ricorsiva as i64,
                s.ignora_repo_guard as i64
            ],
        )?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// File
// ---------------------------------------------------------------------------

/// Ciò che il walker sa di un file prima che venga estratto il testo.
pub struct FileVisto {
    pub path: String,
    pub nome: String,
    pub ext: Option<String>,
    pub size: i64,
    pub mtime: i64,
    pub tipo: Tipo,
    pub motivo_tipo: String,
    pub contesto: Option<String>,
    pub motivo_contesto: Option<String>,
    pub sorgente_id: i64,
    pub archivio_padre: Option<String>,
    pub lotto: Option<String>,
}

/// Converte offset in byte nei corrispondenti offset in caratteri, con una
/// sola passata sul testo. Gli offset che cadono in mezzo a un carattere
/// multibyte vengono attribuiti al carattere che li contiene.
pub fn byte_in_caratteri(testo: &str, offset_byte: &[usize]) -> Vec<usize> {
    let mut ordinati: Vec<(usize, usize)> = offset_byte.iter().copied().zip(0..).collect();
    ordinati.sort_unstable();

    let mut esito = vec![0usize; offset_byte.len()];
    let mut prossimo = 0usize;
    let mut caratteri = 0usize;

    for (b, _) in testo.char_indices() {
        while prossimo < ordinati.len() && ordinati[prossimo].0 <= b {
            esito[ordinati[prossimo].1] = caratteri;
            prossimo += 1;
        }
        caratteri += 1;
    }
    // Gli offset oltre la fine del testo collassano sulla lunghezza totale.
    while prossimo < ordinati.len() {
        esito[ordinati[prossimo].1] = caratteri;
        prossimo += 1;
    }
    esito
}

pub fn riga_in_file(r: &Row) -> rusqlite::Result<FileRecord> {
    Ok(FileRecord {
        id: r.get("id")?,
        path: r.get("path")?,
        nome: r.get("nome")?,
        ext: r.get("ext")?,
        size: r.get("size")?,
        mtime: r.get("mtime")?,
        hash: r.get("hash")?,
        tipo: Tipo::from_str(&r.get::<_, String>("tipo")?),
        contesto: r.get("contesto")?,
        stato: Stato::from_str(&r.get::<_, String>("stato")?),
        motivo_tipo: r.get("motivo_tipo")?,
        motivo_contesto: r.get("motivo_contesto")?,
        sorgente_id: r.get::<_, Option<i64>>("sorgente_id")?.unwrap_or(0),
        archivio_padre: r.get("archivio_padre")?,
        lotto: r.get("lotto")?,
        testo_estratto: r.get::<_, i64>("testo_estratto")? != 0,
        pagine: r.get("pagine")?,
    })
}

/// Le colonne che `riga_in_file` si aspetta, in un posto solo: ogni modulo che
/// costruisce una SELECT su `file` deve usare questa costante, altrimenti al
/// primo campo aggiunto le copie divergono e la lettura fallisce a runtime.
pub const COLONNE_FILE: &str = "id, path, nome, ext, size, mtime, hash, tipo, contesto, stato,
     motivo_tipo, motivo_contesto, sorgente_id, archivio_padre, lotto, testo_estratto, pagine";

impl Db {
    /// Inserisce o aggiorna un file. Ritorna `(id, cambiato)`: `cambiato` è
    /// falso se dimensione e mtime coincidono con quanto già indicizzato, ed è
    /// ciò che rende incrementale la scansione — il chiamante può saltare
    /// l'estrazione del testo.
    pub fn file_upsert(&self, f: &FileVisto) -> Result<(i64, bool)> {
        let conn = self.conn();
        let esistente: Option<(i64, i64, i64)> = conn
            .query_row(
                "SELECT id, size, mtime FROM file WHERE path = ?1",
                params![f.path],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;

        if let Some((id, size, mtime)) = esistente {
            let invariato = size == f.size && mtime == f.mtime;
            conn.execute(
                "UPDATE file SET nome=?2, ext=?3, size=?4, mtime=?5, tipo=?6, motivo_tipo=?7,
                        contesto=?8, motivo_contesto=?9, sorgente_id=?10, archivio_padre=?11,
                        lotto=?12, visto_il=strftime('%s','now')
                 WHERE id=?1",
                params![
                    id,
                    f.nome,
                    f.ext,
                    f.size,
                    f.mtime,
                    f.tipo.as_str(),
                    f.motivo_tipo,
                    f.contesto,
                    f.motivo_contesto,
                    f.sorgente_id,
                    f.archivio_padre,
                    f.lotto
                ],
            )?;
            if !invariato {
                // Contenuto cambiato: hash e testo vanno rifatti.
                conn.execute(
                    "UPDATE file SET hash=NULL, testo_estratto=0 WHERE id=?1",
                    params![id],
                )?;
            }
            Ok((id, !invariato))
        } else {
            conn.execute(
                "INSERT INTO file (path, nome, ext, size, mtime, tipo, motivo_tipo, contesto,
                                   motivo_contesto, sorgente_id, archivio_padre, lotto)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                params![
                    f.path,
                    f.nome,
                    f.ext,
                    f.size,
                    f.mtime,
                    f.tipo.as_str(),
                    f.motivo_tipo,
                    f.contesto,
                    f.motivo_contesto,
                    f.sorgente_id,
                    f.archivio_padre,
                    f.lotto
                ],
            )?;
            Ok((conn.last_insert_rowid(), true))
        }
    }

    /// Salva il testo estratto e lo rende cercabile.
    pub fn testo_salva(&self, file_id: i64, nome: &str, testo: &str, pagine: Option<i64>) -> Result<()> {
        let conn = self.conn();
        conn.execute("DELETE FROM file_fts WHERE rowid = ?1", params![file_id])?;
        conn.execute(
            "INSERT INTO file_fts (rowid, nome, testo) VALUES (?1, ?2, ?3)",
            params![file_id, nome, testo],
        )?;
        conn.execute(
            "UPDATE file SET testo_estratto = 1, pagine = ?2 WHERE id = ?1",
            params![file_id, pagine],
        )?;
        Ok(())
    }

    /// Registra dove inizia ogni pagina dentro il testo estratto, così la
    /// ricerca può rispondere «pagina 3» invece del solo nome file.
    ///
    /// Gli estrattori producono offset in **byte** (`String::len()`), ma la
    /// ricerca li confronta con `instr` di SQLite, che conta **caratteri**.
    /// La conversione avviene qui, una volta sola, perché mescolare le due
    /// unità produrrebbe numeri di pagina sbagliati su ogni testo accentato —
    /// cioè su tutti quelli italiani.
    pub fn offset_pagine_salva(&self, file_id: i64, testo: &str, offset_byte: &[usize]) -> Result<()> {
        if offset_byte.is_empty() {
            return Ok(());
        }
        let in_caratteri = byte_in_caratteri(testo, offset_byte);
        let conn = self.conn();
        let json = serde_json::to_string(&in_caratteri)?;
        conn.execute(
            "UPDATE file SET offset_pagine = ?2 WHERE id = ?1",
            params![file_id, json],
        )?;
        Ok(())
    }

    pub fn hash_salva(&self, file_id: i64, hash: &str) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE file SET hash = ?2 WHERE id = ?1",
            params![file_id, hash],
        )?;
        Ok(())
    }

    pub fn file_per_id(&self, id: i64) -> Result<Option<FileRecord>> {
        let conn = self.conn();
        let sql = format!("SELECT {COLONNE_FILE} FROM file WHERE id = ?1");
        let r = conn
            .query_row(&sql, params![id], riga_in_file)
            .optional()?;
        Ok(r)
    }

    pub fn file_per_path(&self, path: &str) -> Result<Option<FileRecord>> {
        let conn = self.conn();
        let sql = format!("SELECT {COLONNE_FILE} FROM file WHERE path = ?1");
        let r = conn
            .query_row(&sql, params![path], riga_in_file)
            .optional()?;
        Ok(r)
    }

    /// Rimuove dall'indice i file spariti dal disco sotto una sorgente.
    pub fn pota_spariti(&self, sorgente_id: i64) -> Result<usize> {
        let conn = self.conn();
        let mut st = conn.prepare("SELECT id, path FROM file WHERE sorgente_id = ?1")?;
        let candidati: Vec<(i64, String)> = st
            .query_map(params![sorgente_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(st);
        let mut tolti = 0;
        for (id, path) in candidati {
            // I file interni a un archivio non esistono sul filesystem: si
            // controlla l'archivio contenitore, non il path virtuale.
            let vero: Option<String> = conn
                .query_row(
                    "SELECT archivio_padre FROM file WHERE id = ?1",
                    params![id],
                    |r| r.get(0),
                )
                .optional()?
                .flatten();
            let da_verificare = vero.unwrap_or(path);
            if !Path::new(&da_verificare).exists() {
                conn.execute("DELETE FROM file_fts WHERE rowid = ?1", params![id])?;
                conn.execute("DELETE FROM file WHERE id = ?1", params![id])?;
                tolti += 1;
            }
        }
        Ok(tolti)
    }

    pub fn imposta(&self, chiave: &str, valore: &str) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO impostazioni (chiave, valore) VALUES (?1, ?2)
             ON CONFLICT(chiave) DO UPDATE SET valore = excluded.valore",
            params![chiave, valore],
        )?;
        Ok(())
    }

    pub fn leggi(&self, chiave: &str) -> Result<Option<String>> {
        let conn = self.conn();
        let v = conn
            .query_row(
                "SELECT valore FROM impostazioni WHERE chiave = ?1",
                params![chiave],
                |r| r.get(0),
            )
            .optional()?;
        Ok(v)
    }
}

// ---------------------------------------------------------------------------
// Regole
// ---------------------------------------------------------------------------

impl Db {
    pub fn regole(&self) -> Result<Vec<Regola>> {
        let conn = self.conn();
        let mut st = conn.prepare(
            "SELECT id, nome, asse, pattern, valore, priorita, attiva, builtin
               FROM regola ORDER BY priorita, id",
        )?;
        let v = st
            .query_map([], |r| {
                Ok(Regola {
                    id: r.get(0)?,
                    nome: r.get(1)?,
                    asse: r.get(2)?,
                    pattern: r.get(3)?,
                    valore: r.get(4)?,
                    priorita: r.get(5)?,
                    attiva: r.get::<_, i64>(6)? != 0,
                    builtin: r.get::<_, i64>(7)? != 0,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    pub fn regola_aggiungi(&self, nome: &str, asse: &str, pattern: &str, valore: &str, priorita: i64) -> Result<i64> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO regola (nome, asse, pattern, valore, priorita) VALUES (?1,?2,?3,?4,?5)",
            params![nome, asse, pattern, valore, priorita],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn regola_elimina(&self, id: i64) -> Result<()> {
        let conn = self.conn();
        conn.execute("DELETE FROM regola WHERE id = ?1 AND builtin = 0", params![id])?;
        Ok(())
    }

    pub fn regola_attiva(&self, id: i64, attiva: bool) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE regola SET attiva = ?2 WHERE id = ?1",
            params![id, attiva as i64],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn migrazione_e_regole_di_serie() {
        let db = Db::in_memoria().unwrap();
        let regole = db.regole().unwrap();
        assert!(!regole.is_empty(), "le regole builtin devono essere caricate");
        assert!(regole.iter().all(|r| r.builtin));
    }

    #[test]
    fn gli_offset_si_convertono_da_byte_a_caratteri() {
        // "però" = 4 caratteri, 5 byte: la 'ò' occupa i byte 3 e 4.
        let testo = "però\u{c}dopo";
        // Byte 0 = inizio; byte 6 = subito dopo il form feed (pagina 2).
        assert_eq!(byte_in_caratteri(testo, &[0, 6]), vec![0, 5]);
        // Un offset oltre la fine collassa sulla lunghezza in caratteri.
        assert_eq!(byte_in_caratteri(testo, &[999]), vec![9]);
        // L'ordine di uscita segue quello di ingresso, non quello ordinato.
        assert_eq!(byte_in_caratteri(testo, &[6, 0]), vec![5, 0]);
        assert!(byte_in_caratteri(testo, &[]).is_empty());
    }

    #[test]
    fn upsert_segnala_solo_i_cambiamenti_veri() {
        let db = Db::in_memoria().unwrap();
        let id_sorg = db.sorgente_aggiungi("/tmp/x", Fascia::Documenti).unwrap();
        let f = FileVisto {
            path: "/tmp/x/a.pdf".into(),
            nome: "a.pdf".into(),
            ext: Some("pdf".into()),
            size: 10,
            mtime: 100,
            tipo: Tipo::Documento,
            motivo_tipo: "estensione .pdf".into(),
            contesto: None,
            motivo_contesto: None,
            sorgente_id: id_sorg,
            archivio_padre: None,
            lotto: None,
        };
        let (id, cambiato) = db.file_upsert(&f).unwrap();
        assert!(cambiato, "il primo inserimento è sempre un cambiamento");

        let (id2, cambiato2) = db.file_upsert(&f).unwrap();
        assert_eq!(id, id2);
        assert!(!cambiato2, "stesso size+mtime: niente da riestrarre");

        let f2 = FileVisto { mtime: 200, ..f };
        let (_, cambiato3) = db.file_upsert(&f2).unwrap();
        assert!(cambiato3, "mtime diverso: il contenuto va rifatto");
    }
}
