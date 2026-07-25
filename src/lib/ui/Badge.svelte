<script lang="ts">
  import type { Snippet } from "svelte";
  import type { Stato, Tipo } from "../api";

  export type VarianteBadge =
    | "neutro"
    | "accento"
    | "info"
    | "successo"
    | "avviso"
    | "pericolo";

  interface Props {
    /** Testo della pillola. Se manca si usa `children`, poi `tipo`/`stato`. */
    testo?: string;
    /** Colora la pillola secondo il tipo di file e ne usa il nome. */
    tipo?: Tipo;
    /** Colora la pillola secondo lo stato del file e ne usa il nome. */
    stato?: Stato;
    /** Colore generico, quando non si tratta né di tipo né di stato. */
    variante?: VarianteBadge;
    dimensione?: "sm" | "md";
    /** Aggiunge il pallino colorato a sinistra. */
    punto?: boolean;
    /** Solo bordo, senza fondo pieno: per pillole molto fitte. */
    contorno?: boolean;
    children?: Snippet;
  }

  let {
    testo,
    tipo,
    stato,
    variante = "neutro",
    dimensione = "sm",
    punto = false,
    contorno = false,
    children,
  }: Props = $props();

  // La classe determina quali token di colore leggere: i tipi e gli stati
  // hanno token propri (--documento, --canonico, …) definiti in app.css.
  const classeColore = $derived(tipo ?? stato ?? variante);
  const etichetta = $derived(testo ?? tipo ?? stato ?? "");
</script>

<span class="badge {classeColore} {dimensione}" class:contorno>
  {#if punto}<span class="punto" aria-hidden="true"></span>{/if}
  {#if children}{@render children()}{:else}{etichetta}{/if}
</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
    border-radius: var(--raggio-pillola);
    font-weight: var(--peso-forte);
    line-height: 1;
    white-space: nowrap;
    border: 1px solid transparent;
    text-transform: lowercase;
    font-variant-caps: normal;
  }

  .sm {
    height: 20px;
    padding: 0 var(--sp-2);
    font-size: var(--micro);
  }

  .md {
    height: 26px;
    padding: 0 var(--sp-3);
    font-size: var(--minuto);
  }

  .punto {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
  }

  .contorno {
    background: transparent !important;
    border-color: currentColor;
    opacity: 0.9;
  }

  /* Colori generici -------------------------------------------------- */
  .neutro {
    color: var(--testo-2);
    background: var(--superficie-2);
    border-color: var(--bordo);
  }

  .accento {
    color: var(--accento-testo);
    background: var(--accento-tenue);
  }

  .info {
    color: var(--info);
    background: var(--info-bg);
  }

  .successo {
    color: var(--successo);
    background: var(--successo-bg);
  }

  .avviso {
    color: var(--avviso);
    background: var(--avviso-bg);
  }

  .pericolo {
    color: var(--pericolo);
    background: var(--pericolo-bg);
  }

  /* Stati del file --------------------------------------------------- */
  .canonico {
    color: var(--canonico);
    background: var(--canonico-bg);
  }

  .duplicato {
    color: var(--duplicato);
    background: var(--duplicato-bg);
  }

  .orfano {
    color: var(--orfano);
    background: var(--orfano-bg);
  }

  /* Tipi di file ----------------------------------------------------- */
  .documento {
    color: var(--documento);
    background: var(--documento-bg);
  }

  .tracciato {
    color: var(--tracciato);
    background: var(--tracciato-bg);
  }

  .archivio {
    color: var(--archivio);
    background: var(--archivio-bg);
  }

  .media {
    color: var(--media);
    background: var(--media-bg);
  }

  .installer {
    color: var(--installer);
    background: var(--installer-bg);
  }

  .artefatto {
    color: var(--artefatto);
    background: var(--artefatto-bg);
  }

  .altro {
    color: var(--altro);
    background: var(--altro-bg);
  }
</style>
