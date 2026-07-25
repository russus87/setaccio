//! Ricerca, faccette, statistiche e anteprime.
//!
//! La ricerca deve rispondere *dove* sta il match, non solo *in quale file*:
//! la riga del tracciato e la pagina del PDF sono metà del valore. Un elenco
//! di path lo dà già `locate`.

use anyhow::Result;
use rusqlite::{types::Value, OptionalExtension, ToSql};

use crate::db::{riga_in_file, Db};
use crate::types::{
    Anteprima, ConteggioEtichetta, FileRecord, Filtri, Risultato, Statistiche, Tipo,
};

const COLONNE: &str = "f.id, f.path, f.nome, f.ext, f.size, f.mtime, f.hash, f.tipo, f.contesto,
     f.stato, f.motivo_tipo, f.motivo_contesto, f.sorgente_id, f.archivio_padre, f.lotto,
     f.testo_estratto, f.pagine";

/// Traduce quello che l'utente ha digitato in una query FTS5 valida.
///
/// Senza questa normalizzazione basta un apice o una parentesi per far
/// fallire l'intera query con un errore di sintassi incomprensibile: ogni
/// termine viene quindi citato, e il carattere jolly finale viene preservato
/// perché è l'unica sintassi che vale la pena esporre.
fn query_fts(q: &str) -> Option<String> {
    let termini: Vec<String> = q
        .split_whitespace()
        .filter(|t| !t.trim().is_empty())
        .map(|t| {
            let prefisso = t.ends_with('*');
            let nudo = t.trim_end_matches('*').replace('"', "\"\"");
            if prefisso {
                format!("\"{nudo}\"*")
            } else {
                format!("\"{nudo}\"")
            }
        })
        .filter(|t| t != "\"\"" && t != "\"\"*")
        .collect();
    if termini.is_empty() {
        None
    } else {
        Some(termini.join(" AND "))
    }
}

/// Costruisce la parte `WHERE` comune ai filtri componibili.
pub(crate) fn clausole(filtri: &Filtri, cond: &mut Vec<String>, args: &mut Vec<Value>) {
    let lista = |campo: &str, valori: &[String], cond: &mut Vec<String>, args: &mut Vec<Value>| {
        if !valori.is_empty() {
            let segnaposti = vec!["?"; valori.len()].join(",");
            cond.push(format!("{campo} IN ({segnaposti})"));
            for v in valori {
                args.push(Value::Text(v.clone()));
            }
        }
    };

    lista("f.tipo", &filtri.tipi, cond, args);
    lista("f.stato", &filtri.stati, cond, args);
    lista("f.contesto", &filtri.contesti, cond, args);
    lista("f.ext", &filtri.estensioni, cond, args);

    if !filtri.sorgenti.is_empty() {
        let segnaposti = vec!["?"; filtri.sorgenti.len()].join(",");
        cond.push(format!("f.sorgente_id IN ({segnaposti})"));
        for v in &filtri.sorgenti {
            args.push(Value::Integer(*v));
        }
    }
    if let Some(da) = filtri.da_data {
        cond.push("f.mtime >= ?".into());
        args.push(Value::Integer(da));
    }
    if let Some(a) = filtri.a_data {
        cond.push("f.mtime <= ?".into());
        args.push(Value::Integer(a));
    }
    if let Some(m) = filtri.size_min {
        cond.push("f.size >= ?".into());
        args.push(Value::Integer(m));
    }
    if let Some(m) = filtri.size_max {
        cond.push("f.size <= ?".into());
        args.push(Value::Integer(m));
    }
    // Gli artefatti sono l'88% dei documenti su un disco di sviluppo: se
    // comparissero di default la ricerca sarebbe inutilizzabile.
    //
    // L'esclusione dipende dalla scelta esplicita dell'utente — l'interruttore
    // oppure il tipo `artefatto` chiesto per nome — e non dal fatto che ci
    // siano altri filtri di tipo attivi: legarla a `tipi.is_empty()` rendeva
    // l'interruttore inerte appena si sceglieva un tipo qualunque.
    let chiesti_per_nome = filtri.tipi.iter().any(|t| t == "artefatto");
    if !filtri.includi_artefatti && !chiesti_per_nome {
        cond.push("f.tipo <> 'artefatto'".into());
    }
}

fn limite_offset(filtri: &Filtri) -> (i64, i64) {
    (
        filtri.limite.unwrap_or(200).clamp(1, 2000),
        filtri.offset.unwrap_or(0).max(0),
    )
}

/// Clausola `ORDER BY` scelta dall'utente.
///
/// Restituisce sempre una costante compilata qui dentro, mai una stringa che
/// arriva dal frontend: un `ORDER BY` non si può parametrizzare, quindi
/// l'unico modo sicuro di renderlo scegliibile è una tabella chiusa.
/// `predefinito` è ciò che la vista usa quando non è stato chiesto nulla —
/// la rilevanza per il full-text, la data per la ricerca sul nome.
pub(crate) fn ordinamento(filtri: &Filtri, predefinito: &'static str) -> &'static str {
    match filtri.ordine.as_deref() {
        Some("dimensione") => "f.size DESC, f.path",
        Some("recenti") => "f.mtime DESC",
        Some("vecchi") => "f.mtime ASC",
        Some("nome") => "f.nome COLLATE NOCASE, f.path",
        _ => predefinito,
    }
}

/// Ricerca full-text su nome e testo estratto, con snippet evidenziato.
pub fn full_text(db: &Db, query: &str, filtri: &Filtri) -> Result<Vec<Risultato>> {
    let Some(fts) = query_fts(query) else {
        return Ok(Vec::new());
    };

    let mut cond = vec!["file_fts MATCH ?".to_string()];
    let mut args: Vec<Value> = vec![Value::Text(fts.clone())];
    clausole(filtri, &mut cond, &mut args);
    let (lim, off) = limite_offset(filtri);

    // FTS5, a differenza di FTS3/4, non espone la posizione del match. Per
    // sapere in quale pagina cade si localizza il primo termine con `instr`
    // dentro una sottoquery: costa una scansione della sola riga di testo
    // interessata e evita di trasferire l'intero contenuto alla UI.
    let primo_termine = query
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_end_matches('*')
        .to_lowercase();

    let sql = format!(
        "SELECT {COLONNE},
                snippet(file_fts, 1, '[[', ']]', '…', 14) AS frammento,
                bm25(file_fts) AS punteggio,
                f.offset_pagine AS offsets,
                (SELECT instr(lower(t.testo), ?)
                   FROM file_fts t WHERE t.rowid = f.id) AS pos
           FROM file_fts
           JOIN file f ON f.id = file_fts.rowid
          WHERE {}
          ORDER BY {}
          LIMIT ? OFFSET ?",
        cond.join(" AND "),
        ordinamento(filtri, "punteggio")
    );
    // Il segnaposto di `instr` precede quelli della WHERE nell'ordine di
    // apparizione nella query, quindi va inserito in testa agli argomenti.
    args.insert(0, Value::Text(primo_termine));
    args.push(Value::Integer(lim));
    args.push(Value::Integer(off));

    let conn = db.conn();
    let mut st = conn.prepare(&sql)?;

    let riferimenti: Vec<&dyn ToSql> = args.iter().map(|v| v as &dyn ToSql).collect();
    let mut righe = st.query(riferimenti.as_slice())?;

    let mut out = Vec::new();
    while let Some(r) = righe.next()? {
        let file = riga_in_file(r)?;
        let snippet: String = r.get("frammento")?;
        let punteggio: f64 = r.get("punteggio")?;
        let offsets: Option<String> = r.get("offsets")?;
        // `instr` è 1-based e ritorna 0 quando non trova nulla.
        let pos: Option<i64> = r.get::<_, Option<i64>>("pos")?.filter(|p| *p > 0);

        let pagina = match (offsets.as_deref(), pos) {
            (Some(j), Some(p)) => pagina_da_offset(j, (p - 1) as usize),
            _ => None,
        };

        out.push(Risultato {
            riga: None,
            pagina,
            snippet,
            // bm25 restituisce valori negativi, più bassi = più rilevanti:
            // si inverte per dare alla UI un punteggio che cresce col merito.
            rank: -punteggio,
            file,
        });
    }
    drop(righe);
    drop(st);
    drop(conn);

    // Per i tracciati il numero di riga si ricava dall'indice dei record.
    for r in out.iter_mut() {
        if matches!(r.file.tipo, Tipo::Tracciato) {
            r.riga = riga_del_match(db, r.file.id, &fts).unwrap_or(None);
        }
    }
    Ok(out)
}

/// In quale pagina cade un offset di carattere, dato l'elenco degli inizi.
fn pagina_da_offset(json: &str, pos: usize) -> Option<i64> {
    let inizi: Vec<usize> = serde_json::from_str(json).ok()?;
    if inizi.is_empty() {
        return None;
    }
    let idx = match inizi.binary_search(&pos) {
        Ok(i) => i,
        Err(0) => 0,
        Err(i) => i - 1,
    };
    Some(idx as i64 + 1)
}

/// Prima riga di un tracciato che soddisfa la stessa query.
fn riga_del_match(db: &Db, file_id: i64, fts: &str) -> Result<Option<i64>> {
    let conn = db.conn();
    let riga = conn
        .query_row(
            "SELECT rt.numero_riga
               FROM record_fts
               JOIN record_tracciato rt ON rt.id = record_fts.rowid
              WHERE record_fts MATCH ?1 AND rt.file_id = ?2
              ORDER BY rank
              LIMIT 1",
            rusqlite::params![fts, file_id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(riga)
}

/// Ricerca istantanea sul solo nome file, in stile Everything.
pub fn per_nome(db: &Db, query: &str, filtri: &Filtri) -> Result<Vec<FileRecord>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let mut cond = vec!["f.nome LIKE ? ESCAPE '\\'".to_string()];
    // I metacaratteri di LIKE vanno neutralizzati, altrimenti un `_` digitato
    // dall'utente si comporta come un jolly e i risultati sembrano casuali.
    let sanificato = q
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let mut args: Vec<Value> = vec![Value::Text(format!("%{sanificato}%"))];
    clausole(filtri, &mut cond, &mut args);
    let (lim, off) = limite_offset(filtri);

    let sql = format!(
        "SELECT {COLONNE} FROM file f WHERE {} ORDER BY {} LIMIT ? OFFSET ?",
        cond.join(" AND "),
        ordinamento(filtri, "f.mtime DESC")
    );
    args.push(Value::Integer(lim));
    args.push(Value::Integer(off));

    let conn = db.conn();
    let mut st = conn.prepare(&sql)?;
    let riferimenti: Vec<&dyn ToSql> = args.iter().map(|v| v as &dyn ToSql).collect();
    let v = st
        .query_map(riferimenti.as_slice(), riga_in_file)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(v)
}

/// Anteprima del contenuto: righe di tracciato o testo estratto.
pub fn anteprima(db: &Db, id: i64, pagina: Option<i64>) -> Result<Anteprima> {
    let Some(file) = db.file_per_id(id)? else {
        return Ok(Anteprima {
            genere: "nessuna".into(),
            testo: None,
            record: None,
            intestazioni: None,
            pagine: None,
            pagina: None,
            messaggio: Some("file non presente nell'indice".into()),
        });
    };

    if matches!(file.tipo, Tipo::Tracciato) {
        return crate::tracciati::record(db, id, None, 0, 200);
    }

    let conn = db.conn();
    let testo: Option<String> = conn
        .query_row(
            "SELECT testo FROM file_fts WHERE rowid = ?1",
            rusqlite::params![id],
            |r| r.get(0),
        )
        .optional()?;
    let offsets: Option<String> = conn
        .query_row(
            "SELECT offset_pagine FROM file WHERE id = ?1",
            rusqlite::params![id],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    drop(conn);

    let Some(tutto) = testo else {
        return Ok(Anteprima {
            genere: "nessuna".into(),
            testo: None,
            record: None,
            intestazioni: None,
            pagine: file.pagine,
            pagina: None,
            messaggio: Some(if file.testo_estratto {
                "testo non disponibile nell'indice".into()
            } else {
                "da questo file non è stato estratto testo".into()
            }),
        });
    };

    // Con gli offset di pagina si mostra la sola pagina richiesta: su un PDF
    // di 200 pagine riversare tutto nella UI è inutile e lento.
    let (frammento, pag) = match (pagina, offsets.as_deref()) {
        (Some(p), Some(j)) => match ritaglia_pagina(&tutto, j, p) {
            Some(t) => (t, Some(p)),
            None => (tutto.clone(), None),
        },
        _ => (tutto.clone(), None),
    };

    const TETTO: usize = 200_000;
    let troncato: String = if frammento.len() > TETTO {
        frammento.chars().take(TETTO).collect()
    } else {
        frammento
    };

    Ok(Anteprima {
        genere: "testo".into(),
        testo: Some(troncato),
        record: None,
        intestazioni: None,
        pagine: file.pagine,
        pagina: pag,
        messaggio: None,
    })
}

/// Gli offset di pagina sono in CARATTERI, non in byte: è la stessa unità che
/// usa `instr` di SQLite, e mescolare le due porta a numeri di pagina
/// sistematicamente sbagliati sui testi con accenti.
fn ritaglia_pagina(testo: &str, json_offsets: &str, pagina: i64) -> Option<String> {
    let inizi: Vec<usize> = serde_json::from_str(json_offsets).ok()?;
    let i = usize::try_from(pagina.checked_sub(1)?).ok()?;
    let da = *inizi.get(i)?;
    let a = inizi.get(i + 1).copied().unwrap_or(usize::MAX);
    if da >= a {
        return None;
    }
    let ritagliato: String = testo.chars().skip(da).take(a - da).collect();
    if ritagliato.is_empty() {
        None
    } else {
        Some(ritagliato)
    }
}

/// Conteggi per contesto, per popolare i filtri laterali.
pub fn faccette_contesto(db: &Db) -> Result<Vec<ConteggioEtichetta>> {
    let conn = db.conn();
    let mut st = conn.prepare(
        "SELECT COALESCE(contesto, '(non classificati)') AS et, COUNT(*), COALESCE(SUM(size),0)
           FROM file
          WHERE tipo <> 'artefatto'
          GROUP BY et
          ORDER BY 2 DESC",
    )?;
    let v = st
        .query_map([], |r| {
            Ok(ConteggioEtichetta {
                etichetta: r.get(0)?,
                quanti: r.get(1)?,
                byte: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(v)
}

/// Dati della dashboard.
pub fn statistiche(db: &Db) -> Result<Statistiche> {
    let conn = db.conn();

    let uno = |sql: &str| -> rusqlite::Result<i64> { conn.query_row(sql, [], |r| r.get(0)) };

    let file_totali = uno("SELECT COUNT(*) FROM file WHERE tipo <> 'artefatto'")?;
    let byte_totali = uno("SELECT COALESCE(SUM(size),0) FROM file WHERE tipo <> 'artefatto'")?;
    let documenti = uno("SELECT COUNT(*) FROM file WHERE tipo = 'documento'")?;
    let tracciati = uno("SELECT COUNT(*) FROM file WHERE tipo = 'tracciato'")?;
    let artefatti_esclusi = uno("SELECT COUNT(*) FROM file WHERE tipo = 'artefatto'")?;
    let non_classificati =
        uno("SELECT COUNT(*) FROM file WHERE contesto IS NULL AND tipo <> 'artefatto'")?;

    // I gruppi di duplicati si contano qui senza passare da `dedupe::gruppi`:
    // la dashboard deve poter essere aperta senza innescare una riscrittura
    // dello stato dei file.
    let gruppi_duplicati = uno(
        "SELECT COUNT(*) FROM (
             SELECT hash FROM file
              WHERE hash IS NOT NULL AND tipo <> 'artefatto'
              GROUP BY hash HAVING COUNT(*) > 1)",
    )?;
    let spazio_duplicati = uno(
        "SELECT COALESCE(SUM(extra),0) FROM (
             SELECT (COUNT(*) - 1) * MAX(size) AS extra FROM file
              WHERE hash IS NOT NULL AND tipo <> 'artefatto'
              GROUP BY hash HAVING COUNT(*) > 1)",
    )?;

    let per = |campo: &str, filtro: &str| -> rusqlite::Result<Vec<ConteggioEtichetta>> {
        let sql = format!(
            "SELECT COALESCE({campo}, '(non classificati)'), COUNT(*), COALESCE(SUM(size),0)
               FROM file {filtro} GROUP BY 1 ORDER BY 2 DESC"
        );
        let mut st = conn.prepare(&sql)?;
        let v = st
            .query_map([], |r| {
                Ok(ConteggioEtichetta {
                    etichetta: r.get(0)?,
                    quanti: r.get(1)?,
                    byte: r.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    };

    let per_tipo = per("tipo", "")?;
    let per_contesto = per("contesto", "WHERE tipo <> 'artefatto'")?;

    let ultima_scansione = conn
        .query_row(
            "SELECT valore FROM impostazioni WHERE chiave = 'ultima_scansione'",
            [],
            |r| r.get::<_, String>(0),
        )
        .optional()?
        .and_then(|s| s.parse::<i64>().ok());

    Ok(Statistiche {
        file_totali,
        byte_totali,
        documenti,
        tracciati,
        artefatti_esclusi,
        non_classificati,
        gruppi_duplicati,
        spazio_duplicati,
        per_tipo,
        per_contesto,
        ultima_scansione,
    })
}

/// Cosa è arrivato di nuovo negli ultimi `giorni`. È la vista che risolve il
/// bisogno da cui è nato il progetto: capire cosa è finito in Scaricati.
pub fn novita(db: &Db, giorni: i64) -> Result<Vec<FileRecord>> {
    let soglia = chrono::Utc::now().timestamp() - giorni.max(1) * 86_400;
    let conn = db.conn();
    let sql = format!(
        "SELECT {COLONNE} FROM file f
          WHERE f.mtime >= ?1 AND f.tipo <> 'artefatto'
          ORDER BY f.mtime DESC LIMIT 500"
    );
    let mut st = conn.prepare(&sql)?;
    let v = st
        .query_map(rusqlite::params![soglia], riga_in_file)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(v)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn la_query_fts_neutralizza_gli_apici() {
        assert_eq!(query_fts("contratto"), Some("\"contratto\"".into()));
        assert_eq!(
            query_fts("test carta"),
            Some("\"test\" AND \"carta\"".into())
        );
        // Un apice digitato dall'utente non deve rompere la sintassi FTS.
        assert_eq!(query_fts("d\"oh"), Some("\"d\"\"oh\"".into()));
        // Il jolly finale è l'unica sintassi che sopravvive.
        assert_eq!(query_fts("contr*"), Some("\"contr\"*".into()));
        assert_eq!(query_fts("   "), None);
    }

    #[test]
    fn la_pagina_si_ricava_dagli_offset() {
        let j = "[0,100,250]";
        assert_eq!(pagina_da_offset(j, 0), Some(1));
        assert_eq!(pagina_da_offset(j, 99), Some(1));
        assert_eq!(pagina_da_offset(j, 100), Some(2));
        assert_eq!(pagina_da_offset(j, 300), Some(3));
        assert_eq!(pagina_da_offset("[]", 5), None);
    }

    #[test]
    fn il_ritaglio_conta_caratteri_non_byte() {
        // 'però' sono 4 caratteri ma 5 byte: contando byte la pagina 1
        // finirebbe a metà della 'ò'.
        let testo = "però\nsecondo";
        assert_eq!(ritaglia_pagina(testo, "[0,5]", 1).as_deref(), Some("però\n"));
        assert_eq!(
            ritaglia_pagina(testo, "[0,5]", 2).as_deref(),
            Some("secondo")
        );
        assert_eq!(ritaglia_pagina(testo, "[0,5]", 9), None);
    }
}
