<script module lang="ts">
  import type { NomeIcona } from "./Icona.svelte";

  /** Le sezioni dell'applicazione. Sono anche la chiave del routing. */
  export type Sezione =
    | "dashboard"
    | "ricerca"
    | "novita"
    | "duplicati"
    | "lotti"
    | "revisione"
    | "impostazioni";

  export interface VoceNav {
    id: Sezione;
    etichetta: string;
    icona: NomeIcona;
    /** Riga di aiuto, usata come `title` e come sottotitolo della pagina. */
    descrizione: string;
    /** `principale` sta in alto, `secondario` sotto la linea sottile. */
    gruppo: "principale" | "secondario";
  }

  /** L'ordine qui è l'ordine in cui le voci appaiono nella sidebar. */
  export const SEZIONI: readonly VoceNav[] = [
    {
      id: "dashboard",
      etichetta: "Dashboard",
      icona: "dashboard",
      descrizione: "Lo stato dell'indice a colpo d'occhio",
      gruppo: "principale",
    },
    {
      id: "ricerca",
      etichetta: "Ricerca",
      icona: "ricerca",
      descrizione: "Cerca nel testo e nei nomi dei file indicizzati",
      gruppo: "principale",
    },
    {
      id: "novita",
      etichetta: "Novità",
      icona: "novita",
      descrizione: "Cos'è cambiato sul disco negli ultimi giorni",
      gruppo: "principale",
    },
    {
      id: "duplicati",
      etichetta: "Duplicati",
      icona: "duplicati",
      descrizione: "Gruppi di file identici e spazio recuperabile",
      gruppo: "principale",
    },
    {
      id: "lotti",
      etichetta: "Lotti",
      icona: "lotti",
      descrizione: "Tracciati, layout e composizioni documentali",
      gruppo: "principale",
    },
    {
      id: "revisione",
      etichetta: "Revisione",
      icona: "revisione",
      descrizione: "I file che le regole non hanno saputo collocare",
      gruppo: "secondario",
    },
    {
      id: "impostazioni",
      etichetta: "Impostazioni",
      icona: "impostazioni",
      descrizione: "Sorgenti, regole e comportamento della scansione",
      gruppo: "secondario",
    },
  ];

  /** Comodo per titoli e sottotitoli di pagina. */
  export function voceDi(id: Sezione): VoceNav {
    return SEZIONI.find((v) => v.id === id) ?? SEZIONI[0];
  }
</script>

<script lang="ts">
  import { PREFERENZE, tema } from "../tema.svelte";
  import Icona from "./Icona.svelte";

  interface Props {
    /** Sezione attualmente visibile. */
    sezione: Sezione;
    /** Richiesta di cambio sezione. */
    onnaviga: (s: Sezione) => void;
    /** Numeri da mostrare accanto alle voci (duplicati, revisione…). */
    conteggi?: Partial<Record<Sezione, number>>;
  }

  let { sezione, onnaviga, conteggi = {} }: Props = $props();

  const principali = SEZIONI.filter((v) => v.gruppo === "principale");
  const secondarie = SEZIONI.filter((v) => v.gruppo === "secondario");
</script>

<aside class="sidebar">
  <!-- Logo ------------------------------------------------------------ -->
  <div class="marchio">
    <svg viewBox="0 0 32 32" width="26" height="26" aria-hidden="true">
      <defs>
        <linearGradient id="marchio-lime" x1="0" y1="0" x2="0.85" y2="1">
          <stop offset="0%" style="stop-color: var(--accento-chiaro)" />
          <stop offset="55%" style="stop-color: var(--accento)" />
          <stop offset="100%" style="stop-color: var(--accento-scuro)" />
        </linearGradient>
        <clipPath id="marchio-rete">
          <circle cx="16" cy="13" r="8.6" />
        </clipPath>
      </defs>
      <g clip-path="url(#marchio-rete)" stroke="url(#marchio-lime)" stroke-width="1.1" opacity="0.9">
        <path d="M6 8.4h20M6 13h20M6 17.6h20" />
        <path d="M11.4 3v20M16 3v20M20.6 3v20" />
      </g>
      <circle cx="16" cy="13" r="8.6" fill="none" stroke="url(#marchio-lime)" stroke-width="2.2" />
      <circle cx="12.4" cy="27.4" r="1.5" fill="url(#marchio-lime)" />
      <circle cx="17.4" cy="29.2" r="2.1" fill="url(#marchio-lime)" />
      <circle cx="22.6" cy="27.2" r="1.2" fill="url(#marchio-lime)" />
    </svg>
    <span class="nome">setaccio</span>
  </div>

  <!-- Navigazione ----------------------------------------------------- -->
  <nav class="nav" aria-label="Sezioni">
    <ul>
      {#each principali as v (v.id)}
        <li>
          <button
            class="voce"
            class:attiva={sezione === v.id}
            aria-current={sezione === v.id ? "page" : undefined}
            title={v.descrizione}
            onclick={() => onnaviga(v.id)}
          >
            <Icona nome={v.icona} dimensione={18} />
            <span class="testo">{v.etichetta}</span>
            {#if conteggi[v.id]}
              <span class="conteggio cifre">{conteggi[v.id]}</span>
            {/if}
          </button>
        </li>
      {/each}
    </ul>

    <hr class="linea" />

    <ul>
      {#each secondarie as v (v.id)}
        <li>
          <button
            class="voce"
            class:attiva={sezione === v.id}
            aria-current={sezione === v.id ? "page" : undefined}
            title={v.descrizione}
            onclick={() => onnaviga(v.id)}
          >
            <Icona nome={v.icona} dimensione={18} />
            <span class="testo">{v.etichetta}</span>
            {#if conteggi[v.id]}
              <span class="conteggio cifre">{conteggi[v.id]}</span>
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  </nav>

  <!-- Tema ------------------------------------------------------------ -->
  <div class="piede">
    <hr class="linea" />
    <div class="tema" role="group" aria-label="Tema dell'interfaccia">
      {#each PREFERENZE as p (p.id)}
        <button
          class="scelta"
          class:attiva={tema.preferenza === p.id}
          title="Tema {p.etichetta.toLowerCase()}"
          aria-label="Tema {p.etichetta.toLowerCase()}"
          aria-pressed={tema.preferenza === p.id}
          onclick={() => tema.imposta(p.id)}
        >
          <Icona nome={p.icona} dimensione={16} />
        </button>
      {/each}
    </div>
  </div>
</aside>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    width: var(--larghezza-sidebar);
    height: 100%;
    padding: var(--sp-6) var(--sp-4) var(--sp-4);
    background: var(--sfondo);
    border-right: 1px solid var(--bordo);
    flex: 0 0 auto;
  }

  /* Marchio ---------------------------------------------------------- */
  .marchio {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: 0 var(--sp-3) var(--sp-7);
  }

  .nome {
    font-size: var(--medio);
    font-weight: var(--peso-grasso);
    letter-spacing: -0.02em;
  }

  /* Navigazione ------------------------------------------------------ */
  .nav {
    flex: 1 1 auto;
    overflow-y: auto;
  }

  .nav ul {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .voce {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    width: 100%;
    padding: 0 var(--sp-3);
    height: 38px;
    border-radius: var(--raggio);
    color: var(--testo-2);
    font-size: var(--corpo);
    font-weight: var(--peso-medio);
    text-align: left;
    transition:
      background var(--transizione),
      color var(--transizione);
  }

  .voce:hover {
    background: var(--superficie);
    color: var(--testo);
  }

  .voce.attiva {
    background: var(--superficie-2);
    color: var(--testo);
    font-weight: var(--peso-forte);
  }

  /* Il filo lime a sinistra della voce attiva: è il segnale che nel
     mockup distingue la pagina corrente. */
  .voce.attiva::before {
    content: "";
    position: absolute;
    left: -10px;
    top: 50%;
    margin-top: -9px;
    width: 3px;
    height: 18px;
    border-radius: var(--raggio-pillola);
    background: var(--gradiente-accento);
  }

  .nav li {
    position: relative;
  }

  .testo {
    flex: 1 1 auto;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .conteggio {
    flex: 0 0 auto;
    min-width: 20px;
    padding: 0 6px;
    height: 18px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--raggio-pillola);
    background: var(--accento-tenue);
    color: var(--accento-testo);
    font-size: var(--micro);
    font-weight: var(--peso-grasso);
  }

  .linea {
    height: 1px;
    border: 0;
    background: var(--bordo);
    margin: var(--sp-4) var(--sp-3);
  }

  /* Piede ------------------------------------------------------------ */
  .piede {
    flex: 0 0 auto;
  }

  .tema {
    display: flex;
    gap: 2px;
    padding: 3px;
    background: var(--superficie);
    border: 1px solid var(--bordo);
    border-radius: var(--raggio-pillola);
  }

  .scelta {
    flex: 1 1 0;
    display: flex;
    align-items: center;
    justify-content: center;
    height: 30px;
    border-radius: var(--raggio-pillola);
    color: var(--testo-3);
    transition:
      background var(--transizione),
      color var(--transizione);
  }

  .scelta:hover {
    color: var(--testo);
  }

  .scelta.attiva {
    background: var(--superficie-3);
    color: var(--testo);
  }
</style>
