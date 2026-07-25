//! Setaccio — indicizza, categorizza e cerca i file locali.
//!
//! Il principio che tiene insieme il tutto: **il default non tocca mai il
//! disco**. La scansione legge, l'indice vive a parte, e ogni operazione che
//! tocca un file passa da un piano mostrato all'utente prima di partire.
//!
//! Le operazioni che spostano sono annullabili ([`organize`]); quelle che
//! tolgono davvero — cestino di sistema e cancellazione — non lo sono, stanno
//! tutte in [`cestino`] e vanno chieste esplicitamente ogni volta.

pub mod cerca;
pub mod cestino;
pub mod classify;
pub mod db;
pub mod dedupe;
pub mod extract;
pub mod ingombro;
pub mod organize;
pub mod scan;
pub mod tracciati;
pub mod types;

use std::sync::Arc;
use tauri::{Emitter, Manager};

use db::Db;
use types::*;

pub struct Stato0 {
    pub db: Arc<Db>,
    pub scansione: Arc<scan::Controllo>,
}

/// Gli errori arrivano al frontend come stringhe: la UI li mostra in un
/// riquadro, non deve distinguerne il tipo.
type Esito<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

// ---------------------------------------------------------------------------
// Sorgenti
// ---------------------------------------------------------------------------

#[tauri::command]
fn sorgenti_lista(stato: tauri::State<'_, Stato0>) -> Esito<Vec<Sorgente>> {
    stato.db.sorgenti().map_err(err)
}

#[tauri::command]
fn sorgente_aggiungi(stato: tauri::State<'_, Stato0>, path: String, fascia: String) -> Esito<i64> {
    let p = std::path::Path::new(&path);
    if !p.is_dir() {
        return Err(format!("«{path}» non è una cartella esistente"));
    }
    stato
        .db
        .sorgente_aggiungi(&path, Fascia::from_str(&fascia))
        .map_err(err)
}

#[tauri::command]
fn sorgente_rimuovi(stato: tauri::State<'_, Stato0>, id: i64) -> Esito<()> {
    stato.db.sorgente_rimuovi(id).map_err(err)
}

#[tauri::command]
fn sorgente_aggiorna(stato: tauri::State<'_, Stato0>, sorgente: Sorgente) -> Esito<()> {
    stato.db.sorgente_aggiorna(&sorgente).map_err(err)
}

/// Sorgenti proposte alla prima apertura, ricavate dall'analisi del filesystem.
#[tauri::command]
fn sorgenti_suggerite() -> Vec<(String, String)> {
    scan::sorgenti_suggerite()
}

// ---------------------------------------------------------------------------
// Scansione
// ---------------------------------------------------------------------------

#[tauri::command]
fn scan_avvia(app: tauri::AppHandle, stato: tauri::State<'_, Stato0>) -> Esito<()> {
    let db = stato.db.clone();
    let controllo = stato.scansione.clone();
    if controllo.in_corso() {
        return Err("una scansione è già in corso".into());
    }
    let app_progresso = app.clone();
    std::thread::spawn(move || {
        let esito = scan::esegui(db, controllo.clone(), move |p| {
            let _ = app_progresso.emit("scan://progresso", p);
        });
        if let Err(e) = esito {
            let _ = app.emit("scan://errore", e.to_string());
        }
    });
    Ok(())
}

#[tauri::command]
fn scan_ferma(stato: tauri::State<'_, Stato0>) -> Esito<()> {
    stato.scansione.ferma();
    Ok(())
}

#[tauri::command]
fn scan_progresso(stato: tauri::State<'_, Stato0>) -> Progresso {
    stato.scansione.progresso()
}

// ---------------------------------------------------------------------------
// Ricerca
// ---------------------------------------------------------------------------

#[tauri::command]
fn cerca(stato: tauri::State<'_, Stato0>, query: String, filtri: Filtri) -> Esito<Vec<Risultato>> {
    cerca::full_text(&stato.db, &query, &filtri).map_err(err)
}

#[tauri::command]
fn cerca_nome(stato: tauri::State<'_, Stato0>, query: String, filtri: Filtri) -> Esito<Vec<FileRecord>> {
    cerca::per_nome(&stato.db, &query, &filtri).map_err(err)
}

#[tauri::command]
fn file_dettaglio(stato: tauri::State<'_, Stato0>, id: i64) -> Esito<Option<FileRecord>> {
    stato.db.file_per_id(id).map_err(err)
}

#[tauri::command]
fn anteprima(stato: tauri::State<'_, Stato0>, id: i64, pagina: Option<i64>) -> Esito<Anteprima> {
    cerca::anteprima(&stato.db, id, pagina).map_err(err)
}

#[tauri::command]
fn faccette(stato: tauri::State<'_, Stato0>) -> Esito<Vec<ConteggioEtichetta>> {
    cerca::faccette_contesto(&stato.db).map_err(err)
}

// ---------------------------------------------------------------------------
// Ingombro — dove sono finiti i gigabyte
// ---------------------------------------------------------------------------

/// File più grandi, cartelle più pesanti e ripartizione per estensione, tutto
/// sugli stessi filtri. `limite` vale solo per l'elenco dei file: i totali si
/// calcolano su tutto ciò che passa i filtri.
#[tauri::command]
fn ingombro(stato: tauri::State<'_, Stato0>, filtri: Filtri, limite: i64) -> Esito<Ingombro> {
    ingombro::ingombro(&stato.db, &filtri, limite).map_err(err)
}

// ---------------------------------------------------------------------------
// Dashboard e revisione
// ---------------------------------------------------------------------------

#[tauri::command]
fn statistiche(stato: tauri::State<'_, Stato0>) -> Esito<Statistiche> {
    cerca::statistiche(&stato.db).map_err(err)
}

#[tauri::command]
fn novita(stato: tauri::State<'_, Stato0>, giorni: i64) -> Esito<Vec<FileRecord>> {
    cerca::novita(&stato.db, giorni).map_err(err)
}

#[tauri::command]
fn coda_revisione(stato: tauri::State<'_, Stato0>, limite: i64) -> Esito<Vec<DaRevisionare>> {
    classify::coda_revisione(&stato.db, limite).map_err(err)
}

#[tauri::command]
fn regole_lista(stato: tauri::State<'_, Stato0>) -> Esito<Vec<Regola>> {
    stato.db.regole().map_err(err)
}

#[tauri::command]
fn regola_aggiungi(
    stato: tauri::State<'_, Stato0>,
    nome: String,
    asse: String,
    pattern: String,
    valore: String,
    priorita: i64,
) -> Esito<i64> {
    let id = stato
        .db
        .regola_aggiungi(&nome, &asse, &pattern, &valore, priorita)
        .map_err(err)?;
    // Una regola nuova va applicata subito a ciò che è già indicizzato,
    // altrimenti l'utente non vede l'effetto di quello che ha appena scritto.
    classify::riapplica(&stato.db).map_err(err)?;
    Ok(id)
}

/// Quanti file dell'indice colpirebbe un pattern, con qualche esempio. Serve
/// a scrivere una regola sapendo cosa fa, invece di scoprirlo dopo.
#[tauri::command]
fn regola_stima(stato: tauri::State<'_, Stato0>, pattern: String) -> Esito<(i64, Vec<String>)> {
    classify::stima_pattern(&stato.db, &pattern).map_err(err)
}

#[tauri::command]
fn regola_elimina(stato: tauri::State<'_, Stato0>, id: i64) -> Esito<()> {
    stato.db.regola_elimina(id).map_err(err)?;
    classify::riapplica(&stato.db).map_err(err)
}

#[tauri::command]
fn regola_attiva(stato: tauri::State<'_, Stato0>, id: i64, attiva: bool) -> Esito<()> {
    stato.db.regola_attiva(id, attiva).map_err(err)?;
    classify::riapplica(&stato.db).map_err(err)
}

// ---------------------------------------------------------------------------
// Duplicati
// ---------------------------------------------------------------------------

#[tauri::command]
fn duplicati(stato: tauri::State<'_, Stato0>) -> Esito<Vec<GruppoDuplicati>> {
    dedupe::gruppi(&stato.db).map_err(err)
}

#[tauri::command]
fn duplicati_piano(stato: tauri::State<'_, Stato0>, file_ids: Vec<i64>) -> Esito<PianoOperazioni> {
    organize::piano_quarantena(&stato.db, &file_ids).map_err(err)
}

// ---------------------------------------------------------------------------
// Operazioni sul filesystem — sempre in due tempi: piano, poi esecuzione
// ---------------------------------------------------------------------------

#[tauri::command]
fn operazioni_esegui(stato: tauri::State<'_, Stato0>, piano: PianoOperazioni) -> Esito<EsitoOperazioni> {
    organize::esegui(&stato.db, &piano).map_err(err)
}

/// Piano per consegnare i file al cestino di sistema. Restano recuperabili dal
/// gestore file, ma **non** dall'undo di Setaccio: il piano è l'ultimo punto
/// in cui ci si può fermare.
#[tauri::command]
fn cestino_piano(stato: tauri::State<'_, Stato0>, file_ids: Vec<i64>) -> Esito<PianoOperazioni> {
    cestino::piano_cestino(&stato.db, &file_ids).map_err(err)
}

/// Piano per cancellare i file senza passare dal cestino. Irreversibile.
#[tauri::command]
fn elimina_piano(stato: tauri::State<'_, Stato0>, file_ids: Vec<i64>) -> Esito<PianoOperazioni> {
    cestino::piano_elimina(&stato.db, &file_ids).map_err(err)
}

#[tauri::command]
fn operazioni_batch(stato: tauri::State<'_, Stato0>) -> Esito<Vec<Batch>> {
    organize::batch_recenti(&stato.db).map_err(err)
}

#[tauri::command]
fn operazioni_annulla(stato: tauri::State<'_, Stato0>, batch: String) -> Esito<EsitoOperazioni> {
    organize::annulla(&stato.db, &batch).map_err(err)
}

#[tauri::command]
fn organizza_piano(stato: tauri::State<'_, Stato0>, destinazione: String, contesti: Vec<String>) -> Esito<PianoOperazioni> {
    organize::piano_organizza(&stato.db, &destinazione, &contesti).map_err(err)
}

/// Cartelle rimaste vuote sotto le radici indicate (vuoto = tutte le sorgenti
/// più la quarantena). È l'unica operazione che cancella qualcosa, e cancella
/// solo contenitori vuoti: `remove_dir` fallisce se dentro c'è ancora un file.
/// Come ogni altra operazione passa dal piano ed è annullabile.
#[tauri::command]
fn cartelle_vuote_piano(stato: tauri::State<'_, Stato0>, radici: Vec<String>) -> Esito<PianoOperazioni> {
    organize::piano_cartelle_vuote(&stato.db, &radici).map_err(err)
}

/// Cartella da proporre come destinazione: la sorgente che contiene già la
/// maggior parte dei file di quei contesti. Evita all'utente di cercare a mano
/// una cartella che quasi sempre è la sorgente stessa.
#[tauri::command]
fn organizza_destinazione(stato: tauri::State<'_, Stato0>, contesti: Vec<String>) -> Esito<Option<String>> {
    organize::destinazione_suggerita(&stato.db, &contesti).map_err(err)
}

// ---------------------------------------------------------------------------
// Tracciati e lotti
// ---------------------------------------------------------------------------

#[tauri::command]
fn lotti(stato: tauri::State<'_, Stato0>) -> Esito<Vec<Lotto>> {
    tracciati::lotti(&stato.db).map_err(err)
}

#[tauri::command]
fn lotto_dettaglio(stato: tauri::State<'_, Stato0>, codice: String) -> Esito<Option<Lotto>> {
    tracciati::lotto(&stato.db, &codice).map_err(err)
}

#[tauri::command]
fn layout_detect(path: String) -> Esito<LayoutCandidato> {
    tracciati::detect(std::path::Path::new(&path)).map_err(err)
}

#[tauri::command]
fn layout_lista(stato: tauri::State<'_, Stato0>) -> Esito<Vec<Layout>> {
    tracciati::layout_lista(&stato.db).map_err(err)
}

#[tauri::command]
fn layout_salva(stato: tauri::State<'_, Stato0>, nome: String, lunghezza_record: usize, campi: Vec<Campo>) -> Esito<i64> {
    tracciati::layout_salva(&stato.db, &nome, lunghezza_record, &campi).map_err(err)
}

#[tauri::command]
fn layout_elimina(stato: tauri::State<'_, Stato0>, id: i64) -> Esito<()> {
    tracciati::layout_elimina(&stato.db, id).map_err(err)
}

/// Applica un layout a un tracciato e restituisce i record spezzati nei campi.
#[tauri::command]
fn tracciato_record(
    stato: tauri::State<'_, Stato0>,
    file_id: i64,
    layout_id: Option<i64>,
    da: usize,
    quanti: usize,
) -> Esito<Anteprima> {
    tracciati::record(&stato.db, file_id, layout_id, da, quanti).map_err(err)
}

// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let percorso = db::percorso_db()?;
            let db = Arc::new(Db::apri(&percorso)?);
            app.manage(Stato0 {
                db,
                scansione: Arc::new(scan::Controllo::nuovo()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            sorgenti_lista,
            sorgente_aggiungi,
            sorgente_rimuovi,
            sorgente_aggiorna,
            sorgenti_suggerite,
            scan_avvia,
            scan_ferma,
            scan_progresso,
            cerca,
            cerca_nome,
            file_dettaglio,
            anteprima,
            faccette,
            ingombro,
            statistiche,
            novita,
            coda_revisione,
            regole_lista,
            regola_aggiungi,
            regola_stima,
            regola_elimina,
            regola_attiva,
            duplicati,
            duplicati_piano,
            cestino_piano,
            elimina_piano,
            operazioni_esegui,
            operazioni_batch,
            operazioni_annulla,
            organizza_piano,
            organizza_destinazione,
            cartelle_vuote_piano,
            lotti,
            lotto_dettaglio,
            layout_detect,
            layout_lista,
            layout_salva,
            layout_elimina,
            tracciato_record,
        ])
        .run(tauri::generate_context!())
        .expect("errore nell'avvio di Setaccio");
}
