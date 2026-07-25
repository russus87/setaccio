<script lang="ts">
  /**
   * La riga di lista del mockup: tile arrotondata con l'icona del tipo a
   * sinistra, nome in grassetto, percorso piccolo sotto, valore a destra in
   * cifre tabulari. Tutte le viste che elencano file passano di qui, così il
   * ritmo verticale resta lo stesso ovunque.
   */
  import type { Snippet } from "svelte";
  import { accorciaPath, formattaByte, type FileRecord } from "../api";
  import Icona from "../ui/Icona.svelte";
  import { iconaTipo } from "./comuni";

  interface Props {
    file: FileRecord;
    /** Sostituisce il percorso come riga piccola sotto il nome. */
    sottotitolo?: string;
    /** Valore a destra. Se non arriva si mostra la dimensione del file. */
    valore?: string;
    /** Riga piccola sotto il valore. */
    dettaglio?: string;
    /** Rende la riga cliccabile e ne segnala la selezione. */
    onseleziona?: () => void;
    selezionata?: boolean;
    /** Doppio clic: le viste ci attaccano l'apertura col sistema. */
    onapri?: () => void;
    /** Contenuto extra sotto il nome (badge, snippet, pulsanti…). */
    children?: Snippet;
    /** Contenuto a destra, al posto del valore. */
    azioni?: Snippet;
    /** Contenuto a sinistra della tile: una casella di selezione, di solito. */
    prima?: Snippet;
    compatta?: boolean;
  }

  let {
    file,
    sottotitolo,
    valore,
    dettaglio,
    onseleziona,
    selezionata = false,
    onapri,
    children,
    azioni,
    prima,
    compatta = false,
  }: Props = $props();

  const cliccabile = $derived(!!onseleziona);
</script>

<div class="riga-file" class:selezionata class:compatta>
  {#if prima}
    <div class="prima">{@render prima()}</div>
  {/if}

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    class="corpo"
    class:cliccabile
    role={cliccabile ? "button" : undefined}
    tabindex={cliccabile ? 0 : undefined}
    onclick={() => onseleziona?.()}
    ondblclick={() => onapri?.()}
    onkeydown={(e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        onseleziona?.();
      }
    }}
  >
    <span class="tile {file.tipo}">
      <Icona nome={iconaTipo(file.tipo)} dimensione={compatta ? 16 : 18} />
    </span>

    <div class="testi">
      <p class="nome troncato" title={file.nome}>{file.nome}</p>
      <p class="sotto troncato" title={file.path}>
        {sottotitolo ?? accorciaPath(file.path, 72)}
      </p>
      {#if children}
        <div class="extra">{@render children()}</div>
      {/if}
    </div>

    <div class="destra">
      {#if azioni}
        {@render azioni()}
      {:else}
        <span class="valore cifre">{valore ?? formattaByte(file.size)}</span>
        {#if dettaglio}<span class="dettaglio">{dettaglio}</span>{/if}
      {/if}
    </div>
  </div>
</div>

<style>
  .riga-file {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    min-width: 0;
  }

  .prima {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    padding-left: var(--sp-1);
  }

  .corpo {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex: 1 1 auto;
    min-width: 0;
    padding: var(--sp-3);
    border-radius: var(--raggio);
    border: 1px solid transparent;
    text-align: left;
    transition:
      background var(--transizione),
      border-color var(--transizione);
  }

  .compatta .corpo {
    padding: var(--sp-2) var(--sp-3);
  }

  .cliccabile {
    cursor: pointer;
  }

  .cliccabile:hover {
    background: var(--superficie-2);
  }

  .selezionata .corpo {
    background: var(--accento-tenue);
    border-color: var(--accento-bordo);
  }

  .tile {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 38px;
    height: 38px;
    flex: 0 0 auto;
    border-radius: var(--raggio);
    color: var(--testo-2);
    background: var(--superficie-2);
  }

  .compatta .tile {
    width: 32px;
    height: 32px;
    border-radius: var(--raggio-sm);
  }

  .testi {
    flex: 1 1 auto;
    min-width: 0;
  }

  .nome {
    font-size: var(--corpo);
    font-weight: var(--peso-forte);
    color: var(--testo);
  }

  .sotto {
    font-size: var(--minuto);
    color: var(--testo-2);
    font-family: var(--famiglia-mono);
  }

  .extra {
    margin-top: var(--sp-2);
    min-width: 0;
  }

  .destra {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 2px;
    flex: 0 0 auto;
    text-align: right;
  }

  .valore {
    font-size: var(--piccolo);
    font-weight: var(--peso-forte);
    color: var(--testo);
    white-space: nowrap;
  }

  .dettaglio {
    font-size: var(--micro);
    color: var(--testo-3);
    white-space: nowrap;
  }

  /* Toni della tile, sui token di tipo dell'app. */
  .tile.documento {
    color: var(--documento);
    background: var(--documento-bg);
  }
  .tile.tracciato {
    color: var(--tracciato);
    background: var(--tracciato-bg);
  }
  .tile.archivio {
    color: var(--archivio);
    background: var(--archivio-bg);
  }
  .tile.media {
    color: var(--media);
    background: var(--media-bg);
  }
  .tile.installer {
    color: var(--installer);
    background: var(--installer-bg);
  }
  .tile.artefatto {
    color: var(--artefatto);
    background: var(--artefatto-bg);
  }
  .tile.altro {
    color: var(--altro);
    background: var(--altro-bg);
  }
</style>
