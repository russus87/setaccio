//! Dove sono finiti i gigabyte.
//!
//! La domanda «il disco è pieno, di cosa?» non si risponde con un elenco
//! ordinato per dimensione. Un elenco dice *quali* file sono grossi; quasi
//! sempre quello che serve sapere è *quale ramo* dell'albero pesa, perché
//! trecento file da 40 MB nella stessa cartella contano più di un singolo
//! file da 4 GB e in una lista per dimensione non compaiono nemmeno.
//!
//! Questo modulo risponde su tre assi insieme — file, cartelle, estensioni —
//! e li calcola tutti dall'indice, senza toccare il disco: l'aggregazione per
//! cartella si ricava risalendo i percorsi già memorizzati.

use anyhow::Result;
use rusqlite::{types::Value, ToSql};
use std::collections::HashMap;

use crate::cerca::clausole;
use crate::db::{riga_in_file, Db};
use crate::types::{CartellaPesante, ConteggioEtichetta, Filtri, Ingombro};

/// Quante cartelle restituire al massimo.
const CARTELLE_MAX: usize = 40;

/// Quante estensioni restituire al massimo.
const ESTENSIONI_MAX: usize = 20;

const COLONNE: &str = "f.id, f.path, f.nome, f.ext, f.size, f.mtime, f.hash, f.tipo, f.contesto,
     f.stato, f.motivo_tipo, f.motivo_contesto, f.sorgente_id, f.archivio_padre, f.lotto,
     f.testo_estratto, f.pagine";

/// Il quadro completo dell'ingombro per i filtri dati.
///
/// `limite` vale solo per l'elenco dei file: i totali, le cartelle e le
/// estensioni si calcolano su tutto ciò che passa i filtri, altrimenti le
/// percentuali mentirebbero.
pub fn ingombro(db: &Db, filtri: &Filtri, limite: i64) -> Result<Ingombro> {
    let limite = limite.clamp(1, 2000);

    // Ciò che è stato indicizzato dall'interno di uno zip **non** conta qui,
    // ed è l'unico punto dell'applicazione in cui va escluso a priori.
    // Due motivi, entrambi rendono i numeri falsi:
    // 1. quei byte sul disco non esistono due volte. L'archivio è già
    //    contato con la sua dimensione, e sommarci anche il contenuto
    //    scompattato gonfia il totale di quanto comprime lo zip;
    // 2. il loro percorso (`pacco.zip!/dentro/x`) non è una cartella vera, e
    //    l'aggregazione la mostrerebbe come un posto in cui andare a pulire.
    let mut cond: Vec<String> = vec!["f.archivio_padre IS NULL".to_string()];
    let mut args: Vec<Value> = Vec::new();
    clausole(filtri, &mut cond, &mut args);
    let dove = cond.join(" AND ");

    let conn = db.conn();

    // ---- I file più grandi ------------------------------------------------
    let sql = format!("SELECT {COLONNE} FROM file f WHERE {dove} ORDER BY f.size DESC, f.path LIMIT ?");
    let mut args_file = args.clone();
    args_file.push(Value::Integer(limite));
    let file = {
        let mut st = conn.prepare(&sql)?;
        let rif: Vec<&dyn ToSql> = args_file.iter().map(|v| v as &dyn ToSql).collect();
        let v = st
            .query_map(rif.as_slice(), riga_in_file)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        v
    };
    let byte_mostrati = file.iter().map(|f| f.size).sum();

    // ---- Totali -----------------------------------------------------------
    let sql = format!("SELECT COUNT(*), COALESCE(SUM(f.size),0) FROM file f WHERE {dove}");
    let (quanti_totali, byte_totali): (i64, i64) = {
        let mut st = conn.prepare(&sql)?;
        let rif: Vec<&dyn ToSql> = args.iter().map(|v| v as &dyn ToSql).collect();
        st.query_row(rif.as_slice(), |r| Ok((r.get(0)?, r.get(1)?)))?
    };

    // ---- Per estensione ---------------------------------------------------
    let sql = format!(
        "SELECT COALESCE(NULLIF(f.ext,''), '(senza estensione)'), COUNT(*), COALESCE(SUM(f.size),0)
           FROM file f WHERE {dove}
          GROUP BY 1 ORDER BY 3 DESC LIMIT ?"
    );
    let mut args_ext = args.clone();
    args_ext.push(Value::Integer(ESTENSIONI_MAX as i64));
    let per_estensione = {
        let mut st = conn.prepare(&sql)?;
        let rif: Vec<&dyn ToSql> = args_ext.iter().map(|v| v as &dyn ToSql).collect();
        let v = st
            .query_map(rif.as_slice(), conteggio)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        v
    };

    // ---- Per tipo ---------------------------------------------------------
    let sql = format!(
        "SELECT f.tipo, COUNT(*), COALESCE(SUM(f.size),0)
           FROM file f WHERE {dove} GROUP BY 1 ORDER BY 3 DESC"
    );
    let per_tipo = {
        let mut st = conn.prepare(&sql)?;
        let rif: Vec<&dyn ToSql> = args.iter().map(|v| v as &dyn ToSql).collect();
        let v = st
            .query_map(rif.as_slice(), conteggio)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        v
    };

    // ---- Per cartella -----------------------------------------------------
    // Si leggono solo percorso e dimensione di tutto l'insieme filtrato: su
    // un indice da centomila file sono pochi megabyte, e l'alternativa
    // (una GROUP BY per ogni livello di profondità) sarebbe una query per
    // livello senza sapere quanti livelli ci sono.
    let sql = format!("SELECT f.path, f.size FROM file f WHERE {dove}");
    let percorsi: Vec<(String, i64)> = {
        let mut st = conn.prepare(&sql)?;
        let rif: Vec<&dyn ToSql> = args.iter().map(|v| v as &dyn ToSql).collect();
        let v = st
            .query_map(rif.as_slice(), |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        v
    };
    drop(conn);

    let cartelle = aggrega_cartelle(&percorsi);

    Ok(Ingombro {
        file,
        cartelle,
        per_estensione,
        per_tipo,
        byte_totali,
        quanti_totali,
        byte_mostrati,
    })
}

fn conteggio(r: &rusqlite::Row) -> rusqlite::Result<ConteggioEtichetta> {
    Ok(ConteggioEtichetta {
        etichetta: r.get(0)?,
        quanti: r.get(1)?,
        byte: r.get(2)?,
    })
}

/// Accumulatore per una cartella durante la risalita dei percorsi.
#[derive(Default, Clone, Copy)]
struct Peso {
    byte: i64,
    quanti: i64,
    byte_diretti: i64,
    quanti_diretti: i64,
}

/// Somma i file sui loro antenati e restituisce le cartelle che pesano di più.
///
/// Il separatore si normalizza a `/` per il confronto ma il percorso
/// restituito resta quello originale: su Windows una cartella mostrata con le
/// barre rovesciate è quella che l'utente può incollare in Esplora risorse.
fn aggrega_cartelle(percorsi: &[(String, i64)]) -> Vec<CartellaPesante> {
    let mut pesi: HashMap<&str, Peso> = HashMap::new();

    for (path, size) in percorsi {
        let mut primo = true;
        // Si risale tagliando sull'ultimo separatore: `Path::ancestors`
        // costerebbe un `PathBuf` per livello per ogni file.
        let mut resto: &str = path;
        while let Some(taglio) = resto.rfind(['/', '\\']) {
            // Taglio a 0 vuol dire la radice del filesystem: non è una
            // cartella su cui abbia senso ragionare, e la stringa vuota che
            // ne uscirebbe si confonderebbe con «nessun percorso».
            if taglio == 0 {
                break;
            }
            let dir = &resto[..taglio];
            // Un percorso UNC (`\\server\share\x`) scende fino a `\`, che non
            // è una cartella ma il residuo del prefisso: aggregherebbe tutto
            // sotto una voce senza nome in cima all'elenco.
            if dir.chars().all(|c| c == '/' || c == '\\') {
                break;
            }
            let p = pesi.entry(dir).or_default();
            p.byte += size;
            p.quanti += 1;
            if primo {
                p.byte_diretti += size;
                p.quanti_diretti += 1;
                primo = false;
            }
            resto = dir;
        }
    }

    // Per ogni cartella, il conteggio del figlio più capiente. Si ricava in
    // una passata attribuendo ogni cartella al proprio padre: cercare i
    // discendenti per prefisso costerebbe un confronto fra ogni coppia di
    // cartelle, e su un indice grande sono milioni di confronti.
    let mut figlio_massimo: HashMap<&str, i64> = HashMap::new();
    for (dir, p) in &pesi {
        if let Some(taglio) = dir.rfind(['/', '\\']).filter(|t| *t > 0) {
            let padre = &dir[..taglio];
            let v = figlio_massimo.entry(padre).or_insert(0);
            *v = (*v).max(p.quanti);
        }
    }

    // Una cartella che contiene esattamente quello che contiene un suo figlio
    // non aggiunge informazione: è un passaggio dell'albero, non un posto
    // dove guardare. Si tiene quindi solo chi ha file propri o chi divide il
    // peso fra più rami, così l'elenco mostra i punti in cui si può decidere.
    let mut candidate: Vec<CartellaPesante> = pesi
        .iter()
        .filter(|(dir, p)| {
            p.quanti_diretti > 0 || figlio_massimo.get(*dir).copied() != Some(p.quanti)
        })
        .map(|(dir, p)| CartellaPesante {
            path: (*dir).to_string(),
            nome: dir
                .rsplit(['/', '\\'])
                .next()
                .filter(|s| !s.is_empty())
                .unwrap_or(dir)
                .to_string(),
            byte: p.byte,
            quanti: p.quanti,
            byte_diretti: p.byte_diretti,
            profondita: dir.matches(['/', '\\']).count(),
        })
        .collect();

    candidate.sort_by(|a, b| b.byte.cmp(&a.byte).then_with(|| a.path.cmp(&b.path)));
    candidate.truncate(CARTELLE_MAX);
    candidate
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use super::*;

    fn p(path: &str, size: i64) -> (String, i64) {
        (path.to_string(), size)
    }

    #[test]
    fn il_peso_sale_lungo_gli_antenati() {
        let file = vec![
            p("/casa/foto/2024/a.jpg", 100),
            p("/casa/foto/2024/b.jpg", 200),
            p("/casa/documenti/c.pdf", 50),
        ];
        let out = aggrega_cartelle(&file);
        let per = |path: &str| out.iter().find(|c| c.path == path).cloned();

        let casa = per("/casa").expect("la radice comune deve esserci");
        assert_eq!(casa.byte, 350);
        assert_eq!(casa.quanti, 3);
        // In `/casa` non c'è nessun file: tutto il peso viene da sotto.
        assert_eq!(casa.byte_diretti, 0);

        let anno = per("/casa/foto/2024").unwrap();
        assert_eq!(anno.byte, 300);
        assert_eq!(anno.byte_diretti, 300);
        assert_eq!(anno.profondita, 3);
    }

    #[test]
    fn i_passaggi_che_non_dividono_niente_spariscono() {
        // `/a/b/c` contiene tutto ciò che contiene `/a/b`, che contiene tutto
        // ciò che contiene `/a`: dei tre solo il più profondo è un posto dove
        // valga la pena guardare.
        let out = aggrega_cartelle(&[p("/a/b/c/f.bin", 10), p("/a/b/c/g.bin", 20)]);
        let path: Vec<&str> = out.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(path, vec!["/a/b/c"]);
    }

    #[test]
    fn il_punto_in_cui_il_peso_si_divide_resta() {
        let out = aggrega_cartelle(&[p("/a/uno/f.bin", 10), p("/a/due/g.bin", 20)]);
        let path: Vec<&str> = out.iter().map(|c| c.path.as_str()).collect();
        // `/a` è il ramo che si divide e va mostrato; `/a/due` pesa più di
        // `/a/uno` e viene prima.
        assert_eq!(path, vec!["/a", "/a/due", "/a/uno"]);
    }

    #[test]
    fn la_radice_del_filesystem_non_e_una_cartella_da_mostrare() {
        let out = aggrega_cartelle(&[p("/f.bin", 10)]);
        assert!(out.is_empty(), "«/» non è un posto dove fare pulizia");
    }

    #[test]
    fn il_prefisso_unc_non_diventa_una_cartella_senza_nome() {
        let out = aggrega_cartelle(&[p(r"\\server\share\dati\f.bin", 10)]);
        let path: Vec<&str> = out.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(path, vec![r"\\server\share\dati"]);
    }

    // --- Il giro completo sull'indice ------------------------------------

    use crate::db::FileVisto;
    use crate::types::Tipo;

    fn indice() -> Db {
        let db = Db::in_memoria().unwrap();
        let sid = db.sorgente_aggiungi("/casa", Fascia::Documenti).unwrap();
        let metti = |path: &str, size: i64, ext: &str, tipo: Tipo| {
            db.file_upsert(&FileVisto {
                path: path.into(),
                nome: path.rsplit('/').next().unwrap().into(),
                ext: Some(ext.into()),
                size,
                mtime: 1,
                tipo,
                motivo_tipo: "test".into(),
                contesto: None,
                motivo_contesto: None,
                sorgente_id: sid,
                archivio_padre: None,
                lotto: None,
            })
            .unwrap();
        };
        let dentro_archivio = |path: &str, size: i64, archivio: &str| {
            db.file_upsert(&FileVisto {
                path: path.into(),
                nome: path.rsplit('/').next().unwrap().into(),
                ext: Some("txt".into()),
                size,
                mtime: 1,
                tipo: Tipo::Documento,
                motivo_tipo: "test".into(),
                contesto: None,
                motivo_contesto: None,
                sorgente_id: sid,
                archivio_padre: Some(archivio.into()),
                lotto: None,
            })
            .unwrap();
        };
        metti("/casa/video/film.mkv", 4_000, "mkv", Tipo::Media);
        metti("/casa/video/corto.mkv", 1_000, "mkv", Tipo::Media);
        metti("/casa/doc/tesi.pdf", 500, "pdf", Tipo::Documento);
        // Un artefatto grosso: fuori dai conti se non lo si chiede.
        metti("/casa/repo/target/app.bin", 9_000, "bin", Tipo::Artefatto);
        // Contenuto di un archivio: quei byte sul disco stanno già dentro
        // `pacco.zip`, che è contato per conto suo.
        metti("/casa/doc/pacco.zip", 300, "zip", Tipo::Archivio);
        dentro_archivio("/casa/doc/pacco.zip!/enorme.txt", 8_000, "/casa/doc/pacco.zip");
        db
    }

    use crate::types::Fascia;

    #[test]
    fn gli_artefatti_restano_fuori_dai_conti_finche_non_si_chiedono() {
        let db = indice();

        let out = ingombro(&db, &Filtri::default(), 10).unwrap();
        assert_eq!(out.quanti_totali, 4);
        assert_eq!(out.byte_totali, 5_800);
        // Il più grosso in cima, e l'artefatto da 9 KB non compare.
        assert_eq!(out.file[0].nome, "film.mkv");
        assert!(out.file.iter().all(|f| f.nome != "app.bin"));

        let con_artefatti = Filtri {
            includi_artefatti: true,
            ..Default::default()
        };
        let out = ingombro(&db, &con_artefatti, 10).unwrap();
        assert_eq!(out.quanti_totali, 5);
        assert_eq!(out.file[0].nome, "app.bin");

        // Chiederli per nome basta da solo: è la strada della pillola
        // «artefatti», che deve funzionare a interruttore spento.
        let solo_artefatti = Filtri {
            tipi: vec!["artefatto".into()],
            ..Default::default()
        };
        let out = ingombro(&db, &solo_artefatti, 10).unwrap();
        assert_eq!(out.quanti_totali, 1);
        assert_eq!(out.file[0].nome, "app.bin");
    }

    #[test]
    fn il_contenuto_degli_archivi_non_gonfia_i_totali() {
        // `enorme.txt` da 8 KB sta dentro un pacco da 300 byte: contarlo
        // vorrebbe dire dire che il disco è pieno di roba che non c'è, e
        // proporre `pacco.zip!` come una cartella in cui fare pulizia.
        let db = indice();
        let out = ingombro(&db, &Filtri::default(), 10).unwrap();

        assert!(
            out.file.iter().all(|f| f.nome != "enorme.txt"),
            "il contenuto di un archivio non è un file sul disco"
        );
        assert!(
            out.cartelle.iter().all(|c| !c.path.contains(".zip!")),
            "un percorso dentro un archivio non è una cartella: {:?}",
            out.cartelle.iter().map(|c| &c.path).collect::<Vec<_>>()
        );
        // Del pacco resta la sua dimensione vera, quella dell'archivio.
        assert_eq!(out.byte_totali, 5_800);
        let doc = out.cartelle.iter().find(|c| c.path == "/casa/doc").unwrap();
        assert_eq!(doc.byte, 800, "tesi 500 + pacco 300, niente di più");
    }

    #[test]
    fn il_limite_vale_sui_file_ma_non_sui_totali() {
        let db = indice();
        let out = ingombro(&db, &Filtri::default(), 1).unwrap();
        assert_eq!(out.file.len(), 1);
        assert_eq!(out.byte_mostrati, 4_000);
        // I totali continuano a parlare di tutto: senza questo la percentuale
        // «i primi N pesano X» sarebbe sempre 100%.
        assert_eq!(out.byte_totali, 5_800);
        assert_eq!(out.quanti_totali, 4);
    }

    #[test]
    fn la_soglia_di_dimensione_filtra_anche_cartelle_ed_estensioni() {
        let db = indice();
        let grandi = Filtri {
            size_min: Some(900),
            ..Default::default()
        };
        let out = ingombro(&db, &grandi, 10).unwrap();
        assert_eq!(out.quanti_totali, 2);
        // `doc/` conteneva solo un file sotto soglia: sparisce dall'elenco.
        assert!(out.cartelle.iter().all(|c| !c.path.ends_with("/doc")));
        let video = out
            .cartelle
            .iter()
            .find(|c| c.path == "/casa/video")
            .expect("la cartella pesante deve esserci");
        assert_eq!(video.byte, 5_000);
        assert_eq!(video.quanti, 2);
        assert_eq!(
            out.per_estensione.iter().map(|e| e.etichetta.as_str()).collect::<Vec<_>>(),
            vec!["mkv"]
        );
    }
}
