<script lang="ts">
  interface Props {
    /** Stato del toggle, associabile con `bind:attivo`. */
    attivo?: boolean;
    etichetta?: string;
    /** Riga piccola e grigia sotto l'etichetta. */
    descrizione?: string;
    disabilitato?: boolean;
    dimensione?: "sm" | "md";
    /** Mette etichetta e descrizione a sinistra e il toggle a destra. */
    sparso?: boolean;
    /** Etichetta accessibile quando non c'è testo visibile. */
    titolo?: string;
    onchange?: (attivo: boolean) => void;
  }

  let {
    attivo = $bindable(false),
    etichetta,
    descrizione,
    disabilitato = false,
    dimensione = "md",
    sparso = false,
    titolo,
    onchange,
  }: Props = $props();

  function commuta() {
    if (disabilitato) return;
    attivo = !attivo;
    onchange?.(attivo);
  }
</script>

<div class="riga-interruttore {dimensione}" class:sparso class:inattivo={disabilitato}>
  {#if etichetta || descrizione}
    <div class="testi">
      {#if etichetta}<span class="etichetta">{etichetta}</span>{/if}
      {#if descrizione}<span class="descrizione">{descrizione}</span>{/if}
    </div>
  {/if}

  <button
    type="button"
    role="switch"
    class="pista"
    class:acceso={attivo}
    aria-checked={attivo}
    aria-label={titolo ?? etichetta}
    disabled={disabilitato}
    onclick={commuta}
  >
    <span class="pomello"></span>
  </button>
</div>

<style>
  .riga-interruttore {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-width: 0;
  }

  .sparso {
    justify-content: space-between;
    width: 100%;
  }

  .testi {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .etichetta {
    font-size: var(--corpo);
    font-weight: var(--peso-medio);
    color: var(--testo);
  }

  .descrizione {
    font-size: var(--minuto);
    color: var(--testo-2);
  }

  .pista {
    position: relative;
    flex: 0 0 auto;
    width: 44px;
    height: 26px;
    border-radius: var(--raggio-pillola);
    background: var(--superficie-3);
    border: 1px solid var(--bordo);
    transition:
      background var(--transizione),
      border-color var(--transizione);
  }

  .sm .pista {
    width: 36px;
    height: 21px;
  }

  .pista:hover:not(:disabled) {
    border-color: var(--bordo-forte);
  }

  .pista.acceso {
    background: var(--gradiente-accento);
    border-color: transparent;
  }

  .pomello {
    position: absolute;
    top: 50%;
    left: 3px;
    width: 18px;
    height: 18px;
    margin-top: -9px;
    border-radius: 50%;
    background: var(--sfondo);
    box-shadow: var(--ombra-1);
    transition: transform var(--transizione);
  }

  .sm .pomello {
    width: 15px;
    height: 15px;
    margin-top: -7.5px;
  }

  .acceso .pomello {
    transform: translateX(18px);
    background: var(--su-accento);
  }

  .sm .acceso .pomello {
    transform: translateX(15px);
  }

  .inattivo {
    opacity: 0.5;
  }
</style>
