<script module lang="ts">
  import type { NomeIcona } from "./Icona.svelte";

  /** Descrizione di una colonna. `T` è il tipo della riga. */
  export interface Colonna<T = Record<string, unknown>> {
    /** Identificativo della colonna; se non passi `valore` è anche la
     *  proprietà da leggere sulla riga. */
    chiave: string;
    intestazione: string;
    /** Larghezza CSS (`120px`, `18%`, `minmax(…)` no: è una `<col>`). */
    larghezza?: string;
    allinea?: "sinistra" | "centro" | "destra";
    /** Numeri incolonnati e con cifre tabulari. */
    numerica?: boolean;
    /** Testo della cella. Senza questa si legge `riga[chiave]`. */
    valore?: (riga: T) => string | number;
    /** Non manda a capo e tronca con i puntini. */
    troncata?: boolean;
  }

  export type IconaVuoto = NomeIcona;
</script>

<script lang="ts" generics="T">
  import type { Snippet } from "svelte";
  import Vuoto from "./Vuoto.svelte";

  interface Props {
    /** Le righe da mostrare. */
    righe: T[];
    /** Le colonne, nell'ordine di visualizzazione. */
    colonne: Colonna<T>[];
    /** Identificativo stabile della riga: serve per la selezione e le chiavi. */
    chiave: (riga: T) => string | number;
    /** Riga selezionata, associabile con `bind:selezionata`. */
    selezionata?: string | number | null;
    /** Click su una riga (la selezione avviene comunque). */
    onseleziona?: (riga: T) => void;
    /** Doppio click o Invio su una riga: «apri». */
    onattiva?: (riga: T) => void;
    /** Rendering personalizzato di una cella; riceve riga e colonna. */
    cella?: Snippet<[T, Colonna<T>]>;
    /** Righe più basse: per elenchi lunghi di file. */
    densa?: boolean;
    /** Mostra le righe fantasma al posto dei dati. */
    caricamento?: boolean;
    /** Altezza massima dell'area scorrevole (l'intestazione resta fissa). */
    altezzaMax?: string;
    /** Stato vuoto personalizzato: sostituisce titolo/messaggio/icona. */
    vuoto?: Snippet;
    titoloVuoto?: string;
    messaggioVuoto?: string;
    iconaVuoto?: IconaVuoto;
  }

  let {
    righe,
    colonne,
    chiave,
    selezionata = $bindable(null),
    onseleziona,
    onattiva,
    cella,
    densa = false,
    caricamento = false,
    altezzaMax,
    vuoto,
    titoloVuoto = "Nessun file",
    messaggioVuoto,
    iconaVuoto = "documento",
  }: Props = $props();

  const selezionabile = $derived(!!onseleziona || selezionata !== undefined);

  function testo(riga: T, col: Colonna<T>): string {
    if (col.valore) return String(col.valore(riga));
    const v = (riga as Record<string, unknown>)[col.chiave];
    return v === null || v === undefined || v === "" ? "—" : String(v);
  }

  function seleziona(riga: T) {
    selezionata = chiave(riga);
    onseleziona?.(riga);
  }

  function suTastoRiga(e: KeyboardEvent, riga: T) {
    if (e.key === "Enter") {
      e.preventDefault();
      seleziona(riga);
      onattiva?.(riga);
    } else if (e.key === " ") {
      e.preventDefault();
      seleziona(riga);
    }
  }

  /** Frecce su/giù sull'area scorrevole: sposta la selezione. */
  function suTastoTabella(e: KeyboardEvent) {
    if (!selezionabile || righe.length === 0) return;
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const corrente = righe.findIndex((r) => chiave(r) === selezionata);
    const passo = e.key === "ArrowDown" ? 1 : -1;
    const prossimo = Math.min(
      Math.max(corrente === -1 ? 0 : corrente + passo, 0),
      righe.length - 1,
    );
    seleziona(righe[prossimo]);
  }
</script>

<!-- Il contenitore è messo a fuoco per navigare le righe con le frecce: il
     ruolo `group` gli dà una semantica, l'etichetta dice cosa contiene. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="contenitore"
  role="group"
  aria-label="Elenco file"
  style={altezzaMax ? `max-height: ${altezzaMax}` : undefined}
  tabindex={selezionabile ? 0 : undefined}
  onkeydown={suTastoTabella}
>
  <table class="tabella" class:densa>
    <colgroup>
      {#each colonne as col (col.chiave)}
        <col style={col.larghezza ? `width: ${col.larghezza}` : undefined} />
      {/each}
    </colgroup>

    <thead>
      <tr>
        {#each colonne as col (col.chiave)}
          <th class="all-{col.allinea ?? (col.numerica ? 'destra' : 'sinistra')}">
            {col.intestazione}
          </th>
        {/each}
      </tr>
    </thead>

    <tbody>
      {#if caricamento}
        {#each Array.from({ length: 6 }) as _, i (i)}
          <tr class="fantasma">
            {#each colonne as col (col.chiave)}
              <td><span class="scheletro"></span></td>
            {/each}
          </tr>
        {/each}
      {:else}
        {#each righe as riga (chiave(riga))}
          <tr
            class="riga"
            class:selezionata={selezionata === chiave(riga)}
            class:cliccabile={selezionabile}
            aria-selected={selezionabile ? selezionata === chiave(riga) : undefined}
            onclick={() => seleziona(riga)}
            ondblclick={() => onattiva?.(riga)}
            onkeydown={(e) => suTastoRiga(e, riga)}
          >
            {#each colonne as col (col.chiave)}
              <td
                class="all-{col.allinea ?? (col.numerica ? 'destra' : 'sinistra')}"
                class:numerica={col.numerica}
                class:troncata={col.troncata}
              >
                {#if cella}
                  {@render cella(riga, col)}
                {:else}
                  {testo(riga, col)}
                {/if}
              </td>
            {/each}
          </tr>
        {/each}
      {/if}
    </tbody>
  </table>

  {#if !caricamento && righe.length === 0}
    {#if vuoto}
      {@render vuoto()}
    {:else}
      <Vuoto icona={iconaVuoto} titolo={titoloVuoto} messaggio={messaggioVuoto} />
    {/if}
  {/if}
</div>

<style>
  .contenitore {
    position: relative;
    overflow: auto;
    border-radius: inherit;
    min-width: 0;
  }

  .contenitore:focus-visible {
    outline-offset: -2px;
  }

  .tabella {
    width: 100%;
    font-size: var(--piccolo);
    table-layout: fixed;
  }

  thead th {
    position: sticky;
    top: 0;
    z-index: 1;
    padding: var(--sp-3) var(--sp-4);
    background: var(--superficie);
    color: var(--testo-3);
    font-size: var(--micro);
    font-weight: var(--peso-forte);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    white-space: nowrap;
    border-bottom: 1px solid var(--bordo);
  }

  td {
    padding: var(--sp-3) var(--sp-4);
    color: var(--testo);
    border-bottom: 1px solid var(--bordo);
    vertical-align: middle;
  }

  .densa td,
  .densa thead th {
    padding: var(--sp-2) var(--sp-3);
  }

  tbody tr:last-child td {
    border-bottom: none;
  }

  .all-sinistra {
    text-align: left;
  }

  .all-centro {
    text-align: center;
  }

  .all-destra {
    text-align: right;
  }

  td.numerica {
    font-variant-numeric: tabular-nums;
    color: var(--testo);
    font-weight: var(--peso-medio);
  }

  td.troncata {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .cliccabile {
    cursor: pointer;
  }

  .riga {
    transition: background var(--transizione);
  }

  .cliccabile:hover td {
    background: var(--superficie-2);
  }

  .riga.selezionata td {
    background: var(--accento-tenue);
  }

  .riga.selezionata td:first-child {
    box-shadow: inset 3px 0 0 var(--accento);
  }

  /* Caricamento ------------------------------------------------------ */
  .scheletro {
    display: block;
    height: 10px;
    border-radius: var(--raggio-pillola);
    background: var(--superficie-3);
    animation: pulsa 1.2s ease-in-out infinite;
  }

  .fantasma:nth-child(even) .scheletro {
    opacity: 0.7;
  }

  @keyframes pulsa {
    0%,
    100% {
      opacity: 0.5;
    }
    50% {
      opacity: 1;
    }
  }
</style>
