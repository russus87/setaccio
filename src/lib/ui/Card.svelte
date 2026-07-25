<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    /** Titolo mostrato in testa alla card. */
    titolo?: string;
    /** Riga piccola e grigia sotto il titolo. */
    sottotitolo?: string;
    /** Contenuto allineato a destra nell'intestazione (bottoni, filtri…). */
    azioni?: Snippet;
    /** Sostituisce del tutto l'intestazione predefinita. */
    intestazione?: Snippet;
    /** `nessuna` per contenuti che devono toccare i bordi (tabelle, grafici). */
    padding?: "nessuna" | "stretta" | "normale" | "larga";
    /** Card di spicco: fondo a gradiente lime e testo scuro. */
    accento?: boolean;
    /** Toglie il bordo e appiattisce il fondo: per card annidate. */
    piatta?: boolean;
    /** Alza la card al passaggio del mouse: usalo solo se è cliccabile. */
    interattiva?: boolean;
    /** Fa sì che il contenuto occupi tutta l'altezza disponibile. */
    piena?: boolean;
    classe?: string;
    children?: Snippet;
  }

  let {
    titolo,
    sottotitolo,
    azioni,
    intestazione,
    padding = "normale",
    accento = false,
    piatta = false,
    interattiva = false,
    piena = false,
    classe = "",
    children,
  }: Props = $props();

  const conIntestazione = $derived(!!(titolo || sottotitolo || azioni || intestazione));
</script>

<section
  class="card pad-{padding} {classe}"
  class:accento
  class:piatta
  class:interattiva
  class:piena
>
  {#if conIntestazione}
    <header class="intestazione">
      {#if intestazione}
        {@render intestazione()}
      {:else}
        <div class="titoli">
          {#if titolo}<h2 class="titolo">{titolo}</h2>{/if}
          {#if sottotitolo}<p class="sottotitolo">{sottotitolo}</p>{/if}
        </div>
        {#if azioni}
          <div class="azioni">{@render azioni()}</div>
        {/if}
      {/if}
    </header>
  {/if}

  {#if children}
    <div class="corpo">{@render children()}</div>
  {/if}
</section>

<style>
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    background: var(--superficie);
    border: 1px solid var(--bordo);
    border-radius: var(--raggio-xl);
    box-shadow: var(--ombra-1);
    min-width: 0;
  }

  .piena {
    height: 100%;
  }

  .piatta {
    border-color: transparent;
    box-shadow: none;
    background: var(--superficie-2);
  }

  .interattiva {
    cursor: pointer;
    transition:
      border-color var(--transizione),
      box-shadow var(--transizione),
      transform var(--transizione);
  }

  .interattiva:hover {
    border-color: var(--bordo-forte);
    box-shadow: var(--ombra-2);
    transform: translateY(-1px);
  }

  .accento {
    background: var(--gradiente-accento);
    border-color: transparent;
    color: var(--su-accento);
    box-shadow: var(--ombra-accento);
  }

  /* Padding ---------------------------------------------------------- */
  .pad-nessuna {
    padding: 0;
    gap: 0;
  }

  .pad-stretta {
    padding: var(--sp-4);
    gap: var(--sp-3);
  }

  .pad-normale {
    padding: var(--sp-5);
  }

  .pad-larga {
    padding: var(--sp-6) var(--sp-6) var(--sp-7);
    gap: var(--sp-5);
  }

  /* Quando la card non ha padding, l'intestazione se lo mette da sé. */
  .pad-nessuna .intestazione {
    padding: var(--sp-5) var(--sp-5) 0;
  }

  .pad-nessuna .corpo {
    padding-top: var(--sp-4);
  }

  .intestazione {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--sp-4);
    min-height: 0;
  }

  .titoli {
    min-width: 0;
  }

  .titolo {
    font-size: var(--medio);
    font-weight: var(--peso-grasso);
    letter-spacing: -0.01em;
  }

  .sottotitolo {
    margin-top: 2px;
    font-size: var(--minuto);
    color: var(--testo-2);
  }

  .accento .sottotitolo {
    color: rgba(16, 27, 4, 0.7);
  }

  .azioni {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    flex: 0 0 auto;
  }

  .corpo {
    flex: 1 1 auto;
    min-width: 0;
    min-height: 0;
  }
</style>
