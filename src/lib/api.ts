/**
 * Ponte tipizzato verso il backend Rust.
 *
 * I tipi qui sotto rispecchiano `src-tauri/src/types.rs`: le enum sono
 * serializzate in minuscolo (`#[serde(rename_all = "lowercase")]`), mentre i
 * *campi* delle struct mantengono lo snake_case di Rust perché serde non li
 * rinomina. Attenzione alla differenza: gli **argomenti dei comandi** invece
 * Tauri li converte in camelCase (`file_ids` → `fileIds`, `layout_id` →
 * `layoutId`), mentre i **nomi dei comandi** restano identici a come sono
 * dichiarati in `lib.rs`.
 *
 * Ogni funzione esportata corrisponde a un `#[tauri::command]` esistente:
 * qui non c'è nulla di inventato.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/* ==========================================================================
   Tipi — rispecchiano src-tauri/src/types.rs
   ========================================================================== */

/** Cosa *è* un file. Asse deterministico. */
export type Tipo =
  | "documento"
  | "tracciato"
  | "archivio"
  | "media"
  | "installer"
  | "artefatto"
  | "altro";

/** Asse deterministico ricavato da hash e presenza sul disco. */
export type Stato = "canonico" | "duplicato" | "orfano";

/** Fascia di appartenenza di una sorgente. */
export type Fascia = "documenti" | "tracciati";

/** Elenchi pronti per menu e filtri, nell'ordine in cui vanno mostrati. */
export const TIPI: readonly Tipo[] = [
  "documento",
  "tracciato",
  "archivio",
  "media",
  "installer",
  "artefatto",
  "altro",
];

export const STATI: readonly Stato[] = ["canonico", "duplicato", "orfano"];

export const FASCE: readonly Fascia[] = ["documenti", "tracciati"];

export interface Sorgente {
  id: number;
  path: string;
  fascia: Fascia;
  attiva: boolean;
  ricorsiva: boolean;
  /** Indicizza anche ciò che sta dentro un repository di codice. */
  ignora_repo_guard: boolean;
}

/** Una riga dell'indice. */
export interface FileRecord {
  id: number;
  path: string;
  nome: string;
  ext: string | null;
  size: number;
  /** Epoch secondi. */
  mtime: number;
  hash: string | null;
  tipo: Tipo;
  contesto: string | null;
  stato: Stato;
  motivo_tipo: string | null;
  motivo_contesto: string | null;
  sorgente_id: number;
  /** Path dell'archivio contenitore, se indicizzato da dentro uno zip/tar. */
  archivio_padre: string | null;
  /** Codice del lotto di composizione documentale, se correlato. */
  lotto: string | null;
  testo_estratto: boolean;
  pagine: number | null;
}

/**
 * Un risultato di ricerca. In Rust `file` è `#[serde(flatten)]`, quindi in
 * JavaScript i campi del record arrivano al livello principale dell'oggetto.
 */
export interface Risultato extends FileRecord {
  /** Frammento con i termini evidenziati fra `[[` e `]]`. */
  snippet: string;
  riga: number | null;
  pagina: number | null;
  rank: number;
}

/**
 * Filtri componibili della ricerca. Ogni array vuoto e ogni `null`
 * significano «non filtrare su questo asse».
 */
export interface Filtri {
  tipi: string[];
  contesti: string[];
  stati: string[];
  sorgenti: number[];
  estensioni: string[];
  /** Epoch secondi. */
  da_data: number | null;
  a_data: number | null;
  size_min: number | null;
  size_max: number | null;
  /** Include i file classificati `artefatto`, esclusi di default. */
  includi_artefatti: boolean;
  limite: number | null;
  offset: number | null;
}

/** Filtri vuoti, da usare come base a cui applicare le modifiche. */
export function filtriVuoti(): Filtri {
  return {
    tipi: [],
    contesti: [],
    stati: [],
    sorgenti: [],
    estensioni: [],
    da_data: null,
    a_data: null,
    size_min: null,
    size_max: null,
    includi_artefatti: false,
    limite: null,
    offset: null,
  };
}

/** Una regola del motore di categorizzazione. */
export interface Regola {
  id: number;
  nome: string;
  /** `tipo` oppure `contesto`. */
  asse: string;
  /** Glob sul path completo (`**​/T1Q*​/**`) o sul solo nome file (`CV_*`). */
  pattern: string;
  valore: string;
  /** Più basso = valutata prima. */
  priorita: number;
  attiva: boolean;
  /** Regola di serie: non cancellabile, ma disattivabile. */
  builtin: boolean;
}

/** Definizione di un campo dentro un record a lunghezza fissa. */
export interface Campo {
  nome: string;
  /** Offset in caratteri, 0-based. */
  offset: number;
  lunghezza: number;
  /** `testo` | `numero` | `data` */
  tipo: string;
}

/** Profilo di record layout riutilizzabile fra lotti diversi. */
export interface Layout {
  id: number;
  nome: string;
  lunghezza_record: number;
  campi: Campo[];
}

/** Esito dell'auto-detect su un file a record fissi. */
export interface LayoutCandidato {
  lunghezza_record: number;
  numero_record: number;
  /** Intervalli `[inizio, fine]` di colonne stabili su tutti i record. */
  colonne_stabili: [number, number][];
  /** 0.0–1.0, quanto è regolare il file. */
  confidenza: number;
}

/** Un lotto di composizione documentale. */
export interface Lotto {
  codice: string;
  cartella: string;
  tracciati: FileRecord[];
  pdf: FileRecord[];
  altri: FileRecord[];
}

/** Un gruppo di file con contenuto identico. */
export interface GruppoDuplicati {
  chiave: string;
  /** `esatto` (stesso hash) oppure `testo` (stesso testo estratto). */
  genere: string;
  canonico: FileRecord;
  duplicati: FileRecord[];
  /** Byte recuperabili eliminando i duplicati. */
  spazio_recuperabile: number;
}

/** Una mossa proposta: nulla tocca il disco prima del piano. */
export interface Mossa {
  file_id: number;
  origine: string;
  destinazione: string;
  /** `quarantena` | `sposta` */
  genere: string;
  motivo: string;
  eseguibile: boolean;
  avviso: string | null;
}

export interface PianoOperazioni {
  batch: string;
  mosse: Mossa[];
  spazio_liberato: number;
  eseguibili: number;
  saltate: number;
}

export interface EsitoOperazioni {
  batch: string;
  eseguite: number;
  fallite: number;
  errori: string[];
}

export interface Batch {
  batch: string;
  genere: string;
  quante: number;
  /** Epoch secondi. */
  eseguito_il: number;
  annullato: boolean;
}

/** Avanzamento della scansione. */
export interface Progresso {
  in_corso: boolean;
  fase: string;
  path_corrente: string;
  visti: number;
  indicizzati: number;
  saltati_repo: number;
  estratti: number;
  errori: number;
  finito: boolean;
}

export interface ConteggioEtichetta {
  etichetta: string;
  quanti: number;
  byte: number;
}

/** Dati della dashboard. */
export interface Statistiche {
  file_totali: number;
  byte_totali: number;
  documenti: number;
  tracciati: number;
  artefatti_esclusi: number;
  non_classificati: number;
  gruppi_duplicati: number;
  spazio_duplicati: number;
  per_tipo: ConteggioEtichetta[];
  per_contesto: ConteggioEtichetta[];
  /** Epoch secondi. */
  ultima_scansione: number | null;
}

/** Riga della coda di revisione (anche qui il record è flattenato). */
export interface DaRevisionare extends FileRecord {
  /** Pattern proposti, dal più specifico al più generale. */
  pattern_suggeriti: string[];
  contesti_vicini: string[];
}

/** Anteprima del contenuto mostrata nel pannello di destra. */
export interface Anteprima {
  /** `testo` | `record` | `immagine` | `nessuna` */
  genere: string;
  testo: string | null;
  /** Righe del tracciato già spezzate nei campi del layout attivo. */
  record: string[][] | null;
  intestazioni: string[] | null;
  pagine: number | null;
  pagina: number | null;
  messaggio: string | null;
}

/* ==========================================================================
   Chiamata di base
   ========================================================================== */

/**
 * Il backend restituisce gli errori come stringhe. Qui diventano `Error`
 * con un messaggio già leggibile, così le viste possono limitarsi a
 * mostrarne `.message`.
 */
export async function chiama<T>(
  comando: string,
  argomenti?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(comando, argomenti);
  } catch (e) {
    throw new Error(typeof e === "string" ? e : String(e));
  }
}

/* ==========================================================================
   Sorgenti
   ========================================================================== */

export const sorgentiLista = () => chiama<Sorgente[]>("sorgenti_lista");

export const sorgenteAggiungi = (path: string, fascia: Fascia) =>
  chiama<number>("sorgente_aggiungi", { path, fascia });

export const sorgenteRimuovi = (id: number) =>
  chiama<void>("sorgente_rimuovi", { id });

export const sorgenteAggiorna = (sorgente: Sorgente) =>
  chiama<void>("sorgente_aggiorna", { sorgente });

/** Coppie `[path, fascia]` proposte alla prima apertura. */
export const sorgentiSuggerite = () =>
  chiama<[string, string][]>("sorgenti_suggerite");

/* ==========================================================================
   Scansione
   ========================================================================== */

export const scanAvvia = () => chiama<void>("scan_avvia");

export const scanFerma = () => chiama<void>("scan_ferma");

export const scanProgresso = () => chiama<Progresso>("scan_progresso");

/** Evento emesso a ogni passo della scansione. */
export const EVENTO_PROGRESSO = "scan://progresso";
/** Evento emesso quando la scansione si interrompe per errore. */
export const EVENTO_ERRORE_SCANSIONE = "scan://errore";

export function ascoltaProgresso(
  cb: (p: Progresso) => void,
): Promise<UnlistenFn> {
  return listen<Progresso>(EVENTO_PROGRESSO, (e) => cb(e.payload));
}

export function ascoltaErroreScansione(
  cb: (messaggio: string) => void,
): Promise<UnlistenFn> {
  return listen<string>(EVENTO_ERRORE_SCANSIONE, (e) => cb(e.payload));
}

/* ==========================================================================
   Ricerca
   ========================================================================== */

/** Ricerca full-text sul testo estratto. */
export const cerca = (query: string, filtri: Filtri = filtriVuoti()) =>
  chiama<Risultato[]>("cerca", { query, filtri });

/** Ricerca sul solo nome/percorso del file. */
export const cercaNome = (query: string, filtri: Filtri = filtriVuoti()) =>
  chiama<FileRecord[]>("cerca_nome", { query, filtri });

export const fileDettaglio = (id: number) =>
  chiama<FileRecord | null>("file_dettaglio", { id });

export const anteprima = (id: number, pagina: number | null = null) =>
  chiama<Anteprima>("anteprima", { id, pagina });

/** Conteggi per contesto, per le faccette della ricerca. */
export const faccette = () => chiama<ConteggioEtichetta[]>("faccette");

/* ==========================================================================
   Dashboard e revisione
   ========================================================================== */

export const statistiche = () => chiama<Statistiche>("statistiche");

export const novita = (giorni: number) =>
  chiama<FileRecord[]>("novita", { giorni });

export const codaRevisione = (limite: number) =>
  chiama<DaRevisionare[]>("coda_revisione", { limite });

export const regoleLista = () => chiama<Regola[]>("regole_lista");

export const regolaAggiungi = (
  nome: string,
  asse: string,
  pattern: string,
  valore: string,
  priorita: number,
) =>
  chiama<number>("regola_aggiungi", {
    nome,
    asse,
    pattern,
    valore,
    priorita,
  });

/**
 * Quanti file colpirebbe un pattern, e fino a cinque percorsi di esempio.
 *
 * Conta con lo stesso matcher del motore, quindi il numero è esatto e vale
 * anche per i pattern sul percorso. Un pattern non compilabile è un errore,
 * non uno zero.
 */
export const regolaStima = (pattern: string) =>
  chiama<[number, string[]]>("regola_stima", { pattern });

export const regolaElimina = (id: number) =>
  chiama<void>("regola_elimina", { id });

export const regolaAttiva = (id: number, attiva: boolean) =>
  chiama<void>("regola_attiva", { id, attiva });

/* ==========================================================================
   Duplicati
   ========================================================================== */

export const duplicati = () => chiama<GruppoDuplicati[]>("duplicati");

/** `file_ids` in Rust → `fileIds` come argomento del comando. */
export const duplicatiPiano = (fileIds: number[]) =>
  chiama<PianoOperazioni>("duplicati_piano", { fileIds });

/* ==========================================================================
   Operazioni sul filesystem — sempre in due tempi: piano, poi esecuzione
   ========================================================================== */

export const operazioniEsegui = (piano: PianoOperazioni) =>
  chiama<EsitoOperazioni>("operazioni_esegui", { piano });

export const operazioniBatch = () => chiama<Batch[]>("operazioni_batch");

export const operazioniAnnulla = (batch: string) =>
  chiama<EsitoOperazioni>("operazioni_annulla", { batch });

export const organizzaPiano = (destinazione: string, contesti: string[]) =>
  chiama<PianoOperazioni>("organizza_piano", { destinazione, contesti });

/**
 * Le cartelle rimaste vuote sotto le radici indicate, come piano da
 * confermare. `radici` vuoto significa «tutte le sorgenti attive più la
 * cartella di quarantena».
 *
 * Le mosse hanno `genere: "rimuovi_cartella"`, `origine` con il percorso della
 * cartella e `destinazione` vuota: non si sposta niente, si toglie il
 * contenitore. `spazio_liberato` resta 0, perché una cartella vuota non occupa
 * spazio: qui il guadagno è l'ordine.
 */
export const cartelleVuotePiano = (radici: string[] = []) =>
  chiama<PianoOperazioni>("cartelle_vuote_piano", { radici });

/**
 * Cartella da proporre come destinazione: la sorgente che contiene già la
 * maggior parte dei file di quei contesti. `null` quando non c'è nulla da cui
 * dedurla (indice vuoto). Con la lista dei contesti vuota guarda tutto l'indice.
 */
export const organizzaDestinazione = (contesti: string[]) =>
  chiama<string | null>("organizza_destinazione", { contesti });

/* ==========================================================================
   Tracciati e lotti
   ========================================================================== */

export const lotti = () => chiama<Lotto[]>("lotti");

export const lottoDettaglio = (codice: string) =>
  chiama<Lotto | null>("lotto_dettaglio", { codice });

export const layoutDetect = (path: string) =>
  chiama<LayoutCandidato>("layout_detect", { path });

export const layoutLista = () => chiama<Layout[]>("layout_lista");

/** `lunghezza_record` in Rust → `lunghezzaRecord` come argomento. */
export const layoutSalva = (
  nome: string,
  lunghezzaRecord: number,
  campi: Campo[],
) => chiama<number>("layout_salva", { nome, lunghezzaRecord, campi });

export const layoutElimina = (id: number) =>
  chiama<void>("layout_elimina", { id });

/** `file_id`/`layout_id` in Rust → `fileId`/`layoutId` come argomenti. */
export const tracciatoRecord = (
  fileId: number,
  layoutId: number | null,
  da: number,
  quanti: number,
) => chiama<Anteprima>("tracciato_record", { fileId, layoutId, da, quanti });

/* ==========================================================================
   Formattatori — piccoli aiuti condivisi, per non riscriverli in ogni vista
   ========================================================================== */

const UNITA = ["B", "KB", "MB", "GB", "TB", "PB"];

/** Byte in forma leggibile, con lo spazio unificatore prima dell'unità. */
export function formattaByte(byte: number, decimali = 1): string {
  if (!Number.isFinite(byte) || byte <= 0) return "0 B";
  const i = Math.min(Math.floor(Math.log(byte) / Math.log(1024)), UNITA.length - 1);
  const v = byte / Math.pow(1024, i);
  const d = i === 0 ? 0 : v >= 100 ? 0 : decimali;
  return `${v.toFixed(d).replace(".", ",")} ${UNITA[i]}`;
}

/** Numero con separatore delle migliaia all'italiana. */
export function formattaNumero(n: number): string {
  return new Intl.NumberFormat("it-IT").format(n);
}

/** Epoch secondi → `12 mar 2026`. */
export function formattaData(epochSecondi: number | null | undefined): string {
  if (epochSecondi == null) return "—";
  return new Intl.DateTimeFormat("it-IT", {
    day: "2-digit",
    month: "short",
    year: "numeric",
  }).format(new Date(epochSecondi * 1000));
}

/** Epoch secondi → `12 mar 2026, 14:03`. */
export function formattaDataOra(epochSecondi: number | null | undefined): string {
  if (epochSecondi == null) return "—";
  return new Intl.DateTimeFormat("it-IT", {
    day: "2-digit",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(epochSecondi * 1000));
}

/** Scorcia un percorso lungo tenendo inizio e fine: `/home/…/foto/a.jpg`. */
export function accorciaPath(path: string, massimo = 64): string {
  if (path.length <= massimo) return path;
  const testa = Math.ceil(massimo / 2) - 1;
  const coda = Math.floor(massimo / 2) - 2;
  return `${path.slice(0, testa)}…${path.slice(-coda)}`;
}
