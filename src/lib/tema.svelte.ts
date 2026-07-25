/**
 * Stato del tema dell'applicazione.
 *
 * Tre preferenze possibili — `chiaro`, `scuro`, `sistema` — persistite in
 * `localStorage`. Ciò che finisce sul DOM è invece sempre un tema *risolto*
 * (`chiaro` o `scuro`), scritto come attributo `data-tema` su `<html>`: i
 * token di `app.css` leggono quello, e la scelta manuale vince sempre sulla
 * preferenza di sistema perché l'attributo è più specifico della media query.
 */

export type PreferenzaTema = "chiaro" | "scuro" | "sistema";
export type TemaRisolto = "chiaro" | "scuro";

const CHIAVE = "setaccio:tema";
const QUERY_SCURO = "(prefers-color-scheme: dark)";

/** Le tre preferenze, nell'ordine in cui vanno mostrate in un selettore. */
export const PREFERENZE: readonly {
  id: PreferenzaTema;
  etichetta: string;
  icona: "sole" | "luna" | "monitor";
}[] = [
  { id: "chiaro", etichetta: "Chiaro", icona: "sole" },
  { id: "scuro", etichetta: "Scuro", icona: "luna" },
  { id: "sistema", etichetta: "Sistema", icona: "monitor" },
];

function nelBrowser(): boolean {
  return typeof window !== "undefined" && typeof document !== "undefined";
}

function leggiPreferenza(): PreferenzaTema {
  if (!nelBrowser()) return "sistema";
  try {
    const v = window.localStorage.getItem(CHIAVE);
    if (v === "chiaro" || v === "scuro" || v === "sistema") return v;
  } catch {
    /* localStorage può essere negato: si ricade sul default. */
  }
  return "sistema";
}

function temaDiSistema(): TemaRisolto {
  if (!nelBrowser() || typeof window.matchMedia !== "function") return "chiaro";
  return window.matchMedia(QUERY_SCURO).matches ? "scuro" : "chiaro";
}

class StatoTema {
  /** Ciò che l'utente ha scelto. */
  preferenza = $state<PreferenzaTema>(leggiPreferenza());
  /** Ciò che il sistema operativo dice in questo momento. */
  #sistema = $state<TemaRisolto>(temaDiSistema());

  /** Il tema effettivamente applicato al documento. */
  risolto = $derived<TemaRisolto>(
    this.preferenza === "sistema" ? this.#sistema : this.preferenza,
  );

  /** Vero quando il tema applicato è quello scuro. Comodo nei template. */
  scuro = $derived(this.risolto === "scuro");

  /** Imposta la preferenza e la persiste. */
  imposta(p: PreferenzaTema): void {
    this.preferenza = p;
    if (!nelBrowser()) return;
    try {
      window.localStorage.setItem(CHIAVE, p);
    } catch {
      /* Se non si può scrivere, il tema resta valido per questa sessione. */
    }
  }

  /**
   * Commuta fra chiaro e scuro partendo dal tema *risolto*: premendo
   * l'interruttore mentre si è su "sistema" si passa all'opposto di ciò che
   * si sta vedendo, che è l'unica cosa che l'utente si aspetta.
   */
  commuta(): void {
    this.imposta(this.risolto === "scuro" ? "chiaro" : "scuro");
  }

  /** Fa ciclare le tre preferenze: chiaro → scuro → sistema → chiaro. */
  cicla(): void {
    const i = PREFERENZE.findIndex((p) => p.id === this.preferenza);
    this.imposta(PREFERENZE[(i + 1) % PREFERENZE.length].id);
  }

  /** @internal — usato solo dall'ascoltatore della media query. */
  aggiornaSistema(t: TemaRisolto): void {
    this.#sistema = t;
  }
}

export const tema = new StatoTema();

/**
 * Aggancia il tema al documento: scrive `data-tema` su `<html>` e resta in
 * ascolto dei cambi di preferenza del sistema. Va chiamata una volta sola,
 * dal bootstrap in `main.ts`. Restituisce la funzione di sgancio.
 */
export function collegaTema(): () => void {
  if (!nelBrowser()) return () => {};

  const radice = document.documentElement;

  const mq =
    typeof window.matchMedia === "function"
      ? window.matchMedia(QUERY_SCURO)
      : null;
  const suCambio = (e: MediaQueryListEvent) =>
    tema.aggiornaSistema(e.matches ? "scuro" : "chiaro");
  mq?.addEventListener("change", suCambio);

  const ferma = $effect.root(() => {
    $effect(() => {
      radice.dataset.tema = tema.risolto;
      radice.style.colorScheme = tema.risolto === "scuro" ? "dark" : "light";
    });
  });

  return () => {
    mq?.removeEventListener("change", suCambio);
    ferma();
  };
}
