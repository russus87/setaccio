<script lang="ts">
  import type { Snippet } from "svelte";
  import Bottone from "./Bottone.svelte";
  import Icona, { type NomeIcona } from "./Icona.svelte";

  interface Props {
    /** Icona nel cerchio in cima. */
    icona?: NomeIcona;
    /** Titolo dello stato vuoto: dì cosa manca, non «nessun risultato». */
    titolo: string;
    /** Una o due righe che spiegano cosa fare. */
    messaggio?: string;
    /** Testo del bottone principale. Ignorato se passi lo snippet `azione`. */
    testoAzione?: string;
    /** Icona del bottone principale. */
    iconaAzione?: NomeIcona;
    onazione?: () => void;
    /** Sostituisce il bottone con contenuto arbitrario. */
    azione?: Snippet;
    /** Versione compatta, per riquadri piccoli. */
    compatto?: boolean;
  }

  let {
    icona = "ricerca",
    titolo,
    messaggio,
    testoAzione,
    iconaAzione,
    onazione,
    azione,
    compatto = false,
  }: Props = $props();
</script>

<div class="vuoto" class:compatto>
  <span class="cerchio">
    <Icona nome={icona} dimensione={compatto ? 20 : 26} spessore={1.4} />
  </span>

  <h3 class="titolo">{titolo}</h3>
  {#if messaggio}<p class="messaggio">{messaggio}</p>{/if}

  {#if azione}
    <div class="azione">{@render azione()}</div>
  {:else if testoAzione}
    <div class="azione">
      <Bottone variante="primario" icona={iconaAzione} onclick={onazione}>
        {testoAzione}
      </Bottone>
    </div>
  {/if}
</div>

<style>
  .vuoto {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--sp-3);
    padding: var(--sp-9) var(--sp-6);
    text-align: center;
  }

  .compatto {
    padding: var(--sp-6) var(--sp-4);
    gap: var(--sp-2);
  }

  .cerchio {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 56px;
    height: 56px;
    margin-bottom: var(--sp-1);
    border-radius: var(--raggio-lg);
    background: var(--superficie-2);
    border: 1px solid var(--bordo);
    color: var(--testo-3);
  }

  .compatto .cerchio {
    width: 40px;
    height: 40px;
    border-radius: var(--raggio);
  }

  .titolo {
    font-size: var(--medio);
    font-weight: var(--peso-grasso);
    color: var(--testo);
  }

  .compatto .titolo {
    font-size: var(--corpo);
  }

  .messaggio {
    max-width: 44ch;
    font-size: var(--piccolo);
    color: var(--testo-2);
    line-height: var(--riga-larga);
  }

  .azione {
    margin-top: var(--sp-2);
  }
</style>
