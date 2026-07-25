<script lang="ts">
  /**
   * La schermata in cui si insegna a Setaccio dove vanno le cose.
   *
   * Su un disco vero i non classificati sono decine di migliaia: non ha senso
   * chiedere una decisione file per file. Qui si sceglie un pattern fra quelli
   * proposti e si clicca il contesto: la regola nasce, il motore riapplica
   * tutto, e con un solo clic spariscono dalla coda tutti i file che quel
   * pattern intercetta. Il contatore in alto serve a farlo vedere.
   */
  import { openPath } from "@tauri-apps/plugin-opener";
  import {
    accorciaPath,
    codaRevisione,
    formattaByte,
    formattaNumero,
    regolaAggiungi,
    statistiche,
    type DaRevisionare,
    type Statistiche,
  } from "../api";
  import Badge from "../ui/Badge.svelte";
  import Bottone from "../ui/Bottone.svelte";
  import Campo from "../ui/Campo.svelte";
  import Card from "../ui/Card.svelte";
  import Icona from "../ui/Icona.svelte";
  import Progresso from "../ui/Progresso.svelte";
  import Vuoto from "../ui/Vuoto.svelte";
  import { iconaTipo, messaggioErrore } from "./comuni";

  interface Props {
    aggiornamento?: number;
  }

  let { aggiornamento = 0 }: Props = $props();

  const LIMITI = [50, 200, 500] as const;

  let limite = $state(50);
  let coda = $state<DaRevisionare[]>([]);
  let stat = $state<Statistiche | null>(null);
  let caricando = $state(true);
  let errore = $state<string | null>(null);
  let ricarica = $state(0);

  /** Pattern scelto per ciascun file: il primo è già selezionato. */
  let patternScelto = $state<Record<number, string>>({});
  /** Contesto scritto a mano, per i file senza vicini utili. */
  let contestoLibero = $state<Record<number, string>>({});
  /** Id del file su cui si sta creando la regola. */
  let inCorso = $state<number | null>(null);
  /** Ultima regola creata: si mostra per un attimo cosa è appena successo. */
  let ultimaRegola = $state<string | null>(null);

  $effect(() => {
    const l = limite;
    void aggiornamento;
    void ricarica;
    let vivo = true;
    caricando = true;
    Promise.all([codaRevisione(l), statistiche()])
      .then(([c, s]) => {
        if (!vivo) return;
        coda = c;
        stat = s;
        errore = null;
        const scelti: Record<number, string> = {};
        const liberi: Record<number, string> = {};
        for (const d of c) {
          if (d.pattern_suggeriti.length > 0) scelti[d.id] = d.pattern_suggeriti[0];
          liberi[d.id] = "";
        }
        patternScelto = scelti;
        contestoLibero = liberi;
      })
      .catch((e) => {
        if (!vivo) return;
        errore = messaggioErrore(e);
      })
      .finally(() => {
        if (vivo) caricando = false;
      });
    return () => {
      vivo = false;
    };
  });

  /** Quanti file hanno già un contesto, sul totale di quelli classificabili. */
  const classificabili = $derived(
    stat ? Math.max(stat.file_totali - stat.artefatti_esclusi, 0) : 0,
  );
  const collocati = $derived(
    stat ? Math.max(classificabili - stat.non_classificati, 0) : 0,
  );
  const percentuale = $derived(
    classificabili > 0 ? (collocati / classificabili) * 100 : 0,
  );

  /**
   * Priorità della regola nuova: più il pattern è specifico, prima va
   * valutato. Le regole di serie stanno fra 10 e 30, quindi le nuove restano
   * dopo di quelle a meno che non siano un nome esatto.
   */
  function prioritaDi(d: DaRevisionare, pattern: string): number {
    const i = d.pattern_suggeriti.indexOf(pattern);
    return 40 + (i < 0 ? 0 : i) * 10;
  }

  async function creaRegola(d: DaRevisionare, contesto: string) {
    const pattern = patternScelto[d.id] ?? d.nome;
    const valore = contesto.trim();
    if (!valore) return;

    inCorso = d.id;
    try {
      await regolaAggiungi(
        `${valore} da ${pattern}`,
        "contesto",
        pattern,
        valore,
        prioritaDi(d, pattern),
      );
      ultimaRegola = `«${pattern}» → ${valore}`;
      errore = null;
      // Il backend riapplica le regole da sé: basta rileggere la coda.
      ricarica += 1;
    } catch (e) {
      errore = messaggioErrore(e);
    } finally {
      inCorso = null;
    }
  }

  async function apri(path: string) {
    try {
      await openPath(path);
    } catch (e) {
      errore = messaggioErrore(e);
    }
  }
</script>

<div class="impila">
  <!-- Il progresso ---------------------------------------------------- -->
  <Card padding="stretta">
    <div class="impila-stretta">
      <div class="testa">
        <div class="crescente">
          <Progresso
            valore={percentuale}
            etichetta="File collocati in un contesto"
            dettaglio="{formattaNumero(collocati)} su {formattaNumero(classificabili)}"
          />
        </div>

        <div class="limiti" role="group" aria-label="Quanti file mostrare">
          {#each LIMITI as l (l)}
            <button
              class="limite"
              class:acceso={limite === l}
              aria-pressed={limite === l}
              onclick={() => (limite = l)}
            >
              {l}
            </button>
          {/each}
        </div>
      </div>

      <p class="spiega">
        Restano <strong class="cifre">{formattaNumero(stat?.non_classificati ?? 0)}</strong>
        file senza contesto. Non serve deciderli uno a uno: scegli un pattern e
        clicca un contesto — la regola nasce, viene riapplicata a tutto l'indice
        e la coda si accorcia di colpo.
      </p>

      {#if ultimaRegola}
        <p class="fatto">
          <Icona nome="check" dimensione={14} />
          Regola creata: {ultimaRegola}. Puoi rivederla e disattivarla dalle Impostazioni.
        </p>
      {/if}
    </div>
  </Card>

  {#if errore}
    <Card>
      <div class="allarme" role="alert">
        <Icona nome="avviso" dimensione={16} />
        <span class="crescente">{errore}</span>
        <Bottone
          variante="fantasma"
          dimensione="sm"
          icona="chiudi"
          soloIcona
          titolo="Chiudi l'avviso"
          onclick={() => (errore = null)}
        />
      </div>
    </Card>
  {/if}

  {#if caricando && coda.length === 0}
    <Card>
      <p class="testo-secondario testo-piccolo">Lettura della coda…</p>
    </Card>
  {:else if coda.length === 0}
    <Card>
      <Vuoto
        icona="check"
        titolo="Non c'è più niente da collocare"
        messaggio="Ogni file indicizzato ha trovato il suo contesto. Se aggiungi una sorgente nuova e riscansioni, quello che le regole non sanno collocare ricomparirà qui."
      />
    </Card>
  {:else}
    {#each coda as d (d.id)}
      <Card padding="stretta">
        <div class="voce">
          <!-- Il file -->
          <div class="capo">
            <span class="tile {d.tipo}">
              <Icona nome={iconaTipo(d.tipo)} dimensione={18} />
            </span>
            <div class="crescente">
              <p class="nome troncato" title={d.nome}>{d.nome}</p>
              <button
                class="path mono troncato"
                title="Apri {d.path}"
                onclick={() => apri(d.path)}
              >
                {accorciaPath(d.path, 78)}
              </button>
            </div>
            <div class="meta">
              <Badge tipo={d.tipo} />
              <span class="peso cifre">{formattaByte(d.size)}</span>
            </div>
          </div>

          <!-- I pattern -->
          <div class="scelte">
            <p class="etichetta-scelte">
              1. Su cosa deve valere la regola
              <span class="testo-tenue">— dal più specifico al più generale</span>
            </p>
            <div class="chip-riga">
              {#each d.pattern_suggeriti as p (p)}
                <button
                  class="chip pattern"
                  class:acceso={(patternScelto[d.id] ?? d.pattern_suggeriti[0]) === p}
                  aria-pressed={(patternScelto[d.id] ?? d.pattern_suggeriti[0]) === p}
                  onclick={() => (patternScelto = { ...patternScelto, [d.id]: p })}
                >
                  <span class="mono">{p}</span>
                </button>
              {/each}
            </div>
          </div>

          <!-- I contesti -->
          <div class="scelte">
            <p class="etichetta-scelte">
              2. Dove va
              <span class="testo-tenue">— un clic e la regola è fatta</span>
            </p>

            {#if d.contesti_vicini.length > 0}
              <div class="chip-riga">
                {#each d.contesti_vicini as c (c)}
                  <button
                    class="chip contesto"
                    disabled={inCorso === d.id}
                    onclick={() => creaRegola(d, c)}
                  >
                    <Icona nome="piu" dimensione={13} />
                    {c}
                  </button>
                {/each}
              </div>
            {:else}
              <p class="nota">
                Nessun file vicino ha un contesto da cui prendere spunto: scrivilo
                tu qui sotto.
              </p>
            {/if}

            <div class="libero">
              <Campo
                bind:valore={contestoLibero[d.id]}
                segnaposto="oppure scrivi un contesto nuovo…"
                autocomplete="off"
                spellcheck={false}
                oninvio={(v) => creaRegola(d, v)}
              />
              <Bottone
                variante="secondario"
                icona="piu"
                caricamento={inCorso === d.id}
                disabled={!(contestoLibero[d.id] ?? "").trim()}
                onclick={() => creaRegola(d, contestoLibero[d.id] ?? "")}
              >
                Crea
              </Bottone>
            </div>
          </div>
        </div>
      </Card>
    {/each}

    {#if coda.length >= limite}
      <p class="coda-fine testo-piccolo testo-secondario">
        Sono i {limite} file più grandi ancora senza contesto. Ogni regola che
        crei ne toglie di mezzo molti in una volta: rileggi la coda per vedere
        cosa resta.
      </p>
    {/if}
  {/if}
</div>

<style>
  .testa {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
  }

  .limiti {
    display: inline-flex;
    gap: 2px;
    padding: 3px;
    flex: 0 0 auto;
    border-radius: var(--raggio-pillola);
    background: var(--superficie-2);
    border: 1px solid var(--bordo);
  }

  .limite {
    min-width: 44px;
    height: 26px;
    padding: 0 var(--sp-2);
    border-radius: var(--raggio-pillola);
    font-size: var(--minuto);
    font-weight: var(--peso-forte);
    color: var(--testo-2);
    font-variant-numeric: tabular-nums;
  }

  .limite.acceso {
    background: var(--superficie);
    color: var(--testo);
    box-shadow: var(--ombra-1);
  }

  .spiega {
    font-size: var(--piccolo);
    color: var(--testo-2);
    line-height: var(--riga-larga);
  }

  .spiega strong {
    color: var(--testo);
  }

  .fatto {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-2) var(--sp-3);
    border-radius: var(--raggio-sm);
    background: var(--successo-bg);
    color: var(--successo);
    font-size: var(--minuto);
  }

  /* Voce della coda ---------------------------------------------------- */
  .voce {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    min-width: 0;
  }

  .capo {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-width: 0;
  }

  .tile {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 38px;
    height: 38px;
    flex: 0 0 auto;
    border-radius: var(--raggio);
    background: var(--superficie-2);
    color: var(--testo-2);
  }

  .nome {
    font-size: var(--corpo);
    font-weight: var(--peso-forte);
  }

  .path {
    display: block;
    max-width: 100%;
    font-size: var(--minuto);
    color: var(--testo-2);
    text-align: left;
  }

  .path:hover {
    color: var(--accento-testo);
    text-decoration: underline;
  }

  .meta {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    flex: 0 0 auto;
  }

  .peso {
    font-size: var(--piccolo);
    font-weight: var(--peso-forte);
    color: var(--testo-2);
    white-space: nowrap;
  }

  /* Scelte -------------------------------------------------------------- */
  .scelte {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding: var(--sp-3);
    border-radius: var(--raggio-lg);
    background: var(--superficie-2);
    min-width: 0;
  }

  .etichetta-scelte {
    font-size: var(--micro);
    font-weight: var(--peso-forte);
    color: var(--testo-2);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .etichetta-scelte span {
    font-weight: var(--peso-normale);
    text-transform: none;
    letter-spacing: 0;
  }

  .chip-riga {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
    height: 28px;
    padding: 0 var(--sp-3);
    border-radius: var(--raggio-pillola);
    border: 1px solid var(--bordo);
    background: var(--superficie);
    color: var(--testo-2);
    font-size: var(--minuto);
    font-weight: var(--peso-medio);
    white-space: nowrap;
    transition:
      background var(--transizione),
      color var(--transizione),
      border-color var(--transizione);
  }

  .chip:hover:not(:disabled) {
    border-color: var(--bordo-forte);
    color: var(--testo);
  }

  .chip:disabled {
    opacity: 0.5;
  }

  .chip.pattern.acceso {
    background: var(--accento-tenue);
    border-color: var(--accento-bordo);
    color: var(--accento-testo);
    font-weight: var(--peso-forte);
  }

  .chip.contesto:hover:not(:disabled) {
    background: var(--accento-tenue);
    border-color: var(--accento-bordo);
    color: var(--accento-testo);
  }

  .nota {
    font-size: var(--minuto);
    color: var(--testo-3);
    line-height: var(--riga-larga);
  }

  .libero {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-2);
  }

  .libero :global(.campo) {
    flex: 1 1 auto;
  }

  .coda-fine {
    padding: var(--sp-2) var(--sp-1) 0;
    line-height: var(--riga-larga);
  }

  .allarme {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    color: var(--pericolo);
    font-size: var(--piccolo);
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
