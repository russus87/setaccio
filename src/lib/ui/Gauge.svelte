<script lang="ts">
  /**
   * Arco a 240° con traccia grigia e riempimento a gradiente lime, come nel
   * mockup. Il valore sta al centro dell'arco.
   */
  interface Props {
    /** Valore corrente. */
    valore: number;
    /** Fondo scala. */
    massimo?: number;
    /** Testo grande al centro. Se manca si mostra la percentuale. */
    testo?: string;
    /** Unità piccola accanto al testo grande. */
    unita?: string;
    /** Etichetta grigia sotto il valore. */
    etichetta?: string;
    /** Lato in pixel dell'arco (l'altezza è ricavata: circa 3/4 del lato). */
    dimensione?: number;
    /** Spessore del tratto. */
    spessore?: number;
    /** Descrizione per i lettori di schermo. */
    titolo?: string;
  }

  let {
    valore,
    massimo = 100,
    testo,
    unita,
    etichetta,
    dimensione = 200,
    spessore = 20,
    titolo,
  }: Props = $props();

  const idGradiente = $props.id();

  const r = $derived((dimensione - spessore) / 2);
  const cx = $derived(dimensione / 2);
  const cy = $derived(spessore / 2 + r);
  const altezza = $derived(1.5 * r + spessore);

  // L'arco parte da 150° e ne copre 240 in senso orario (y verso il basso):
  // resta aperto in basso, come nel mockup.
  const INIZIO = 150;
  const AMPIEZZA = 240;

  const frazione = $derived(
    massimo > 0 ? Math.min(Math.max(valore / massimo, 0), 1) : 0,
  );

  function punto(angoloGradi: number, raggio: number): [number, number] {
    const a = (angoloGradi * Math.PI) / 180;
    return [cx + raggio * Math.cos(a), cy + raggio * Math.sin(a)];
  }

  function arco(daGradi: number, aGradi: number, raggio: number): string {
    const [x1, y1] = punto(daGradi, raggio);
    const [x2, y2] = punto(aGradi, raggio);
    const grande = Math.abs(aGradi - daGradi) > 180 ? 1 : 0;
    return `M ${x1.toFixed(2)} ${y1.toFixed(2)} A ${raggio.toFixed(2)} ${raggio.toFixed(2)} 0 ${grande} 1 ${x2.toFixed(2)} ${y2.toFixed(2)}`;
  }

  const traccia = $derived(arco(INIZIO, INIZIO + AMPIEZZA, r));
  const riempimento = $derived(arco(INIZIO, INIZIO + AMPIEZZA * frazione, r));

  const percentuale = $derived(Math.round(frazione * 100));
  const centro = $derived(testo ?? `${percentuale}%`);
</script>

<figure class="gauge" style="width: {dimensione}px">
  <svg
    viewBox="0 0 {dimensione} {altezza}"
    width={dimensione}
    height={altezza}
    role="img"
    aria-label={titolo ?? `${etichetta ?? "Valore"}: ${centro}`}
  >
    <defs>
      <linearGradient id="grad-{idGradiente}" x1="0" y1="1" x2="1" y2="0">
        <stop offset="0%" style="stop-color: var(--accento-scuro)" />
        <stop offset="55%" style="stop-color: var(--accento)" />
        <stop offset="100%" style="stop-color: var(--accento-chiaro)" />
      </linearGradient>
    </defs>

    <path
      class="traccia"
      d={traccia}
      fill="none"
      stroke-width={spessore}
      stroke-linecap="round"
    />

    {#if frazione > 0}
      <path
        class="riempimento"
        d={riempimento}
        fill="none"
        stroke="url(#grad-{idGradiente})"
        stroke-width={spessore}
        stroke-linecap="round"
      />
    {/if}

    <text class="valore cifre" x={cx} y={cy - r * 0.06} text-anchor="middle">
      {centro}{#if unita}<tspan class="unita" dx="4">{unita}</tspan>{/if}
    </text>

    {#if etichetta}
      <text class="etichetta" x={cx} y={cy + r * 0.28} text-anchor="middle">
        {etichetta}
      </text>
    {/if}
  </svg>
</figure>

<style>
  .gauge {
    margin: 0 auto;
    max-width: 100%;
  }

  svg {
    max-width: 100%;
    height: auto;
    overflow: visible;
  }

  .traccia {
    stroke: var(--traccia);
  }

  .riempimento {
    transition: d var(--transizione-lenta);
  }

  .valore {
    fill: var(--testo);
    font-size: var(--cifra-grande);
    font-weight: var(--peso-grasso);
    letter-spacing: -0.02em;
    dominant-baseline: middle;
  }

  .unita {
    fill: var(--testo-2);
    font-size: var(--medio);
    font-weight: var(--peso-medio);
    letter-spacing: 0;
  }

  .etichetta {
    fill: var(--testo-2);
    font-size: var(--minuto);
    font-weight: var(--peso-medio);
    dominant-baseline: hanging;
  }
</style>
