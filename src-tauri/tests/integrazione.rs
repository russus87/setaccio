//! Verifica end-to-end su dati reali.
//!
//! I test unitari dei singoli moduli girano su fixture costruite al momento.
//! Qui invece si scandisce una cartella vera del disco dell'autore — il lotto
//! di composizione documentale `T1Q73KFP` — e si controlla che la catena
//! completa funzioni: walker → classificazione → estrazione → indice → ricerca
//! → correlazione di lotto.
//!
//! Se la cartella non c'è (altra macchina, CI) i test escono senza fallire:
//! meglio un test che si astiene di uno che mente.

use std::path::Path;
use std::sync::Arc;

use setaccio_lib::db::Db;
use setaccio_lib::types::{Fascia, Filtri, Tipo};
use setaccio_lib::{cerca, dedupe, scan, tracciati};

/// Dove può trovarsi il lotto di prova.
///
/// Il secondo percorso esiste perché questi test girano sulla macchina di chi
/// *usa* Setaccio: la modalità Organizza sposta davvero i file, quindi un
/// fixture ancorato a un percorso fisso si rompe il giorno in cui lo strumento
/// fa il suo mestiere. Si cerca il lotto dove può essere finito.
const CANDIDATI_LOTTO: &[&str] = &[
    "/home/russus/Scaricati/T1Q73KFP",
    "/home/russus/Scaricati/lavoro/ILC",
];

/// Il primo candidato che contiene davvero dei file.
fn lotto() -> Option<String> {
    CANDIDATI_LOTTO
        .iter()
        .find(|c| ha_file(Path::new(c)))
        .map(|c| c.to_string())
}

fn ha_file(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|voci| {
            voci.flatten()
                .any(|v| v.file_type().map(|t| t.is_file()).unwrap_or(false))
        })
        .unwrap_or(false)
}

/// Indice temporaneo popolato col lotto di prova, ovunque esso sia finito.
fn scandisci_lotto(fascia: Fascia) -> Option<Arc<Db>> {
    let dir = lotto()?;
    scandisci(&dir, fascia)
}

/// Prepara un indice temporaneo e ci scandisce dentro la cartella indicata.
fn scandisci(sorgente: &str, fascia: Fascia) -> Option<Arc<Db>> {
    if !Path::new(sorgente).is_dir() {
        eprintln!("salto: «{sorgente}» non presente su questa macchina");
        return None;
    }
    if !ha_file(Path::new(sorgente)) {
        eprintln!("salto: «{sorgente}» non contiene file");
        return None;
    }
    let db = Arc::new(Db::in_memoria().expect("indice in memoria"));
    db.sorgente_aggiungi(sorgente, fascia).expect("sorgente");

    let controllo = Arc::new(scan::Controllo::nuovo());
    scan::esegui(db.clone(), controllo.clone(), |_| {}).expect("scansione");

    let p = controllo.progresso();
    assert!(p.finito, "la scansione deve dichiararsi finita");
    assert!(p.visti > 0, "nessun file visto in «{sorgente}»");
    Some(db)
}

#[test]
fn il_lotto_reale_viene_indicizzato_e_classificato() {
    let Some(db) = scandisci_lotto(Fascia::Tracciati) else {
        return;
    };
    let dir = lotto().expect("lotto individuato dalla scansione");

    // Il file `328102` non ha estensione, è UTF-8 con record a lunghezza
    // fissa: nessun indexer generalista lo apre, ed è esattamente il caso per
    // cui Setaccio esiste.
    let tracciato = db
        .file_per_path(&format!("{dir}/328102"))
        .expect("query")
        .expect("il tracciato 328102 deve essere nell'indice");
    assert_eq!(
        tracciato.tipo,
        Tipo::Tracciato,
        "328102 classificato come {:?} invece che tracciato (motivo: {:?})",
        tracciato.tipo,
        tracciato.motivo_tipo
    );

    // I PDF generati dal lotto sono documenti, non artefatti: non c'è nessun
    // marker di repository fra i loro antenati.
    let pdf = db
        .file_per_path(&format!("{dir}/T1Q73KFP_0000001.pdf"))
        .expect("query")
        .expect("il PDF del lotto deve essere nell'indice");
    assert_eq!(pdf.tipo, Tipo::Documento);
    assert!(
        pdf.testo_estratto,
        "dal PDF del lotto deve essere stato estratto il testo"
    );

    // Ogni classificazione deve poter essere spiegata.
    assert!(
        tracciato.motivo_tipo.is_some(),
        "senza il «perché è finito qui» l'utente non si fida della categoria"
    );
}

/// Regressione. Il riconoscimento dei tracciati non deve dipendere dalla
/// fascia della sorgente: la classificazione usava un'euristica propria
/// («tutte le righe della stessa lunghezza») incompatibile con i flussi
/// multi-tipo reali, e su una sorgente di fascia `documenti` scartava i
/// tracciati prima che il modulo dedicato potesse vederli. Un censimento su
/// 17.414 file ne trovava zero.
#[test]
fn il_tracciato_si_riconosce_anche_in_una_sorgente_documenti() {
    let Some(db) = scandisci_lotto(Fascia::Documenti) else {
        return;
    };
    let dir = lotto().expect("lotto individuato dalla scansione");
    let t = db
        .file_per_path(&format!("{dir}/328102"))
        .expect("query")
        .expect("328102 deve essere nell'indice");
    assert_eq!(
        t.tipo,
        Tipo::Tracciato,
        "in fascia «documenti» 328102 è stato classificato {:?} (motivo: {:?})",
        t.tipo,
        t.motivo_tipo
    );

    let s = cerca::statistiche(&db).expect("statistiche");
    assert!(
        s.tracciati > 0,
        "la dashboard deve contare almeno un tracciato"
    );
}

#[test]
fn la_ricerca_trova_il_testo_dentro_il_pdf_del_lotto() {
    let Some(db) = scandisci_lotto(Fascia::Tracciati) else {
        return;
    };

    // Stringa realmente presente nella lettera contenuta nel PDF.
    let risultati = cerca::full_text(&db, "TEST CARTA 1", &Filtri::default()).expect("ricerca");
    assert!(
        !risultati.is_empty(),
        "«TEST CARTA 1» deve essere trovato dentro i PDF del lotto"
    );
    let r = &risultati[0];
    assert!(
        r.snippet.contains("[[") ,
        "lo snippet deve evidenziare i termini trovati, era: {:?}",
        r.snippet
    );

    // La ricerca per nome è un'altra strada, non deve dipendere dal testo.
    // Niente conteggi esatti: questa cartella è quella vera dell'utente e col
    // tempo raccoglie altri lotti. Si verifica che il file ci sia, non quanti
    // gliene stiano accanto.
    let per_nome = cerca::per_nome(&db, "328102", &Filtri::default()).expect("ricerca per nome");
    assert!(
        per_nome.iter().any(|f| f.nome == "328102"),
        "il tracciato si deve trovare per nome, trovati invece: {:?}",
        per_nome.iter().map(|f| &f.nome).collect::<Vec<_>>()
    );
}

#[test]
fn i_file_del_lotto_vengono_correlati_fra_loro() {
    let Some(db) = scandisci_lotto(Fascia::Tracciati) else {
        return;
    };

    let lotti = tracciati::lotti(&db).expect("lotti");
    assert!(!lotti.is_empty(), "il lotto T1Q73KFP deve essere riconosciuto");

    let l = lotti
        .iter()
        .find(|l| l.codice.contains("T1Q73KFP"))
        .expect("codice di lotto T1Q73KFP");

    // È questa la correlazione che dà valore al modulo: tracciato e PDF
    // generati smettono di essere due mucchi di file scollegati.
    assert!(!l.pdf.is_empty(), "il lotto deve raccogliere i PDF generati");
    assert!(
        !l.tracciati.is_empty(),
        "il lotto deve raccogliere il tracciato di partenza"
    );
}

#[test]
fn la_dashboard_riflette_quello_che_e_stato_indicizzato() {
    let Some(db) = scandisci_lotto(Fascia::Tracciati) else {
        return;
    };

    dedupe::calcola_hash_mancanti(&db).expect("hash");
    let s = cerca::statistiche(&db).expect("statistiche");

    assert!(s.file_totali > 0);
    assert!(s.byte_totali > 0);
    assert_eq!(
        s.file_totali,
        s.per_tipo
            .iter()
            .filter(|c| c.etichetta != "artefatto")
            .map(|c| c.quanti)
            .sum::<i64>(),
        "i conteggi per tipo devono quadrare col totale"
    );
}

/// La guardia repo è il differenziatore del prodotto: qui si verifica sul
/// filesystem vero, non su una fixture, che un PDF dentro un albero di codice
/// venga riconosciuto come artefatto.
#[test]
fn un_pdf_dentro_un_repo_e_un_artefatto() {
    let repo = "/home/russus/RiderProjects/phoenix_old";
    if !Path::new(repo).is_dir() {
        eprintln!("salto: «{repo}» non presente su questa macchina");
        return;
    }
    let radice = scan::dentro_repo(
        Path::new(&format!("{repo}/PhoenixTest/TestOutput/AccessibleListsTest.pdf")),
        None,
    );
    assert!(
        radice.is_some(),
        "un PDF sotto PhoenixTest/TestOutput deve risultare dentro un repository"
    );
}
