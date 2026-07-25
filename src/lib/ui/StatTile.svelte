<script lang="ts">
  import type { Snippet } from "svelte";
  import Icona, { type NomeIcona } from "./Icona.svelte";

  interface Props {
    /** Il numero, già formattato: la tile non decide come si scrivono i byte. */
    valore: string | number;
    /** Unità piccola accanto al numero: `Gb`, `file`, `%`… */
    unita?: string;
    /** Etichetta sotto il numero. */
    etichetta: string;
    /** Variazione percentuale: positiva verde, negativa rossa. */
    variazione?: number;
    /** Testo accanto alla variazione, per dire rispetto a cosa. */
    periodo?: string;
    /** Icona nella tile arrotondata in alto a destra. */
    icona?: NomeIcona;
    /** Colore del tile dell'icona, con i token di tipo/stato (`documento`…). */
    tono?: string;
    /** Rende la tile cliccabile: aggiunge cursore e reazione al passaggio. */
    onclick?: () => void;
    /** Contenuto extra sotto l'etichetta (una sparkline, una barra…). */
    children?: Snippet;
  }

  let {
    valore,
    unita,
    etichetta,
    variazione,
    periodo,
    icona,
    tono = "neutro",
    onclick,
    children,
  }: Props = $props();

  const segno = $derived(
    variazione === undefined || variazione === 0
      ? "fermo"
      : variazione > 0
        ? "su"
        : "giu",
  );

  const variazioneTesto = $derived(
    variazione === undefined
      ? ""
      : `${variazione > 0 ? "+" : ""}${variazione.toFixed(1).replace(".", ",")}%`,
  );
</script>

{#snippet contenuto()}
  <div class="testa">
    <p class="etichetta">{etichetta}</p>
    {#if icona}
      <span class="riquadro {tono}"><Icona nome={icona} dimensione={18} /></span>
    {/if}
  </div>

  <p class="valore cifre">
    {valore}{#if unita}<span class="unita">{unita}</span>{/if}
  </p>

  {#if variazione !== undefined || periodo}
    <p class="piede">
      {#if variazione !== undefined}
        <span class="variazione {segno}">
          {#if segno !== "fermo"}
            <Icona nome="freccia" dimensione={13} ruota={segno === "su" ? -90 : 90} />
          {/if}
          {variazioneTesto}
        </span>
      {/if}
      {#if periodo}<span class="periodo">{periodo}</span>{/if}
    </p>
  {/if}

  {#if children}
    <div class="extra">{@render children()}</div>
  {/if}
{/snippet}

{#if onclick}
  <button type="button" class="tile cliccabile" {onclick}>{@render contenuto()}</button>
{:else}
  <div class="tile">{@render contenuto()}</div>
{/if}

<style>
  .tile {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    width: 100%;
    text-align: left;
    padding: var(--sp-5);
    background: var(--superficie);
    border: 1px solid var(--bordo);
    border-radius: var(--raggio-xl);
    box-shadow: var(--ombra-1);
    min-width: 0;
  }

  .cliccabile {
    cursor: pointer;
    transition:
      border-color var(--transizione),
      transform var(--transizione),
      box-shadow var(--transizione);
  }

  .cliccabile:hover {
    border-color: var(--bordo-forte);
    box-shadow: var(--ombra-2);
    transform: translateY(-1px);
  }

  .testa {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--sp-3);
  }

  .etichetta {
    font-size: var(--minuto);
    font-weight: var(--peso-medio);
    color: var(--testo-2);
    letter-spacing: 0.01em;
  }

  .riquadro {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    border-radius: var(--raggio);
    flex: 0 0 auto;
    color: var(--testo-2);
    background: var(--superficie-2);
  }

  .valore {
    font-size: var(--cifra);
    font-weight: var(--peso-grasso);
    line-height: 1.05;
    letter-spacing: -0.02em;
    color: var(--testo);
  }

  .unita {
    margin-left: var(--sp-2);
    font-size: var(--corpo);
    font-weight: var(--peso-medio);
    color: var(--testo-2);
    letter-spacing: 0;
  }

  .piede {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    font-size: var(--minuto);
  }

  .variazione {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    font-weight: var(--peso-forte);
  }

  .variazione.su {
    color: var(--successo);
  }

  .variazione.giu {
    color: var(--pericolo);
  }

  .variazione.fermo {
    color: var(--testo-3);
  }

  .periodo {
    color: var(--testo-3);
  }

  .extra {
    margin-top: var(--sp-1);
  }

  /* Toni del riquadro icona, sui token semantici dell'app. */
  .riquadro.accento {
    color: var(--su-accento);
    background: var(--gradiente-accento);
  }
  .riquadro.documento {
    color: var(--documento);
    background: var(--documento-bg);
  }
  .riquadro.tracciato {
    color: var(--tracciato);
    background: var(--tracciato-bg);
  }
  .riquadro.archivio {
    color: var(--archivio);
    background: var(--archivio-bg);
  }
  .riquadro.media {
    color: var(--media);
    background: var(--media-bg);
  }
  .riquadro.installer {
    color: var(--installer);
    background: var(--installer-bg);
  }
  .riquadro.artefatto {
    color: var(--artefatto);
    background: var(--artefatto-bg);
  }
  .riquadro.canonico {
    color: var(--canonico);
    background: var(--canonico-bg);
  }
  .riquadro.duplicato {
    color: var(--duplicato);
    background: var(--duplicato-bg);
  }
  .riquadro.orfano {
    color: var(--orfano);
    background: var(--orfano-bg);
  }
  .riquadro.avviso {
    color: var(--avviso);
    background: var(--avviso-bg);
  }
  .riquadro.pericolo {
    color: var(--pericolo);
    background: var(--pericolo-bg);
  }
</style>
