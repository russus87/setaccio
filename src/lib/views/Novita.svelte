<script lang="ts">
  /**
   * Cos'è finito sul disco negli ultimi giorni, raggruppato per giornata.
   *
   * È la vista da cui è nato il progetto: la cartella dei download si riempie
   * e non si sa più cosa ci sia dentro. Qui la domanda «cosa ho scaricato
   * ieri?» ha una risposta in due secondi.
   */
  import { openPath } from "@tauri-apps/plugin-opener";
  import {
    formattaByte,
    formattaNumero,
    novita,
    type FileRecord,
  } from "../api";
  import Badge from "../ui/Badge.svelte";
  import Card from "../ui/Card.svelte";
  import Icona from "../ui/Icona.svelte";
  import Vuoto from "../ui/Vuoto.svelte";
  import RigaFile from "./RigaFile.svelte";
  import { chiaveGiorno, etichettaGiorno, messaggioErrore } from "./comuni";

  /** Cambia a fine scansione: fa rileggere l'elenco. */
  interface Props {
    aggiornamento?: number;
  }

  let { aggiornamento = 0 }: Props = $props();

  const PERIODI = [
    { giorni: 1, etichetta: "Ultime 24 ore" },
    { giorni: 7, etichetta: "7 giorni" },
    { giorni: 30, etichetta: "30 giorni" },
  ] as const;

  let giorni = $state(7);
  let file = $state<FileRecord[]>([]);
  let caricando = $state(true);
  let errore = $state<string | null>(null);

  $effect(() => {
    const g = giorni;
    void aggiornamento;
    let vivo = true;
    caricando = true;
    novita(g)
      .then((f) => {
        if (!vivo) return;
        file = f;
        errore = null;
      })
      .catch((e) => {
        if (!vivo) return;
        file = [];
        errore = messaggioErrore(e);
      })
      .finally(() => {
        if (vivo) caricando = false;
      });
    return () => {
      vivo = false;
    };
  });

  interface Giornata {
    chiave: string;
    etichetta: string;
    file: FileRecord[];
    byte: number;
  }

  /** Il backend restituisce già in ordine di data: basta accumulare. */
  const giornate = $derived.by<Giornata[]>(() => {
    const out: Giornata[] = [];
    for (const f of [...file].sort((a, b) => b.mtime - a.mtime)) {
      const k = chiaveGiorno(f.mtime);
      const ultima = out[out.length - 1];
      if (ultima && ultima.chiave === k) {
        ultima.file.push(f);
        ultima.byte += f.size;
      } else {
        out.push({
          chiave: k,
          etichetta: etichettaGiorno(f.mtime),
          file: [f],
          byte: f.size,
        });
      }
    }
    return out;
  });

  const byteTotali = $derived(file.reduce((s, f) => s + f.size, 0));

  async function apri(path: string) {
    try {
      await openPath(path);
    } catch (e) {
      errore = messaggioErrore(e);
    }
  }
</script>

<div class="impila">
  <Card padding="stretta">
    {#snippet intestazione()}
      <div class="testa">
        <div class="periodi" role="group" aria-label="Periodo">
          {#each PERIODI as p (p.giorni)}
            <button
              class="periodo"
              class:acceso={giorni === p.giorni}
              aria-pressed={giorni === p.giorni}
              onclick={() => (giorni = p.giorni)}
            >
              {p.etichetta}
            </button>
          {/each}
        </div>

        <p class="riepilogo testo-piccolo testo-secondario">
          {#if caricando}
            lettura…
          {:else}
            <span class="cifre testo-forte">{formattaNumero(file.length)}</span>
            file ·
            <span class="cifre testo-forte">{formattaByte(byteTotali)}</span>
            in {giornate.length}
            {giornate.length === 1 ? "giornata" : "giornate"} · doppio clic per aprire
          {/if}
        </p>
      </div>
    {/snippet}
  </Card>

  {#if errore}
    <Card>
      <div class="allarme" role="alert">
        <Icona nome="avviso" dimensione={16} />
        <span>{errore}</span>
      </div>
    </Card>
  {:else if !caricando && file.length === 0}
    <Card>
      <Vuoto
        icona="novita"
        titolo="Niente di nuovo in questo periodo"
        messaggio="Allarga la finestra temporale qui sopra, oppure lancia una scansione con il pulsante Scansiona: le novità si calcolano sulla data di modifica dei file indicizzati."
      />
    </Card>
  {:else}
    {#each giornate as g (g.chiave)}
      <Card padding="nessuna">
        {#snippet intestazione()}
          <div class="testa-giorno">
            <div class="crescente">
              <h2 class="giorno">{g.etichetta}</h2>
              <p class="testo-piccolo testo-tenue">
                {formattaNumero(g.file.length)} file
              </p>
            </div>
            <span class="peso-giorno cifre">{formattaByte(g.byte)}</span>
          </div>
        {/snippet}

        <div class="elenco">
          {#each g.file as f (f.id)}
            <RigaFile file={f} compatta onapri={() => apri(f.path)}>
              <div class="badge-riga">
                <Badge tipo={f.tipo} />
                {#if f.contesto}
                  <Badge testo={f.contesto} variante="accento" />
                {:else}
                  <Badge testo="da collocare" variante="avviso" />
                {/if}
                {#if f.ext}<span class="ext">.{f.ext}</span>{/if}
              </div>
            </RigaFile>
          {/each}
        </div>
      </Card>
    {/each}
  {/if}
</div>

<style>
  .testa,
  .testa-giorno {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-3);
    width: 100%;
    flex-wrap: wrap;
  }

  .periodi {
    display: inline-flex;
    gap: 2px;
    padding: 3px;
    border-radius: var(--raggio-pillola);
    background: var(--superficie-2);
    border: 1px solid var(--bordo);
  }

  .periodo {
    height: 28px;
    padding: 0 var(--sp-4);
    border-radius: var(--raggio-pillola);
    font-size: var(--minuto);
    font-weight: var(--peso-forte);
    color: var(--testo-2);
    transition:
      background var(--transizione),
      color var(--transizione);
  }

  .periodo:hover {
    color: var(--testo);
  }

  .periodo.acceso {
    background: var(--superficie);
    color: var(--testo);
    box-shadow: var(--ombra-1);
  }

  .riepilogo {
    flex: 0 0 auto;
  }

  .peso-giorno {
    flex: 0 0 auto;
    font-size: var(--piccolo);
    font-weight: var(--peso-forte);
    color: var(--testo-2);
    white-space: nowrap;
  }

  .giorno {
    font-size: var(--medio);
    font-weight: var(--peso-grasso);
    letter-spacing: -0.01em;
    text-transform: capitalize;
  }

  .elenco {
    display: flex;
    flex-direction: column;
    padding: 0 var(--sp-2) var(--sp-2);
  }

  .badge-riga {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-1);
  }

  .ext {
    font-size: var(--micro);
    color: var(--testo-3);
    font-family: var(--famiglia-mono);
  }

  .allarme {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    color: var(--pericolo);
    font-size: var(--piccolo);
  }
</style>
