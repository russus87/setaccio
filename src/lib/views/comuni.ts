/**
 * Piccoli aiuti condivisi fra le viste.
 *
 * Qui non c'è nessuna chiamata al backend: quella sta tutta in `lib/api.ts`.
 * Questo file tiene solo le conversioni che più viste rifarebbero uguali —
 * l'evidenziazione degli snippet, l'icona di un tipo, il raggruppamento per
 * giorno — così restano scritte una volta sola.
 */

import type { Tipo } from "../api";

/* ==========================================================================
   Evidenziazione degli snippet
   ========================================================================== */

/** Un pezzo di snippet: `forte` significa «era fra [[ e ]]». */
export interface Pezzo {
  testo: string;
  forte: boolean;
}

/**
 * Spezza lo snippet del backend nei suoi pezzi evidenziati e non.
 *
 * Il backend marca i termini trovati fra `[[` e `]]`. Restituendo dei pezzi
 * invece di una stringa di HTML, la vista può rendere `<mark>` con un
 * `{#each}` normale: niente `@html` su testo che viene dal disco dell'utente,
 * quindi niente iniezione possibile da un file con dentro dei tag.
 */
export function spezzaEvidenziato(snippet: string): Pezzo[] {
  const pezzi: Pezzo[] = [];
  let resto = snippet;

  while (resto.length > 0) {
    const apre = resto.indexOf("[[");
    if (apre === -1) break;
    const chiude = resto.indexOf("]]", apre + 2);
    if (chiude === -1) break;

    if (apre > 0) pezzi.push({ testo: resto.slice(0, apre), forte: false });
    const dentro = resto.slice(apre + 2, chiude);
    if (dentro) pezzi.push({ testo: dentro, forte: true });
    resto = resto.slice(chiude + 2);
  }

  if (resto.length > 0) pezzi.push({ testo: resto, forte: false });
  return pezzi;
}

/* ==========================================================================
   Tipi di file
   ========================================================================== */

/** Icona del tile a sinistra nelle liste, per ogni tipo di file. */
export function iconaTipo(tipo: Tipo) {
  switch (tipo) {
    case "documento":
      return "documento" as const;
    case "tracciato":
      return "tracciati" as const;
    case "archivio":
      return "archivio" as const;
    case "media":
      return "media" as const;
    case "installer":
      return "installer" as const;
    case "artefatto":
      return "artefatto" as const;
    default:
      return "altro" as const;
  }
}

/** Etichetta al plurale di un tipo, per titoli e legende. */
export function tipoAlPlurale(tipo: string): string {
  switch (tipo) {
    case "documento":
      return "documenti";
    case "tracciato":
      return "tracciati";
    case "archivio":
      return "archivi";
    case "media":
      return "media";
    case "installer":
      return "installer";
    case "artefatto":
      return "artefatti";
    default:
      return "altri";
  }
}

/* ==========================================================================
   Date
   ========================================================================== */

/** Epoch secondi → `2026-07-25`, la chiave con cui si raggruppa per giorno. */
export function chiaveGiorno(epochSecondi: number): string {
  const d = new Date(epochSecondi * 1000);
  const mese = String(d.getMonth() + 1).padStart(2, "0");
  const giorno = String(d.getDate()).padStart(2, "0");
  return `${d.getFullYear()}-${mese}-${giorno}`;
}

/** `oggi`, `ieri`, oppure la data per esteso. */
export function etichettaGiorno(epochSecondi: number): string {
  const oggi = chiaveGiorno(Date.now() / 1000);
  const ieri = chiaveGiorno(Date.now() / 1000 - 86400);
  const chiave = chiaveGiorno(epochSecondi);
  if (chiave === oggi) return "Oggi";
  if (chiave === ieri) return "Ieri";
  return new Intl.DateTimeFormat("it-IT", {
    weekday: "long",
    day: "numeric",
    month: "long",
  }).format(new Date(epochSecondi * 1000));
}

/** `2026-07-25` → epoch secondi di mezzanotte locale; stringa vuota → null. */
export function dataInEpoch(iso: string, fineGiornata = false): number | null {
  if (!iso) return null;
  const t = Date.parse(fineGiornata ? `${iso}T23:59:59` : `${iso}T00:00:00`);
  return Number.isNaN(t) ? null : Math.floor(t / 1000);
}

/** Epoch secondi → `2026-07-25`, per riempire un `<input type="date">`. */
export function epochInData(epoch: number | null): string {
  return epoch == null ? "" : chiaveGiorno(epoch);
}

/* ==========================================================================
   Varie
   ========================================================================== */

/** Cartella che contiene il file, senza il nome. */
export function cartellaDi(path: string): string {
  const taglio = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return taglio <= 0 ? path : path.slice(0, taglio);
}

/** Messaggio leggibile da un errore qualunque. */
export function messaggioErrore(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}
