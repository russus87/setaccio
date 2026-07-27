<script module lang="ts">
  /** Una tessera della treemap. */
  export interface VoceTreemap {
    /** Identificatore stabile: torna indietro in `onseleziona`. */
    id: number;
    /** Etichetta scritta dentro la tessera, se ci sta. */
    nome: string;
    /** Etichetta del raggruppamento: le voci con lo stesso valore stanno insieme. */
    gruppo: string;
    /** Quanto pesa: è l'area. */
    valore: number;
    /** Colore di fondo, di solito un token (`var(--documento)`, …). */
    colore: string;
    /** Riga aggiuntiva per il tooltip e per i lettori di schermo. */
    dettaglio?: string;
  }
</script>

<script lang="ts">
  /**
   * Treemap a due livelli: prima si dispongono i gruppi nel riquadro, poi le
   * voci dentro il gruppo che è toccato loro.
   *
   * È scritta con dei `div` posizionati invece che in SVG — al contrario di
   * `BarChart`, che l'SVG ce l'ha per buoni motivi. Qui le tessere sono
   * centinaia e sono *cliccabili*: servono focus da tastiera, troncamento del
   * testo con i puntini e `aria-pressed`, che nel DOM si hanno gratis e in SVG
   * andrebbero rifatti a mano.
   *
   * L'algoritmo di disposizione sta in `treemap.ts`, puro e senza DOM.
   */
  import { squarifica, type PiastraTreemap } from "./treemap";

  interface Props {
    /** Le tessere. L'ordine non conta: dispone per peso decrescente. */
    voci: VoceTreemap[];
    /** Altezza del riquadro in pixel. */
    altezza?: number;
    /** Gli id già scelti: disegnati con la cornice d'accento. */
    scelti?: number[];
    /** Se manca, le tessere non sono cliccabili e restano puramente descrittive. */
    onseleziona?: (id: number) => void;
    /** Come scrivere il peso nelle etichette e nei tooltip. */
    formato?: (n: number) => string;
    /** Messaggio quando non c'è niente da disporre. */
    messaggioVuoto?: string;
  }

  let {
    voci,
    altezza = 420,
    scelti = [],
    onseleziona,
    formato = (n) => String(n),
    messaggioVuoto = "Nessun dato da disporre",
  }: Props = $props();

  /** Alta quanto basta a scrivere il nome del gruppo sopra le sue tessere. */
  const CIMA = 20;

  /** Sotto queste misure l'etichetta non ci sta e si scrive solo il tooltip. */
  const MIN_NOME = { w: 54, h: 26 };
  const MIN_PESO_H = 40;
  const MIN_TITOLO = { w: 90, h: 46 };

  let larghezza = $state(0);

  const scelte = $derived(new Set(scelti));
  const vuoto = $derived(voci.length === 0 || voci.every((v) => v.valore <= 0));

  interface Gruppo {
    nome: string;
    voci: VoceTreemap[];
    totale: number;
  }

  /** Le voci raccolte per gruppo, nell'ordine in cui compaiono la prima volta. */
  const gruppi = $derived.by<Gruppo[]>(() => {
    const per = new Map<string, Gruppo>();
    for (const v of voci) {
      if (!(v.valore > 0)) continue;
      let g = per.get(v.gruppo);
      if (!g) {
        g = { nome: v.gruppo, voci: [], totale: 0 };
        per.set(v.gruppo, g);
      }
      g.voci.push(v);
      g.totale += v.valore;
    }
    return [...per.values()];
  });

  interface Disposto {
    gruppo: Gruppo;
    riquadro: PiastraTreemap<Gruppo>;
    titolo: boolean;
    tessere: PiastraTreemap<VoceTreemap>[];
  }

  const disposizione = $derived.by<Disposto[]>(() => {
    if (larghezza <= 0 || altezza <= 0 || gruppi.length === 0) return [];

    const riquadri = squarifica(
      gruppi.map((g) => ({ dato: g, valore: g.totale })),
      0,
      0,
      larghezza,
      altezza,
    );

    return riquadri.map((r) => {
      // Il titolo del gruppo si scrive solo se resta spazio anche per le
      // tessere: altrimenti mangerebbe tutto il riquadro.
      const titolo = r.h > MIN_TITOLO.h && r.w > MIN_TITOLO.w;
      const cima = titolo ? CIMA : 0;
      return {
        gruppo: r.dato,
        riquadro: r,
        titolo,
        tessere: squarifica(
          r.dato.voci.map((v) => ({ dato: v, valore: v.valore })),
          0,
          cima,
          r.w,
          Math.max(0, r.h - cima),
        ),
      };
    });
  });

  function etichetta(v: VoceTreemap): string {
    const testa = `${v.nome} — ${formato(v.valore)}`;
    return v.dettaglio ? `${testa} · ${v.dettaglio}` : testa;
  }
</script>

<div class="riquadro" style="height: {altezza}px" bind:clientWidth={larghezza}>
  {#if vuoto}
    <p class="vuoto">{messaggioVuoto}</p>
  {:else}
    {#each disposizione as d (d.gruppo.nome)}
      <div
        class="gruppo"
        style="left: {d.riquadro.x}px; top: {d.riquadro.y}px; width: {d.riquadro
          .w}px; height: {d.riquadro.h}px"
      >
        {#if d.titolo}
          <div class="titolo-gruppo" title={d.gruppo.nome}>
            <span class="troncato">{d.gruppo.nome}</span>
            <span class="peso-gruppo">{formato(d.gruppo.totale)}</span>
          </div>
        {/if}

        {#each d.tessere as t (t.dato.id)}
          {@const v = t.dato}
          {@const attiva = scelte.has(v.id)}
          {@const stile = `left: ${t.x}px; top: ${t.y}px; width: ${t.w}px; height: ${t.h}px; background: ${v.colore}`}
          {#if onseleziona}
            <button
              type="button"
              class="tessera"
              class:scelta={attiva}
              style={stile}
              aria-pressed={attiva}
              aria-label={etichetta(v)}
              title={etichetta(v)}
              onclick={() => onseleziona?.(v.id)}
            >
              {#if t.w > MIN_NOME.w && t.h > MIN_NOME.h}
                <span class="nome">{v.nome}</span>
                {#if t.h > MIN_PESO_H}
                  <span class="peso">{formato(v.valore)}</span>
                {/if}
              {/if}
            </button>
          {:else}
            <div class="tessera" style={stile} role="img" aria-label={etichetta(v)} title={etichetta(v)}>
              {#if t.w > MIN_NOME.w && t.h > MIN_NOME.h}
                <span class="nome">{v.nome}</span>
                {#if t.h > MIN_PESO_H}
                  <span class="peso">{formato(v.valore)}</span>
                {/if}
              {/if}
            </div>
          {/if}
        {/each}
      </div>
    {/each}
  {/if}
</div>

<style>
  .riquadro {
    position: relative;
    width: 100%;
    min-width: 0;
  }

  .gruppo {
    position: absolute;
    border-radius: var(--raggio-sm);
    overflow: hidden;
  }

  .titolo-gruppo {
    position: absolute;
    inset: 0 0 auto 0;
    height: 20px;
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: 0 var(--sp-2);
    font-size: var(--micro);
    font-weight: var(--peso-forte);
    color: var(--testo-2);
    white-space: nowrap;
    overflow: hidden;
    pointer-events: none;
    z-index: 2;
  }

  .troncato {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .peso-gruppo {
    margin-left: auto;
    color: var(--testo-3);
    font-variant-numeric: tabular-nums;
    flex: none;
  }

  .tessera {
    position: absolute;
    display: block;
    text-align: left;
    padding: 5px 6px;
    overflow: hidden;
    border: 1px solid var(--sfondo);
    border-radius: 4px;
    /* I token dei tipi sono pensati per il testo, quindi sono scuri sul tema
       chiaro e chiari su quello scuro. Usandoli come fondo, il colore che
       resta leggibile sopra è sempre quello dello sfondo della pagina. */
    color: var(--sfondo);
    transition: filter var(--transizione);
  }

  .tessera:hover {
    filter: brightness(1.12);
    z-index: 3;
  }

  .tessera.scelta {
    box-shadow:
      inset 0 0 0 2px var(--accento),
      inset 0 0 0 4px var(--su-accento);
    z-index: 2;
  }

  .nome {
    display: block;
    font-size: var(--micro);
    font-weight: var(--peso-forte);
    line-height: 1.2;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .peso {
    display: block;
    font-size: var(--micro);
    line-height: 1.3;
    opacity: 0.82;
    font-variant-numeric: tabular-nums;
  }

  .vuoto {
    padding: var(--sp-8) 0;
    text-align: center;
    color: var(--testo-3);
    font-size: var(--minuto);
  }
</style>
