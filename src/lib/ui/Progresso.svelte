<script lang="ts">
  interface Props {
    /** Valore corrente. */
    valore?: number;
    /** Fondo scala. */
    massimo?: number;
    /** Etichetta a sinistra, sopra la barra. */
    etichetta?: string;
    /** Testo a destra: se manca e `mostraPercentuale`, si usa la percentuale. */
    dettaglio?: string;
    mostraPercentuale?: boolean;
    /** Barra che scorre da sola: per attese di durata ignota. */
    indeterminato?: boolean;
    dimensione?: "sm" | "md";
    /** Colore alternativo alla barra lime, con i token semantici. */
    tono?: "accento" | "info" | "avviso" | "pericolo" | "successo";
  }

  let {
    valore = 0,
    massimo = 100,
    etichetta,
    dettaglio,
    mostraPercentuale = true,
    indeterminato = false,
    dimensione = "md",
    tono = "accento",
  }: Props = $props();

  const frazione = $derived(
    massimo > 0 ? Math.min(Math.max(valore / massimo, 0), 1) : 0,
  );
  const percentuale = $derived(Math.round(frazione * 100));
  const testoDestra = $derived(
    dettaglio ?? (mostraPercentuale && !indeterminato ? `${percentuale}%` : ""),
  );
</script>

<div class="progresso {dimensione}">
  {#if etichetta || testoDestra}
    <div class="testa">
      {#if etichetta}<span class="etichetta troncato">{etichetta}</span>{/if}
      {#if testoDestra}<span class="dettaglio cifre">{testoDestra}</span>{/if}
    </div>
  {/if}

  <div
    class="pista"
    role="progressbar"
    aria-valuemin={indeterminato ? undefined : 0}
    aria-valuemax={indeterminato ? undefined : massimo}
    aria-valuenow={indeterminato ? undefined : valore}
    aria-label={etichetta}
  >
    {#if indeterminato}
      <div class="barra {tono} indeterminata"></div>
    {:else}
      <div class="barra {tono}" style="width: {frazione * 100}%"></div>
    {/if}
  </div>
</div>

<style>
  .progresso {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
  }

  .testa {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--sp-3);
    font-size: var(--minuto);
  }

  .etichetta {
    color: var(--testo-2);
  }

  .dettaglio {
    color: var(--testo);
    font-weight: var(--peso-forte);
    flex: 0 0 auto;
  }

  .pista {
    position: relative;
    height: 8px;
    border-radius: var(--raggio-pillola);
    background: var(--traccia);
    overflow: hidden;
  }

  .sm .pista {
    height: 5px;
  }

  .barra {
    height: 100%;
    border-radius: var(--raggio-pillola);
    transition: width var(--transizione-lenta);
  }

  .accento {
    background: var(--gradiente-accento);
  }

  .info {
    background: var(--info);
  }

  .avviso {
    background: var(--avviso);
  }

  .pericolo {
    background: var(--pericolo);
  }

  .successo {
    background: var(--successo);
  }

  .indeterminata {
    width: 35%;
    animation: scorri 1.3s ease-in-out infinite;
  }

  @keyframes scorri {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(320%);
    }
  }
</style>
