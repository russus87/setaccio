<script lang="ts">
  /**
   * Guscio dell'applicazione: sidebar fissa a sinistra, barra superiore e
   * area di contenuto scorrevole a destra.
   *
   * Il routing è uno stato locale con le rune: nessun router esterno, nessun
   * URL. Ogni sezione è una vista che un altro agent sostituirà al posto del
   * `Segnaposto`; questo file resta il solo punto in cui si decide *quale*
   * vista è visibile.
   */
  import {
    ascoltaErroreScansione,
    ascoltaProgresso,
    formattaNumero,
    scanAvvia,
    scanFerma,
    scanProgresso,
    type Progresso,
  } from "./lib/api";
  import BarraProgresso from "./lib/ui/Progresso.svelte";
  import Bottone from "./lib/ui/Bottone.svelte";
  import Icona from "./lib/ui/Icona.svelte";
  import Sidebar, { voceDi, type Sezione } from "./lib/ui/Sidebar.svelte";
  import TopBar from "./lib/ui/TopBar.svelte";
  import Dashboard from "./lib/views/Dashboard.svelte";
  import Duplicati from "./lib/views/Duplicati.svelte";
  import Impostazioni from "./lib/views/Impostazioni.svelte";
  import Lotti from "./lib/views/Lotti.svelte";
  import Novita from "./lib/views/Novita.svelte";
  import Revisione from "./lib/views/Revisione.svelte";
  import Ricerca from "./lib/views/Ricerca.svelte";

  // ---- Routing -----------------------------------------------------------
  let sezione = $state<Sezione>("dashboard");
  const voce = $derived(voceDi(sezione));

  // Il testo della ricerca vive qui perché la barra è nel guscio: le viste lo
  // ricevono già pronto invece di tenerne una copia ciascuna.
  let ricerca = $state("");

  /** Ultima query confermata con Invio: è quella su cui la vista interroga. */
  let queryConfermata = $state("");

  function naviga(s: Sezione) {
    if (s === sezione) return;
    sezione = s;
    // La ricerca è di pertinenza della sua sezione: cambiando pagina si
    // riparte puliti, così non si trascinano filtri invisibili.
    if (s !== "ricerca") {
      ricerca = "";
      queryConfermata = "";
    }
  }

  // ---- Scansione ---------------------------------------------------------
  // La scansione è di tutta l'applicazione, non di una singola vista: il
  // comando e l'avanzamento stanno perciò nel guscio.
  let progresso = $state<Progresso | null>(null);
  let erroreScansione = $state<string | null>(null);
  let inAttesa = $state(false);

  const inCorso = $derived(progresso?.in_corso === true);

  // Le viste leggono dal database: quando una scansione finisce, i loro dati
  // sono vecchi. Questo contatore cambia una volta sola per scansione ed è
  // l'unico segnale che serve loro per rileggere.
  let fineScansione = $state(0);
  let eraInCorso = false;

  $effect(() => {
    const ora = inCorso;
    if (eraInCorso && !ora) fineScansione += 1;
    eraInCorso = ora;
  });

  $effect(() => {
    let vivo = true;
    const disiscrizioni: (() => void)[] = [];

    (async () => {
      try {
        const p = await scanProgresso();
        if (vivo) progresso = p;
        const a = await ascoltaProgresso((x) => {
          if (vivo) progresso = x;
        });
        const b = await ascoltaErroreScansione((m) => {
          if (vivo) erroreScansione = m;
        });
        disiscrizioni.push(a, b);
      } catch {
        // Fuori da Tauri (`npm run dev` in un browser) non c'è backend: il
        // guscio resta usabile, semplicemente senza scansione.
      }
    })();

    return () => {
      vivo = false;
      for (const f of disiscrizioni) f();
    };
  });

  async function commutaScansione() {
    erroreScansione = null;
    inAttesa = true;
    try {
      if (inCorso) {
        await scanFerma();
      } else {
        await scanAvvia();
      }
      progresso = await scanProgresso();
    } catch (e) {
      erroreScansione = e instanceof Error ? e.message : String(e);
    } finally {
      inAttesa = false;
    }
  }

  // Ctrl/Cmd+K porta alla ricerca da qualunque sezione.
  function suTasto(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
      e.preventDefault();
      naviga("ricerca");
      document.querySelector<HTMLInputElement>(".area-ricerca input")?.focus();
    }
  }
</script>

<svelte:window onkeydown={suTasto} />

<div class="guscio">
  <Sidebar {sezione} onnaviga={naviga} />

  <div class="colonna">
    <div class="area-ricerca">
      <TopBar
        titolo={voce.etichetta}
        sottotitolo={voce.descrizione}
        bind:ricerca
        mostraRicerca={sezione !== "impostazioni"}
        onricerca={(q) => {
          queryConfermata = q;
          if (q && sezione !== "ricerca") naviga("ricerca");
        }}
      >
        {#snippet azioni()}
          <Bottone
            variante={inCorso ? "secondario" : "primario"}
            icona={inCorso ? "ferma" : "avvia"}
            caricamento={inAttesa}
            onclick={commutaScansione}
          >
            {inCorso ? "Ferma" : "Scansiona"}
          </Bottone>
        {/snippet}
      </TopBar>

      {#if inCorso && progresso}
        <div class="striscia">
          <BarraProgresso
            dimensione="sm"
            indeterminato
            etichetta={progresso.fase || "Scansione in corso"}
            dettaglio="{formattaNumero(progresso.indicizzati)} indicizzati · {formattaNumero(
              progresso.visti,
            )} visti"
          />
          {#if progresso.path_corrente}
            <p class="corrente troncato">{progresso.path_corrente}</p>
          {/if}
        </div>
      {/if}

      {#if erroreScansione}
        <div class="striscia errore" role="alert">
          <Icona nome="avviso" dimensione={16} />
          <span class="crescente">{erroreScansione}</span>
          <Bottone
            variante="fantasma"
            dimensione="sm"
            icona="chiudi"
            soloIcona
            titolo="Chiudi l'avviso"
            onclick={() => (erroreScansione = null)}
          />
        </div>
      {/if}
    </div>

    <main class="contenuto">
      <div class="dentro">
        {#if sezione === "dashboard"}
          <Dashboard onnaviga={naviga} aggiornamento={fineScansione} />
        {:else if sezione === "ricerca"}
          <Ricerca query={queryConfermata} bozza={ricerca} />
        {:else if sezione === "novita"}
          <Novita aggiornamento={fineScansione} />
        {:else if sezione === "duplicati"}
          <Duplicati aggiornamento={fineScansione} />
        {:else if sezione === "lotti"}
          <Lotti aggiornamento={fineScansione} />
        {:else if sezione === "revisione"}
          <Revisione aggiornamento={fineScansione} />
        {:else}
          <Impostazioni />
        {/if}
      </div>
    </main>
  </div>
</div>

<style>
  .guscio {
    display: flex;
    height: 100%;
    background: var(--sfondo);
  }

  .colonna {
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    min-width: 0;
    min-height: 0;
  }

  .area-ricerca {
    flex: 0 0 auto;
  }

  .striscia {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-3) var(--sp-7);
    background: var(--sfondo);
    border-bottom: 1px solid var(--bordo);
    font-size: var(--minuto);
  }

  .striscia :global(.progresso) {
    flex: 1 1 auto;
  }

  .corrente {
    flex: 1 1 40%;
    color: var(--testo-3);
    font-family: var(--famiglia-mono);
    font-size: var(--micro);
  }

  .striscia.errore {
    color: var(--pericolo);
    background: var(--pericolo-bg);
    border-bottom-color: transparent;
  }

  /* Il contenuto sta su un fondo leggermente staccato: è ciò che fa
     «galleggiare» le card molto arrotondate, come nei mockup. */
  .contenuto {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
    background: var(--sfondo-2);
  }

  .dentro {
    max-width: var(--larghezza-contenuto);
    margin: 0 auto;
    padding: var(--sp-6) var(--sp-7) var(--sp-8);
  }

  @media (max-width: 900px) {
    .dentro {
      padding: var(--sp-4) var(--sp-4) var(--sp-7);
    }
  }
</style>
