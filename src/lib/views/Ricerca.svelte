<script lang="ts">
  /**
   * La vista principale: filtri componibili a sinistra, risultati al centro,
   * anteprima a destra.
   *
   * La barra di ricerca sta nel guscio, non qui: arrivano da fuori sia il
   * testo confermato con Invio (`query`, per il full-text) sia quello che
   * l'utente sta ancora scrivendo (`bozza`, per la ricerca istantanea sul
   * nome). Le due modalità sono esplicite perché fanno cose diverse: una
   * legge il testo estratto, l'altra solo nome e percorso.
   */
  import { openPath } from "@tauri-apps/plugin-opener";
  import {
    accorciaPath,
    anteprima as leggiAnteprima,
    cerca,
    cercaNome,
    faccette,
    filtriVuoti,
    formattaByte,
    formattaData,
    formattaNumero,
    STATI,
    TIPI,
    type Anteprima,
    type ConteggioEtichetta,
    type FileRecord,
    type Filtri,
    type Risultato,
  } from "../api";
  import Badge from "../ui/Badge.svelte";
  import Bottone from "../ui/Bottone.svelte";
  import Campo from "../ui/Campo.svelte";
  import Card from "../ui/Card.svelte";
  import Icona from "../ui/Icona.svelte";
  import Interruttore from "../ui/Interruttore.svelte";
  import Vuoto from "../ui/Vuoto.svelte";
  import RigaFile from "./RigaFile.svelte";
  import {
    dataInEpoch,
    messaggioErrore,
    spezzaEvidenziato,
    tipoAlPlurale,
  } from "./comuni";

  interface Props {
    /** Testo confermato con Invio nella barra del guscio. */
    query: string;
    /** Testo mentre lo si scrive: alimenta la ricerca istantanea sul nome. */
    bozza: string;
  }

  let { query, bozza }: Props = $props();

  /** Un risultato, comune alle due modalità. */
  interface Voce {
    file: FileRecord;
    snippet: string | null;
    riga: number | null;
    pagina: number | null;
  }

  type Modalita = "testo" | "nome";
  let modalita = $state<Modalita>("testo");

  // ---- Filtri ------------------------------------------------------------
  let tipiSel = $state<string[]>([]);
  let contestiSel = $state<string[]>([]);
  let statiSel = $state<string[]>([]);
  let estensioni = $state("");
  let dataDa = $state("");
  let dataA = $state("");
  let sizeMinMb = $state("");
  let sizeMaxMb = $state("");
  let includiArtefatti = $state(false);

  const LIMITE = 200;

  function numeroByte(mb: string): number | null {
    const n = Number(mb.replace(",", "."));
    return mb.trim() === "" || !Number.isFinite(n) || n < 0
      ? null
      : Math.round(n * 1024 * 1024);
  }

  const filtri: Filtri = $derived({
    ...filtriVuoti(),
    tipi: tipiSel,
    contesti: contestiSel,
    stati: statiSel,
    estensioni: estensioni
      .split(",")
      .map((e) => e.trim().replace(/^\./, "").toLowerCase())
      .filter(Boolean),
    da_data: dataInEpoch(dataDa),
    a_data: dataInEpoch(dataA, true),
    size_min: numeroByte(sizeMinMb),
    size_max: numeroByte(sizeMaxMb),
    includi_artefatti: includiArtefatti,
    limite: LIMITE,
  });

  const filtriAttivi = $derived(
    tipiSel.length +
      contestiSel.length +
      statiSel.length +
      (filtri.estensioni.length > 0 ? 1 : 0) +
      (filtri.da_data !== null ? 1 : 0) +
      (filtri.a_data !== null ? 1 : 0) +
      (filtri.size_min !== null ? 1 : 0) +
      (filtri.size_max !== null ? 1 : 0) +
      (includiArtefatti ? 1 : 0),
  );

  function azzeraFiltri() {
    tipiSel = [];
    contestiSel = [];
    statiSel = [];
    estensioni = "";
    dataDa = "";
    dataA = "";
    sizeMinMb = "";
    sizeMaxMb = "";
    includiArtefatti = false;
  }

  function commuta(elenco: string[], valore: string): string[] {
    return elenco.includes(valore)
      ? elenco.filter((v) => v !== valore)
      : [...elenco, valore];
  }

  // ---- Faccette dei contesti --------------------------------------------
  let contesti = $state<ConteggioEtichetta[]>([]);

  $effect(() => {
    let vivo = true;
    faccette()
      .then((f) => {
        if (vivo) contesti = f;
      })
      .catch(() => {
        // Le faccette sono un aiuto, non un requisito: se il backend non
        // risponde la ricerca resta comunque utilizzabile.
      });
    return () => {
      vivo = false;
    };
  });

  // ---- Termine di ricerca -----------------------------------------------
  // Sul nome si cerca mentre si scrive, ma non a ogni battuta: un quarto di
  // secondo di quiete evita una raffica di query inutili.
  let bozzaRitardata = $state("");

  $effect(() => {
    const v = bozza;
    const t = setTimeout(() => (bozzaRitardata = v), 220);
    return () => clearTimeout(t);
  });

  const termine = $derived(modalita === "nome" ? bozzaRitardata : query);

  // ---- Esecuzione della ricerca -----------------------------------------
  let voci = $state<Voce[]>([]);
  let cercando = $state(false);
  let errore = $state<string | null>(null);
  let eseguita = $state(false);

  $effect(() => {
    const t = termine.trim();
    const f = filtri;
    const m = modalita;

    if (t === "") {
      voci = [];
      eseguita = false;
      errore = null;
      cercando = false;
      return;
    }

    let vivo = true;
    cercando = true;

    const promessa =
      m === "testo"
        ? cerca(t, f).then((r: Risultato[]) =>
            r.map((x) => ({
              file: x,
              snippet: x.snippet,
              riga: x.riga,
              pagina: x.pagina,
            })),
          )
        : cercaNome(t, f).then((r: FileRecord[]) =>
            r.map((x) => ({ file: x, snippet: null, riga: null, pagina: null })),
          );

    promessa
      .then((v) => {
        if (!vivo) return;
        voci = v;
        errore = null;
        eseguita = true;
      })
      .catch((e) => {
        if (!vivo) return;
        voci = [];
        errore = messaggioErrore(e);
        eseguita = true;
      })
      .finally(() => {
        if (vivo) cercando = false;
      });

    return () => {
      vivo = false;
    };
  });

  // ---- Selezione e anteprima --------------------------------------------
  let selezionato = $state<Voce | null>(null);
  let paginaChiesta = $state<number | null>(null);
  let ant = $state<Anteprima | null>(null);
  let anteprimaErrore = $state<string | null>(null);
  let caricandoAnteprima = $state(false);

  // Se la lista cambia sotto i piedi, la selezione che non c'è più va via.
  $effect(() => {
    if (selezionato && !voci.some((v) => v.file.id === selezionato?.file.id)) {
      selezionato = null;
      ant = null;
    }
  });

  function seleziona(v: Voce) {
    selezionato = v;
    paginaChiesta = v.pagina;
  }

  $effect(() => {
    const v = selezionato;
    const p = paginaChiesta;
    if (!v) {
      ant = null;
      anteprimaErrore = null;
      return;
    }

    let vivo = true;
    caricandoAnteprima = true;
    leggiAnteprima(v.file.id, p)
      .then((a) => {
        if (!vivo) return;
        ant = a;
        anteprimaErrore = null;
      })
      .catch((e) => {
        if (!vivo) return;
        ant = null;
        anteprimaErrore = messaggioErrore(e);
      })
      .finally(() => {
        if (vivo) caricandoAnteprima = false;
      });

    return () => {
      vivo = false;
    };
  });

  async function apri(path: string) {
    try {
      await openPath(path);
    } catch (e) {
      errore = messaggioErrore(e);
    }
  }
</script>

<div class="pagina">
  <!-- Filtri ---------------------------------------------------------- -->
  <aside class="filtri">
    <Card padding="stretta">
      {#snippet intestazione()}
        <div class="riga-sparsa testa-filtri">
          <span class="titolo-filtri">
            <Icona nome="filtro" dimensione={16} />
            Filtri
            {#if filtriAttivi > 0}<Badge testo={String(filtriAttivi)} variante="accento" />{/if}
          </span>
          {#if filtriAttivi > 0}
            <Bottone variante="fantasma" dimensione="sm" onclick={azzeraFiltri}>
              Azzera
            </Bottone>
          {/if}
        </div>
      {/snippet}

      <div class="impila">
        <fieldset class="gruppo">
          <legend>Tipo</legend>
          <div class="chip-riga">
            {#each TIPI as t (t)}
              <button
                class="chip"
                class:acceso={tipiSel.includes(t)}
                aria-pressed={tipiSel.includes(t)}
                onclick={() => (tipiSel = commuta(tipiSel, t))}
              >
                {tipoAlPlurale(t)}
              </button>
            {/each}
          </div>
        </fieldset>

        <fieldset class="gruppo">
          <legend>Contesto</legend>
          {#if contesti.length === 0}
            <p class="nota-gruppo">
              Nessun contesto ancora assegnato: si creano dalla Revisione.
            </p>
          {:else}
            <div class="chip-colonna">
              {#each contesti as c (c.etichetta)}
                <button
                  class="chip largo"
                  class:acceso={contestiSel.includes(c.etichetta)}
                  aria-pressed={contestiSel.includes(c.etichetta)}
                  onclick={() => (contestiSel = commuta(contestiSel, c.etichetta))}
                >
                  <span class="troncato">{c.etichetta}</span>
                  <span class="conta cifre">{formattaNumero(c.quanti)}</span>
                </button>
              {/each}
            </div>
          {/if}
        </fieldset>

        <fieldset class="gruppo">
          <legend>Stato</legend>
          <div class="chip-riga">
            {#each STATI as s (s)}
              <button
                class="chip"
                class:acceso={statiSel.includes(s)}
                aria-pressed={statiSel.includes(s)}
                onclick={() => (statiSel = commuta(statiSel, s))}
              >
                {s}
              </button>
            {/each}
          </div>
        </fieldset>

        <fieldset class="gruppo">
          <legend>Estensione</legend>
          <Campo
            bind:valore={estensioni}
            segnaposto="pdf, docx, txt"
            descrizione="Separale con la virgola."
            azzerabile
            autocomplete="off"
            spellcheck={false}
          />
        </fieldset>

        <fieldset class="gruppo">
          <legend>Modificato fra</legend>
          <div class="coppia">
            <Campo type="date" bind:valore={dataDa} etichetta="Dal" />
            <Campo type="date" bind:valore={dataA} etichetta="Al" />
          </div>
        </fieldset>

        <fieldset class="gruppo">
          <legend>Dimensione in MB</legend>
          <div class="coppia">
            <Campo type="number" min="0" bind:valore={sizeMinMb} etichetta="Da" />
            <Campo type="number" min="0" bind:valore={sizeMaxMb} etichetta="A" />
          </div>
        </fieldset>

        <div class="gruppo">
          <Interruttore
            sparso
            dimensione="sm"
            bind:attivo={includiArtefatti}
            etichetta="Includi artefatti"
            descrizione="Output di build e fixture: fuori dalle ricerche per scelta."
          />
        </div>
      </div>
    </Card>
  </aside>

  <!-- Risultati -------------------------------------------------------- -->
  <section class="risultati">
    <Card padding="nessuna">
      {#snippet intestazione()}
        <div class="testa-risultati">
          <div class="modi" role="group" aria-label="Modalità di ricerca">
            <button
              class="modo"
              class:acceso={modalita === "testo"}
              aria-pressed={modalita === "testo"}
              onclick={() => (modalita = "testo")}
            >
              Nel contenuto
            </button>
            <button
              class="modo"
              class:acceso={modalita === "nome"}
              aria-pressed={modalita === "nome"}
              onclick={() => (modalita = "nome")}
            >
              Nel nome
            </button>
          </div>

          <p class="conteggio-risultati testo-piccolo testo-secondario">
            {#if cercando}
              ricerca in corso…
            {:else if eseguita}
              {formattaNumero(voci.length)}
              {voci.length === 1 ? "risultato" : "risultati"}
              {#if voci.length >= LIMITE}(primi {LIMITE}){/if}
            {:else}
              {modalita === "testo"
                ? "scrivi e premi Invio nella barra qui sopra"
                : "scrivi nella barra qui sopra"}
            {/if}
          </p>
        </div>
      {/snippet}

      <div class="elenco">
        {#if errore}
          <div class="allarme" role="alert">
            <Icona nome="avviso" dimensione={16} />
            <span>{errore}</span>
          </div>
        {/if}

        {#if !eseguita && !cercando}
          <Vuoto
            icona="ricerca"
            titolo={modalita === "testo"
              ? "Cerca dentro i documenti"
              : "Cerca per nome di file"}
            messaggio={modalita === "testo"
              ? "Scrivi una o più parole nella barra in alto e premi Invio: Setaccio legge il testo estratto da PDF, documenti e tracciati e ti mostra il punto in cui compaiono."
              : "Scrivi nella barra in alto: i nomi e i percorsi si filtrano mentre digiti, senza premere Invio."}
          />
        {:else if voci.length === 0 && !cercando}
          <Vuoto
            icona="filtro"
            titolo="Qui non arriva niente con questi filtri"
            messaggio={filtriAttivi > 0
              ? "Prova ad allentare i filtri a sinistra, oppure accendi «Includi artefatti» se quello che cerchi è un file di build."
              : "Prova con meno parole, oppure passa alla ricerca nel nome se ricordi come si chiama il file ma non cosa contiene."}
            testoAzione={filtriAttivi > 0 ? "Azzera i filtri" : undefined}
            onazione={azzeraFiltri}
          />
        {:else}
          {#each voci as v (v.file.id)}
            <RigaFile
              file={v.file}
              selezionata={selezionato?.file.id === v.file.id}
              onseleziona={() => seleziona(v)}
              onapri={() => apri(v.file.path)}
              valore={formattaByte(v.file.size)}
              dettaglio={formattaData(v.file.mtime)}
            >
              <div class="sotto-riga">
                <div class="badge-riga">
                  <Badge tipo={v.file.tipo} />
                  <Badge stato={v.file.stato} />
                  {#if v.file.contesto}
                    <Badge testo={v.file.contesto} variante="accento" />
                  {/if}
                  {#if v.pagina !== null}
                    <span class="posizione">pag. {v.pagina}</span>
                  {:else if v.riga !== null}
                    <span class="posizione">riga {v.riga}</span>
                  {/if}
                </div>

                {#if v.snippet}
                  <p class="snippet">
                    {#each spezzaEvidenziato(v.snippet) as p, i (i)}
                      {#if p.forte}<mark>{p.testo}</mark>{:else}{p.testo}{/if}
                    {/each}
                  </p>
                {/if}
              </div>
            </RigaFile>
          {/each}
        {/if}
      </div>
    </Card>
  </section>

  <!-- Anteprima --------------------------------------------------------- -->
  <aside class="anteprima">
    <Card padding="nessuna" piena>
      {#snippet intestazione()}
        <div class="testa-anteprima">
          <div class="crescente">
            <p class="titolo-anteprima troncato">
              {selezionato ? selezionato.file.nome : "Anteprima"}
            </p>
            {#if selezionato}
              <p class="path-anteprima troncato" title={selezionato.file.path}>
                {accorciaPath(selezionato.file.path, 46)}
              </p>
            {/if}
          </div>
          {#if selezionato}
            <Bottone
              variante="fantasma"
              dimensione="sm"
              icona="esterno"
              soloIcona
              titolo="Apri col programma di sistema"
              onclick={() => selezionato && apri(selezionato.file.path)}
            />
          {/if}
        </div>
      {/snippet}

      <div class="corpo-anteprima">
        {#if !selezionato}
          <Vuoto
            compatto
            icona="occhio"
            titolo="Scegli un risultato"
            messaggio="Il contenuto compare qui. Doppio clic su una riga per aprire il file col programma di sistema."
          />
        {:else if anteprimaErrore}
          <div class="allarme" role="alert">
            <Icona nome="avviso" dimensione={16} />
            <span>{anteprimaErrore}</span>
          </div>
        {:else if caricandoAnteprima && !ant}
          <p class="testo-secondario testo-piccolo attesa">Lettura del contenuto…</p>
        {:else if ant}
          {#if ant.genere === "record" && ant.record}
            <div class="tabella-record">
              <table>
                {#if ant.intestazioni}
                  <thead>
                    <tr>
                      {#each ant.intestazioni as h, i (i)}
                        <th>{h}</th>
                      {/each}
                    </tr>
                  </thead>
                {/if}
                <tbody>
                  {#each ant.record as riga, i (i)}
                    <tr>
                      {#each riga as cella, j (j)}
                        <td class="mono">{cella}</td>
                      {/each}
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
            {#if ant.messaggio}
              <p class="nota-anteprima">{ant.messaggio}</p>
            {/if}
          {:else if ant.genere === "testo" && ant.testo}
            {#if ant.pagine && ant.pagine > 1}
              <div class="pagine">
                <Bottone
                  variante="secondario"
                  dimensione="sm"
                  disabled={(ant.pagina ?? 1) <= 1}
                  onclick={() => (paginaChiesta = Math.max(1, (ant?.pagina ?? 1) - 1))}
                >
                  Precedente
                </Bottone>
                <span class="testo-piccolo cifre">
                  pag. {ant.pagina ?? 1} di {ant.pagine}
                </span>
                <Bottone
                  variante="secondario"
                  dimensione="sm"
                  iconaDestra="freccia"
                  disabled={(ant.pagina ?? 1) >= ant.pagine}
                  onclick={() =>
                    (paginaChiesta = Math.min(
                      ant?.pagine ?? 1,
                      (ant?.pagina ?? 1) + 1,
                    ))}
                >
                  Successiva
                </Bottone>
              </div>
            {/if}
            <pre class="testo-anteprima">{ant.testo}</pre>
          {:else}
            <Vuoto
              compatto
              icona="documento"
              titolo="Nessun contenuto da mostrare"
              messaggio={ant.messaggio ??
                "Da questo file non è stato estratto testo: puoi comunque aprirlo col programma di sistema."}
            />
          {/if}
        {/if}
      </div>
    </Card>
  </aside>
</div>

<style>
  .pagina {
    display: grid;
    grid-template-columns: 260px minmax(0, 1fr) 340px;
    gap: var(--sp-4);
    align-items: start;
  }

  @media (max-width: 1400px) {
    .pagina {
      grid-template-columns: 240px minmax(0, 1fr);
    }
    .anteprima {
      grid-column: 1 / -1;
    }
  }

  @media (max-width: 900px) {
    .pagina {
      grid-template-columns: minmax(0, 1fr);
    }
  }

  .filtri,
  .anteprima {
    position: sticky;
    top: 0;
    min-width: 0;
  }

  .risultati {
    min-width: 0;
  }

  /* Filtri ------------------------------------------------------------ */
  .testa-filtri {
    width: 100%;
  }

  .titolo-filtri {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    font-size: var(--corpo);
    font-weight: var(--peso-grasso);
  }

  .gruppo {
    border: 0;
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
  }

  .gruppo legend {
    font-size: var(--micro);
    font-weight: var(--peso-forte);
    color: var(--testo-3);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding-bottom: var(--sp-2);
  }

  .nota-gruppo {
    font-size: var(--minuto);
    color: var(--testo-3);
    line-height: var(--riga-larga);
  }

  .chip-riga {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-1);
  }

  .chip-colonna {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 200px;
    overflow-y: auto;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    height: 26px;
    padding: 0 var(--sp-3);
    border-radius: var(--raggio-pillola);
    border: 1px solid var(--bordo);
    background: var(--superficie-2);
    color: var(--testo-2);
    font-size: var(--micro);
    font-weight: var(--peso-medio);
    white-space: nowrap;
    transition:
      background var(--transizione),
      color var(--transizione),
      border-color var(--transizione);
  }

  .chip:hover {
    border-color: var(--bordo-forte);
    color: var(--testo);
  }

  .chip.acceso {
    background: var(--accento-tenue);
    border-color: var(--accento-bordo);
    color: var(--accento-testo);
    font-weight: var(--peso-forte);
  }

  .chip.largo {
    justify-content: space-between;
    width: 100%;
    height: 28px;
  }

  .conta {
    flex: 0 0 auto;
    color: var(--testo-3);
  }

  .chip.acceso .conta {
    color: inherit;
  }

  .coppia {
    display: flex;
    gap: var(--sp-2);
  }

  .coppia :global(.campo) {
    flex: 1 1 0;
  }

  /* Risultati --------------------------------------------------------- */
  .testa-risultati {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-3);
    width: 100%;
    flex-wrap: wrap;
  }

  .modi {
    display: inline-flex;
    gap: 2px;
    padding: 3px;
    border-radius: var(--raggio-pillola);
    background: var(--superficie-2);
    border: 1px solid var(--bordo);
  }

  .modo {
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

  .modo:hover {
    color: var(--testo);
  }

  .modo.acceso {
    background: var(--superficie);
    color: var(--testo);
    box-shadow: var(--ombra-1);
  }

  .conteggio-risultati {
    flex: 0 0 auto;
  }

  .elenco {
    display: flex;
    flex-direction: column;
    padding: var(--sp-2) var(--sp-2) var(--sp-3);
    max-height: calc(100vh - 300px);
    overflow-y: auto;
  }

  .sotto-riga {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
  }

  .badge-riga {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-1);
  }

  .posizione {
    font-size: var(--micro);
    font-weight: var(--peso-forte);
    color: var(--testo-3);
    font-variant-numeric: tabular-nums;
  }

  .snippet {
    font-size: var(--piccolo);
    line-height: var(--riga-larga);
    color: var(--testo-2);
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .snippet mark {
    background: var(--accento-tenue);
    color: var(--testo);
    font-weight: var(--peso-forte);
    border-radius: 3px;
    padding: 0 2px;
  }

  /* Anteprima --------------------------------------------------------- */
  .testa-anteprima {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-2);
    width: 100%;
    min-width: 0;
  }

  .titolo-anteprima {
    font-size: var(--corpo);
    font-weight: var(--peso-grasso);
  }

  .path-anteprima {
    font-size: var(--micro);
    color: var(--testo-3);
    font-family: var(--famiglia-mono);
  }

  .corpo-anteprima {
    padding: 0 var(--sp-4) var(--sp-4);
    max-height: calc(100vh - 300px);
    overflow: auto;
  }

  .attesa {
    padding: var(--sp-6) 0;
    text-align: center;
  }

  .testo-anteprima {
    margin-top: var(--sp-2);
    padding: var(--sp-3);
    border-radius: var(--raggio);
    background: var(--superficie-2);
    font-family: var(--famiglia-mono);
    font-size: var(--micro);
    line-height: var(--riga-larga);
    color: var(--testo-2);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .pagine {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--sp-3);
    padding-bottom: var(--sp-2);
  }

  .tabella-record {
    overflow-x: auto;
    border-radius: var(--raggio);
    border: 1px solid var(--bordo);
  }

  .tabella-record table {
    width: 100%;
    font-size: var(--micro);
  }

  .tabella-record th {
    position: sticky;
    top: 0;
    padding: var(--sp-2);
    background: var(--superficie-2);
    color: var(--testo-3);
    font-weight: var(--peso-forte);
    text-align: left;
    white-space: nowrap;
    border-bottom: 1px solid var(--bordo);
  }

  .tabella-record td {
    padding: var(--sp-1) var(--sp-2);
    border-bottom: 1px solid var(--bordo);
    white-space: nowrap;
    color: var(--testo-2);
  }

  .nota-anteprima {
    margin-top: var(--sp-2);
    font-size: var(--micro);
    color: var(--testo-3);
  }

  .allarme {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    margin: var(--sp-2) 0;
    padding: var(--sp-3);
    border-radius: var(--raggio);
    background: var(--pericolo-bg);
    color: var(--pericolo);
    font-size: var(--piccolo);
  }
</style>
