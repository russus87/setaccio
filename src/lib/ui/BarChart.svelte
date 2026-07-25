<script module lang="ts">
  export interface SerieBarre {
    nome: string;
    valori: number[];
    /** Colore CSS. Se omesso si prendono, in ordine, i token --serie-1/2/3. */
    colore?: string;
  }
</script>

<script lang="ts">
  /**
   * Barre raggruppate in SVG scritto a mano: griglia orizzontale leggera,
   * asse Y con poche tacche, barre a estremità arrotondate come nel mockup.
   */
  interface Props {
    /** Etichette dell'asse X, una per gruppo. */
    etichette: string[];
    /** Le serie: ognuna ha un valore per ciascuna etichetta. */
    serie: SerieBarre[];
    /** Altezza del disegno in pixel. */
    altezza?: number;
    /** Fondo scala. Se omesso si ricava dal massimo dei dati. */
    massimo?: number;
    /** Quante linee di griglia (oltre allo zero). */
    tacche?: number;
    /** Come scrivere i numeri sull'asse Y e nei tooltip. */
    formato?: (n: number) => string;
    /** Mostra la legenda sopra il grafico. */
    legenda?: boolean;
    /** Messaggio quando non ci sono dati. */
    messaggioVuoto?: string;
  }

  let {
    etichette,
    serie,
    altezza = 260,
    massimo,
    tacche = 4,
    formato = (n) => String(n),
    legenda = true,
    messaggioVuoto = "Nessun dato da mostrare",
  }: Props = $props();

  const LARGHEZZA = 680;
  const MARGINE = { su: 12, giu: 30, sx: 52, dx: 10 };

  const colori = ["var(--serie-1)", "var(--serie-3)", "var(--serie-2)"];

  const larghezzaGrafico = $derived(LARGHEZZA - MARGINE.sx - MARGINE.dx);
  const altezzaGrafico = $derived(altezza - MARGINE.su - MARGINE.giu);

  const vuoto = $derived(
    etichette.length === 0 ||
      serie.length === 0 ||
      serie.every((s) => s.valori.every((v) => !v)),
  );

  const scala = $derived.by(() => {
    const trovato = Math.max(
      0,
      ...serie.flatMap((s) => s.valori.map((v) => (Number.isFinite(v) ? v : 0))),
    );
    const grezzo = massimo ?? trovato;
    if (grezzo <= 0) return 1;
    // Arrotonda a una cifra significativa in alto: fa uscire tacche leggibili.
    const potenza = Math.pow(10, Math.floor(Math.log10(grezzo)));
    return Math.ceil(grezzo / potenza) * potenza;
  });

  const livelli = $derived(
    Array.from({ length: tacche + 1 }, (_, i) => (scala / tacche) * i),
  );

  const passoGruppo = $derived(
    etichette.length > 0 ? larghezzaGrafico / etichette.length : larghezzaGrafico,
  );

  const larghezzaBarra = $derived(
    Math.max(4, Math.min(16, (passoGruppo * 0.62) / Math.max(serie.length, 1))),
  );

  const larghezzaGruppo = $derived(larghezzaBarra * serie.length + 3 * (serie.length - 1));

  function y(valore: number): number {
    return MARGINE.su + altezzaGrafico - (valore / scala) * altezzaGrafico;
  }

  function x(indiceGruppo: number, indiceSerie: number): number {
    const centro = MARGINE.sx + passoGruppo * (indiceGruppo + 0.5);
    return centro - larghezzaGruppo / 2 + indiceSerie * (larghezzaBarra + 3);
  }

  function coloreDi(s: SerieBarre, i: number): string {
    return s.colore ?? colori[i % colori.length];
  }
</script>

<div class="grafico">
  {#if legenda && !vuoto}
    <ul class="legenda">
      {#each serie as s, i (i)}
        <li>
          <span class="pastiglia" style="background: {coloreDi(s, i)}"></span>
          {s.nome}
        </li>
      {/each}
    </ul>
  {/if}

  {#if vuoto}
    <p class="vuoto">{messaggioVuoto}</p>
  {:else}
    <svg
      viewBox="0 0 {LARGHEZZA} {altezza}"
      width="100%"
      height={altezza}
      preserveAspectRatio="xMidYMid meet"
      role="img"
      aria-label="Grafico a barre: {serie.map((s) => s.nome).join(', ')}"
    >
      <!-- Griglia e asse Y -->
      {#each livelli as livello, t (t)}
        <line
          class="griglia"
          x1={MARGINE.sx}
          x2={LARGHEZZA - MARGINE.dx}
          y1={y(livello)}
          y2={y(livello)}
        />
        <text class="tacca" x={MARGINE.sx - 10} y={y(livello)} text-anchor="end">
          {formato(livello)}
        </text>
      {/each}

      <!-- Barre -->
      {#each etichette as etichetta, g (g)}
        {#each serie as s, i (i)}
          {@const valore = Math.max(0, s.valori[g] ?? 0)}
          {@const alt = Math.max(valore > 0 ? larghezzaBarra : 0, y(0) - y(valore))}
          {#if alt > 0}
            <rect
              x={x(g, i)}
              y={y(0) - alt}
              width={larghezzaBarra}
              height={alt}
              rx={larghezzaBarra / 2}
              fill={coloreDi(s, i)}
            >
              <title>{s.nome} · {etichetta}: {formato(valore)}</title>
            </rect>
          {/if}
        {/each}

        <text
          class="etichetta-x"
          x={MARGINE.sx + passoGruppo * (g + 0.5)}
          y={altezza - 10}
          text-anchor="middle"
        >
          {etichetta}
        </text>
      {/each}
    </svg>
  {/if}
</div>

<style>
  .grafico {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    min-width: 0;
  }

  svg {
    width: 100%;
    height: auto;
    overflow: visible;
  }

  .legenda {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-4);
    font-size: var(--minuto);
    color: var(--testo-2);
  }

  .legenda li {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
  }

  .pastiglia {
    width: 10px;
    height: 10px;
    border-radius: 3px;
  }

  .griglia {
    stroke: var(--griglia);
    stroke-width: 1;
    stroke-dasharray: 2 4;
  }

  .tacca {
    fill: var(--testo-3);
    font-size: 11px;
    dominant-baseline: middle;
  }

  .etichetta-x {
    fill: var(--testo-2);
    font-size: 11px;
    font-weight: var(--peso-medio);
  }

  .vuoto {
    padding: var(--sp-8) 0;
    text-align: center;
    color: var(--testo-3);
    font-size: var(--minuto);
  }
</style>
