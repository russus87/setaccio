<script lang="ts">
  import type { Snippet } from "svelte";
  import Campo from "./Campo.svelte";

  interface Props {
    /** Titolo della pagina: grande e in grassetto, come nel mockup. */
    titolo: string;
    /** Riga grigia sotto il titolo. */
    sottotitolo?: string;
    /** Testo della ricerca, associabile con `bind:ricerca`. */
    ricerca?: string;
    segnapostoRicerca?: string;
    /** Nasconde del tutto la barra di ricerca (viste che non cercano nulla). */
    mostraRicerca?: boolean;
    /** Invio nella barra di ricerca. */
    onricerca?: (query: string) => void;
    /** Ogni battuta nella barra di ricerca (per la ricerca incrementale). */
    ondigita?: (query: string) => void;
    /** Bottoni e controlli a destra del titolo. */
    azioni?: Snippet;
    /** Riga aggiuntiva sotto la ricerca: filtri, faccette, chip… */
    filtri?: Snippet;
  }

  let {
    titolo,
    sottotitolo,
    ricerca = $bindable(""),
    segnapostoRicerca = "Cerca fra i file indicizzati…",
    mostraRicerca = true,
    onricerca,
    ondigita,
    azioni,
    filtri,
  }: Props = $props();
</script>

<header class="topbar">
  <div class="riga-titolo">
    <div class="titoli">
      <h1 class="titolo">{titolo}</h1>
      {#if sottotitolo}<p class="sottotitolo">{sottotitolo}</p>{/if}
    </div>

    {#if azioni}
      <div class="azioni">{@render azioni()}</div>
    {/if}
  </div>

  {#if mostraRicerca}
    <div class="ricerca">
      <Campo
        bind:valore={ricerca}
        icona="ricerca"
        dimensione="lg"
        segnaposto={segnapostoRicerca}
        azzerabile
        autocomplete="off"
        spellcheck={false}
        oninvio={(v) => onricerca?.(v)}
        oninput={(e) => ondigita?.(e.currentTarget.value)}
        onazzera={() => {
          ondigita?.("");
          onricerca?.("");
        }}
      />
    </div>
  {/if}

  {#if filtri}
    <div class="filtri">{@render filtri()}</div>
  {/if}
</header>

<style>
  .topbar {
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
    padding: var(--sp-6) var(--sp-7) var(--sp-5);
    background: var(--sfondo);
    border-bottom: 1px solid var(--bordo);
  }

  .riga-titolo {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-5);
    min-height: 40px;
  }

  .titoli {
    min-width: 0;
  }

  .titolo {
    font-size: var(--titolo);
    font-weight: var(--peso-grasso);
    letter-spacing: -0.03em;
    color: var(--testo);
  }

  .sottotitolo {
    margin-top: 2px;
    font-size: var(--piccolo);
    color: var(--testo-2);
  }

  .azioni {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    flex: 0 0 auto;
  }

  /* La ricerca è l'elemento dominante: tutta la larghezza, alta 46px. */
  .ricerca {
    width: 100%;
  }

  .filtri {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--sp-2);
  }
</style>
