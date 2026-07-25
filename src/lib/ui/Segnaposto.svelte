<script lang="ts">
  import Badge from "./Badge.svelte";
  import Icona, { type NomeIcona } from "./Icona.svelte";

  /**
   * Riempitivo per le viste non ancora scritte. Chi costruisce le viste
   * sostituisce questo componente con il contenuto vero: non è pensato per
   * restare in produzione.
   */
  interface Props {
    /** Nome della vista che andrà qui. */
    titolo: string;
    /** Una o due righe su cosa mostrerà la vista. */
    descrizione?: string;
    icona?: NomeIcona;
    /** Elenco puntato di ciò che la vista dovrà contenere. */
    elementi?: string[];
    /** Comandi del backend su cui la vista si appoggerà. */
    comandi?: string[];
  }

  let { titolo, descrizione, icona = "dashboard", elementi = [], comandi = [] }: Props =
    $props();
</script>

<section class="segnaposto">
  <div class="riquadro">
    <header class="testa">
      <span class="tile"><Icona nome={icona} dimensione={24} spessore={1.4} /></span>
      <div class="titoli">
        <div class="riga-titolo">
          <h2>{titolo}</h2>
          <Badge variante="accento" testo="da costruire" />
        </div>
        {#if descrizione}<p class="descrizione">{descrizione}</p>{/if}
      </div>
    </header>

    {#if elementi.length > 0}
      <div class="blocco">
        <h3>Cosa andrà qui</h3>
        <ul class="elenco">
          {#each elementi as voce, i (i)}
            <li><Icona nome="check" dimensione={15} />{voce}</li>
          {/each}
        </ul>
      </div>
    {/if}

    {#if comandi.length > 0}
      <div class="blocco">
        <h3>Comandi del backend</h3>
        <ul class="comandi">
          {#each comandi as c, i (i)}
            <li class="mono">{c}</li>
          {/each}
        </ul>
      </div>
    {/if}
  </div>
</section>

<style>
  .segnaposto {
    display: flex;
    justify-content: center;
    padding: var(--sp-6) 0;
  }

  .riquadro {
    width: 100%;
    max-width: 620px;
    display: flex;
    flex-direction: column;
    gap: var(--sp-6);
    padding: var(--sp-7);
    background: var(--superficie);
    border: 1px dashed var(--bordo-forte);
    border-radius: var(--raggio-xxl);
  }

  .testa {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-4);
  }

  .tile {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 48px;
    height: 48px;
    flex: 0 0 auto;
    border-radius: var(--raggio-lg);
    background: var(--accento-tenue);
    color: var(--accento-testo);
  }

  .titoli {
    min-width: 0;
  }

  .riga-titolo {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex-wrap: wrap;
  }

  h2 {
    font-size: var(--grande);
    font-weight: var(--peso-grasso);
    letter-spacing: -0.02em;
  }

  .descrizione {
    margin-top: var(--sp-2);
    font-size: var(--corpo);
    color: var(--testo-2);
    line-height: var(--riga-larga);
  }

  .blocco {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }

  h3 {
    font-size: var(--micro);
    font-weight: var(--peso-forte);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--testo-3);
  }

  .elenco {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    font-size: var(--corpo);
    color: var(--testo-2);
  }

  .elenco li {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    color: var(--testo-2);
  }

  .comandi {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
  }

  .comandi li {
    padding: 3px var(--sp-2);
    border-radius: var(--raggio-sm);
    background: var(--superficie-2);
    border: 1px solid var(--bordo);
    color: var(--testo-2);
    font-size: var(--micro);
  }

  .mono {
    font-family: var(--famiglia-mono);
  }
</style>
