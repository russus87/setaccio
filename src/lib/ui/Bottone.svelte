<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";
  import Icona, { type NomeIcona } from "./Icona.svelte";

  export type VarianteBottone =
    | "primario"
    | "secondario"
    | "pericolo"
    | "fantasma";

  interface Props extends HTMLButtonAttributes {
    /** `primario` usa il gradiente lime; è l'unico da usare una volta per vista. */
    variante?: VarianteBottone;
    dimensione?: "sm" | "md" | "lg";
    /** Icona a sinistra del testo. */
    icona?: NomeIcona;
    /** Icona a destra del testo (per «avanti», «apri», …). */
    iconaDestra?: NomeIcona;
    /** Nasconde il testo e rende il bottone quadrato: serve `titolo`. */
    soloIcona?: boolean;
    caricamento?: boolean;
    /** Occupa tutta la larghezza disponibile. */
    piena?: boolean;
    /** Testo del tooltip, obbligatorio di fatto quando `soloIcona`. */
    titolo?: string;
    children?: Snippet;
  }

  let {
    variante = "secondario",
    dimensione = "md",
    icona,
    iconaDestra,
    soloIcona = false,
    caricamento = false,
    piena = false,
    titolo,
    disabled = false,
    type = "button",
    children,
    ...resto
  }: Props = $props();

  const lato = $derived(dimensione === "sm" ? 15 : dimensione === "lg" ? 19 : 17);
</script>

<button
  {type}
  class="bottone {variante} {dimensione}"
  class:piena
  class:solo-icona={soloIcona}
  class:in-caricamento={caricamento}
  disabled={disabled || caricamento}
  aria-busy={caricamento || undefined}
  aria-label={soloIcona ? titolo : undefined}
  title={titolo}
  {...resto}
>
  {#if caricamento}
    <span class="rotella" aria-hidden="true"></span>
  {:else if icona}
    <Icona nome={icona} dimensione={lato} />
  {/if}

  {#if !soloIcona && children}
    <span class="etichetta">{@render children()}</span>
  {/if}

  {#if iconaDestra && !soloIcona}
    <Icona nome={iconaDestra} dimensione={lato} />
  {/if}
</button>

<style>
  .bottone {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--sp-2);
    height: var(--altezza-controllo);
    padding: 0 var(--sp-4);
    border-radius: var(--raggio-pillola);
    font-size: var(--corpo);
    font-weight: var(--peso-forte);
    line-height: 1;
    white-space: nowrap;
    transition:
      background var(--transizione),
      color var(--transizione),
      border-color var(--transizione),
      box-shadow var(--transizione),
      transform var(--transizione);
  }

  .bottone:active:not(:disabled) {
    transform: translateY(1px);
  }

  .bottone:disabled {
    opacity: 0.45;
  }

  .etichetta {
    display: inline-block;
  }

  /* Dimensioni ------------------------------------------------------- */
  .sm {
    height: 30px;
    padding: 0 var(--sp-3);
    font-size: var(--minuto);
    gap: var(--sp-1);
  }

  .lg {
    height: var(--altezza-controllo-lg);
    padding: 0 var(--sp-6);
    font-size: var(--medio);
  }

  .solo-icona {
    padding: 0;
    width: var(--altezza-controllo);
  }

  .solo-icona.sm {
    width: 30px;
  }

  .solo-icona.lg {
    width: var(--altezza-controllo-lg);
  }

  .piena {
    width: 100%;
  }

  /* Varianti --------------------------------------------------------- */
  .primario {
    background: var(--gradiente-accento);
    color: var(--su-accento);
    box-shadow: var(--ombra-1);
  }

  .primario:hover:not(:disabled) {
    box-shadow: var(--ombra-accento);
  }

  .secondario {
    background: var(--superficie-2);
    color: var(--testo);
    border: 1px solid var(--bordo);
  }

  .secondario:hover:not(:disabled) {
    background: var(--superficie-3);
    border-color: var(--bordo-forte);
  }

  .pericolo {
    background: var(--pericolo-bg);
    color: var(--pericolo);
    border: 1px solid transparent;
  }

  .pericolo:hover:not(:disabled) {
    border-color: var(--pericolo);
  }

  .fantasma {
    background: transparent;
    color: var(--testo-2);
  }

  .fantasma:hover:not(:disabled) {
    background: var(--superficie-2);
    color: var(--testo);
  }

  /* Caricamento ------------------------------------------------------ */
  .rotella {
    width: 15px;
    height: 15px;
    border-radius: 50%;
    border: 2px solid currentColor;
    border-top-color: transparent;
    opacity: 0.8;
    animation: gira 700ms linear infinite;
  }

  @keyframes gira {
    to {
      transform: rotate(360deg);
    }
  }
</style>
