//! Tipi condivisi fra i moduli e serializzati verso il frontend.
//!
//! Le stringhe usate su SQLite e verso la UI sono in italiano e minuscole
//! (`documento`, `canonico`, …) così che l'indice resti leggibile a mano con
//! `sqlite3` senza doversi ricordare una mappatura numerica.

use serde::{Deserialize, Serialize};

/// Cosa *è* un file. Asse deterministico: si ricava da estensione, magic bytes
/// e struttura, mai dal contenuto semantico.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tipo {
    Documento,
    Tracciato,
    Archivio,
    Media,
    Installer,
    /// Output di build o fixture di test: vive dentro un repository di codice.
    Artefatto,
    Altro,
}

impl Tipo {
    pub fn as_str(&self) -> &'static str {
        match self {
            Tipo::Documento => "documento",
            Tipo::Tracciato => "tracciato",
            Tipo::Archivio => "archivio",
            Tipo::Media => "media",
            Tipo::Installer => "installer",
            Tipo::Artefatto => "artefatto",
            Tipo::Altro => "altro",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "documento" => Tipo::Documento,
            "tracciato" => Tipo::Tracciato,
            "archivio" => Tipo::Archivio,
            "media" => Tipo::Media,
            "installer" => Tipo::Installer,
            "artefatto" => Tipo::Artefatto,
            _ => Tipo::Altro,
        }
    }
}

/// Asse deterministico ricavato da hash e presenza sul disco.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stato {
    /// Copia eletta a riferimento del suo gruppo di duplicati.
    Canonico,
    /// Ha lo stesso contenuto di un file canonico.
    Duplicato,
    /// Archivio la cui cartella estratta esiste già accanto, installer superato.
    Orfano,
}

impl Stato {
    pub fn as_str(&self) -> &'static str {
        match self {
            Stato::Canonico => "canonico",
            Stato::Duplicato => "duplicato",
            Stato::Orfano => "orfano",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "duplicato" => Stato::Duplicato,
            "orfano" => Stato::Orfano,
            _ => Stato::Canonico,
        }
    }
}

/// Fascia di appartenenza di una sorgente, come definito in analisi:
/// A = documenti da indicizzare full-text, B = flussi con parser dedicato.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fascia {
    Documenti,
    Tracciati,
}

impl Fascia {
    pub fn as_str(&self) -> &'static str {
        match self {
            Fascia::Documenti => "documenti",
            Fascia::Tracciati => "tracciati",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "tracciati" => Fascia::Tracciati,
            _ => Fascia::Documenti,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sorgente {
    pub id: i64,
    pub path: String,
    pub fascia: Fascia,
    pub attiva: bool,
    pub ricorsiva: bool,
    /// Quando true il walker indicizza anche ciò che sta dentro un repository
    /// di codice. Serve per le cartelle che *sono* dentro un repo ma
    /// contengono documenti veri.
    pub ignora_repo_guard: bool,
}

/// Una riga dell'indice. `motivo_*` esiste perché ogni classificazione deve
/// poter essere spiegata: senza il "perché è finito qui" l'utente non si fida
/// mai del risultato.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub id: i64,
    pub path: String,
    pub nome: String,
    pub ext: Option<String>,
    pub size: i64,
    pub mtime: i64,
    pub hash: Option<String>,
    pub tipo: Tipo,
    pub contesto: Option<String>,
    pub stato: Stato,
    pub motivo_tipo: Option<String>,
    pub motivo_contesto: Option<String>,
    pub sorgente_id: i64,
    /// Path dell'archivio contenitore, se il file è stato indicizzato
    /// dall'interno di uno zip/tar senza estrarlo.
    pub archivio_padre: Option<String>,
    /// Codice del lotto di composizione documentale, se correlato.
    pub lotto: Option<String>,
    pub testo_estratto: bool,
    pub pagine: Option<i64>,
}

/// Un risultato di ricerca: il record più il frammento di testo che ha
/// prodotto il match.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Risultato {
    #[serde(flatten)]
    pub file: FileRecord,
    /// Frammento con i termini evidenziati fra `[[` e `]]`.
    pub snippet: String,
    /// Riga del tracciato in cui cade il match, quando applicabile.
    pub riga: Option<i64>,
    /// Pagina del PDF in cui cade il match, quando applicabile.
    pub pagina: Option<i64>,
    pub rank: f64,
}

/// Filtri componibili della schermata di ricerca. Ogni campo `None`/vuoto
/// significa "non filtrare su questo asse".
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Filtri {
    pub tipi: Vec<String>,
    pub contesti: Vec<String>,
    pub stati: Vec<String>,
    pub sorgenti: Vec<i64>,
    pub estensioni: Vec<String>,
    /// Epoch secondi.
    pub da_data: Option<i64>,
    pub a_data: Option<i64>,
    pub size_min: Option<i64>,
    pub size_max: Option<i64>,
    /// Include i file classificati `artefatto`, esclusi di default.
    pub includi_artefatti: bool,
    /// `rilevanza` (predefinito) | `dimensione` | `recenti` | `vecchi` | `nome`.
    /// Un valore non riconosciuto vale come predefinito: un ordinamento
    /// sbagliato non deve far fallire una ricerca.
    pub ordine: Option<String>,
    pub limite: Option<i64>,
    pub offset: Option<i64>,
}

/// Una regola del motore di categorizzazione.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Regola {
    pub id: i64,
    pub nome: String,
    /// `tipo` oppure `contesto`.
    pub asse: String,
    /// Glob sul path completo (`**/T1Q*/**`) o sul solo nome file (`CV_*`).
    pub pattern: String,
    pub valore: String,
    /// Più basso = valutata prima.
    pub priorita: i64,
    pub attiva: bool,
    /// Regola di serie, non cancellabile ma disattivabile.
    pub builtin: bool,
}

/// Definizione di un campo dentro un record a lunghezza fissa.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Campo {
    pub nome: String,
    /// Offset in caratteri, 0-based.
    pub offset: usize,
    pub lunghezza: usize,
    /// `testo` | `numero` | `data`
    pub tipo: String,
}

/// Profilo di record layout riutilizzabile fra lotti diversi.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layout {
    pub id: i64,
    pub nome: String,
    pub lunghezza_record: usize,
    pub campi: Vec<Campo>,
}

/// Esito dell'auto-detect su un file a record fissi.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutCandidato {
    pub lunghezza_record: usize,
    pub numero_record: usize,
    /// Intervalli di colonne che restano stabili su tutti i record: sono i
    /// confini di campo più probabili.
    pub colonne_stabili: Vec<(usize, usize)>,
    /// 0.0–1.0, quanto è regolare il file.
    pub confidenza: f64,
}

/// Un lotto di composizione documentale: il tracciato più i file che ne
/// derivano.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lotto {
    pub codice: String,
    pub cartella: String,
    pub tracciati: Vec<FileRecord>,
    pub pdf: Vec<FileRecord>,
    pub altri: Vec<FileRecord>,
}

/// Un gruppo di file con contenuto identico (o testo identico, per i PDF).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GruppoDuplicati {
    pub chiave: String,
    /// `esatto` (stesso hash) oppure `testo` (stesso testo estratto).
    pub genere: String,
    pub canonico: FileRecord,
    pub duplicati: Vec<FileRecord>,
    /// Byte recuperabili eliminando i duplicati.
    pub spazio_recuperabile: i64,
}

/// Una mossa proposta. Nessuna operazione tocca il disco prima che l'utente
/// abbia visto il piano.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mossa {
    pub file_id: i64,
    pub origine: String,
    pub destinazione: String,
    /// `quarantena` | `sposta` | `rimuovi_cartella` | `cestino` | `elimina`.
    /// Gli ultimi due non hanno destinazione e non sono annullabili da qui:
    /// vedi [`crate::cestino`].
    pub genere: String,
    pub motivo: String,
    /// Falso quando la mossa verrebbe saltata (destinazione occupata,
    /// filesystem diverso, sorgente sparita).
    pub eseguibile: bool,
    pub avviso: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PianoOperazioni {
    pub batch: String,
    pub mosse: Vec<Mossa>,
    pub spazio_liberato: i64,
    pub eseguibili: usize,
    pub saltate: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EsitoOperazioni {
    pub batch: String,
    pub eseguite: usize,
    pub fallite: usize,
    pub errori: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Batch {
    pub batch: String,
    pub genere: String,
    pub quante: i64,
    pub eseguito_il: i64,
    pub annullato: bool,
    /// Falso per i batch che hanno consegnato i file al cestino di sistema o
    /// li hanno cancellati: da Setaccio non si torna indietro, e l'elenco dei
    /// batch deve dirlo invece di offrire un pulsante che fallirebbe.
    pub annullabile: bool,
}

/// Avanzamento della scansione, emesso come evento `scan://progresso`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Progresso {
    pub in_corso: bool,
    pub fase: String,
    pub path_corrente: String,
    pub visti: u64,
    pub indicizzati: u64,
    pub saltati_repo: u64,
    pub estratti: u64,
    pub errori: u64,
    pub finito: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConteggioEtichetta {
    pub etichetta: String,
    pub quanti: i64,
    pub byte: i64,
}

/// Dati della dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Statistiche {
    pub file_totali: i64,
    pub byte_totali: i64,
    pub documenti: i64,
    pub tracciati: i64,
    pub artefatti_esclusi: i64,
    pub non_classificati: i64,
    pub gruppi_duplicati: i64,
    pub spazio_duplicati: i64,
    pub per_tipo: Vec<ConteggioEtichetta>,
    pub per_contesto: Vec<ConteggioEtichetta>,
    pub ultima_scansione: Option<i64>,
}

/// Una cartella con quanto pesa l'albero che ha sotto.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartellaPesante {
    pub path: String,
    /// Ultimo segmento del percorso: è quello che si legge nella lista.
    pub nome: String,
    /// Byte di tutto ciò che sta sotto, sottocartelle comprese.
    pub byte: i64,
    pub quanti: i64,
    /// Byte dei soli file che stanno direttamente dentro questa cartella.
    /// La differenza con `byte` dice se il peso è qui o più in basso.
    pub byte_diretti: i64,
    /// Quanti separatori ha il percorso: serve alla UI per rientrare le
    /// cartelle annidate invece di mostrarle tutte allo stesso livello.
    pub profondita: usize,
}

/// Dati della vista Ingombro: dove sono finiti i gigabyte.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ingombro {
    /// I file più grandi che passano i filtri, dal più pesante.
    pub file: Vec<FileRecord>,
    /// Le cartelle più pesanti, già sfoltite dei passaggi che non aggiungono
    /// nulla rispetto al padre.
    pub cartelle: Vec<CartellaPesante>,
    pub per_estensione: Vec<ConteggioEtichetta>,
    pub per_tipo: Vec<ConteggioEtichetta>,
    /// Totali su *tutto* ciò che passa i filtri, non solo su quanto è in `file`.
    pub byte_totali: i64,
    pub quanti_totali: i64,
    /// Byte dei soli file elencati in `file`: serve a dire «i primi 50
    /// pesano il 60% del totale», che è l'informazione che fa agire.
    pub byte_mostrati: i64,
}

/// Riga della coda di revisione: un file che le regole non hanno saputo
/// collocare, con i suggerimenti su cui l'utente può creare una regola nuova.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaRevisionare {
    #[serde(flatten)]
    pub file: FileRecord,
    /// Pattern proposti, dal più specifico al più generale.
    pub pattern_suggeriti: Vec<String>,
    pub contesti_vicini: Vec<String>,
}

/// Anteprima del contenuto mostrata nel pannello di destra.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anteprima {
    /// `testo` | `record` | `immagine` | `nessuna`
    pub genere: String,
    pub testo: Option<String>,
    /// Righe del tracciato già spezzate nei campi del layout attivo.
    pub record: Option<Vec<Vec<String>>>,
    pub intestazioni: Option<Vec<String>>,
    pub pagine: Option<i64>,
    pub pagina: Option<i64>,
    pub messaggio: Option<String>,
}
