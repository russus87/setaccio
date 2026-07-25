<script lang="ts">
  /**
   * Le composizioni documentali: il tracciato di partenza, i PDF che ne sono
   * usciti e i file rimasti attorno.
   *
   * La domanda a cui questa vista risponde è «da quale tracciato vengono
   * questi PDF?»: per questo la card di ogni lotto mostra prima la
   * correlazione e solo dopo i dettagli.
   */
  import { openPath } from "@tauri-apps/plugin-opener";
  import {
    accorciaPath,
    formattaNumero,
    layoutDetect,
    layoutElimina,
    layoutLista,
    lotti as leggiLotti,
    lottoDettaglio,
    tracciatoRecord,
    type Anteprima,
    type FileRecord,
    type Layout,
    type LayoutCandidato,
    type Lotto,
  } from "../api";
  import Badge from "../ui/Badge.svelte";
  import Bottone from "../ui/Bottone.svelte";
  import Card from "../ui/Card.svelte";
  import Icona from "../ui/Icona.svelte";
  import Progresso from "../ui/Progresso.svelte";
  import Vuoto from "../ui/Vuoto.svelte";
  import RigaFile from "./RigaFile.svelte";
  import { messaggioErrore } from "./comuni";

  interface Props {
    aggiornamento?: number;
  }

  let { aggiornamento = 0 }: Props = $props();

  const PAGINA_RECORD = 50;

  let lotti = $state<Lotto[]>([]);
  let layout = $state<Layout[]>([]);
  let caricando = $state(true);
  let errore = $state<string | null>(null);
  let ricarica = $state(0);

  // ---- Dettaglio ---------------------------------------------------------
  let apertoCodice = $state<string | null>(null);
  let dettaglio = $state<Lotto | null>(null);
  let caricandoDettaglio = $state(false);

  // ---- Sfoglio dei record ------------------------------------------------
  let tracciatoScelto = $state<FileRecord | null>(null);
  let layoutScelto = $state<number | null>(null);
  let da = $state(0);
  let record = $state<Anteprima | null>(null);
  let caricandoRecord = $state(false);

  // ---- Auto-detect -------------------------------------------------------
  let candidato = $state<LayoutCandidato | null>(null);
  let candidatoDi = $state<string | null>(null);
  let rilevando = $state(false);

  $effect(() => {
    void aggiornamento;
    void ricarica;
    let vivo = true;
    caricando = true;
    Promise.all([leggiLotti(), layoutLista()])
      .then(([l, y]) => {
        if (!vivo) return;
        lotti = l;
        layout = y;
        errore = null;
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

  // Dettaglio del lotto aperto.
  $effect(() => {
    const codice = apertoCodice;
    if (!codice) {
      dettaglio = null;
      return;
    }
    let vivo = true;
    caricandoDettaglio = true;
    lottoDettaglio(codice)
      .then((l) => {
        if (vivo) dettaglio = l;
      })
      .catch((e) => {
        if (vivo) errore = messaggioErrore(e);
      })
      .finally(() => {
        if (vivo) caricandoDettaglio = false;
      });
    return () => {
      vivo = false;
    };
  });

  // Record del tracciato scelto, con il layout scelto.
  $effect(() => {
    const t = tracciatoScelto;
    const l = layoutScelto;
    const offset = da;
    if (!t) {
      record = null;
      return;
    }
    let vivo = true;
    caricandoRecord = true;
    tracciatoRecord(t.id, l, offset, PAGINA_RECORD)
      .then((a) => {
        if (vivo) record = a;
      })
      .catch((e) => {
        if (vivo) errore = messaggioErrore(e);
      })
      .finally(() => {
        if (vivo) caricandoRecord = false;
      });
    return () => {
      vivo = false;
    };
  });

  function apriLotto(codice: string) {
    if (apertoCodice === codice) {
      apertoCodice = null;
      tracciatoScelto = null;
      candidato = null;
      return;
    }
    apertoCodice = codice;
    tracciatoScelto = null;
    record = null;
    candidato = null;
    candidatoDi = null;
    da = 0;
  }

  function scegliTracciato(f: FileRecord) {
    tracciatoScelto = tracciatoScelto?.id === f.id ? null : f;
    da = 0;
  }

  async function rileva(f: FileRecord) {
    rilevando = true;
    candidatoDi = f.path;
    try {
      candidato = await layoutDetect(f.path);
      errore = null;
    } catch (e) {
      candidato = null;
      errore = messaggioErrore(e);
    } finally {
      rilevando = false;
    }
  }

  async function eliminaLayout(id: number) {
    try {
      await layoutElimina(id);
      if (layoutScelto === id) layoutScelto = null;
      ricarica += 1;
    } catch (e) {
      errore = messaggioErrore(e);
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

  {#if !caricando && lotti.length === 0}
    <Card>
      <Vuoto
        icona="lotti"
        titolo="Nessuna composizione documentale trovata"
        messaggio="Un lotto nasce quando nella stessa cartella convivono un tracciato a record fissi e i PDF che ne derivano. Aggiungi una sorgente di fascia «tracciati» nelle impostazioni e lancia una scansione."
      />
    </Card>
  {:else}
    <div class="conta testo-piccolo testo-secondario">
      <span class="cifre testo-forte">{formattaNumero(lotti.length)}</span>
      {lotti.length === 1 ? "lotto" : "lotti"} · clicca una card per aprirne il dettaglio
    </div>

    {#each lotti as l (l.codice)}
      {@const aperto = apertoCodice === l.codice}
      {@const corpo = aperto && dettaglio?.codice === l.codice ? dettaglio : l}
      <Card padding="nessuna">
        {#snippet intestazione()}
          <button class="testa-lotto" onclick={() => apriLotto(l.codice)}>
            <span class="tile-lotto"><Icona nome="lotti" dimensione={18} /></span>

            <span class="crescente titoli-lotto">
              <span class="codice">{l.codice}</span>
              <span class="cartella mono troncato" title={l.cartella}>
                {accorciaPath(l.cartella, 62)}
              </span>
            </span>

            <span class="correlazione">
              <span class="pezzo tracciato">
                <span class="cifre">{l.tracciati.length}</span>
                {l.tracciati.length === 1 ? "tracciato" : "tracciati"}
              </span>
              <Icona nome="freccia" dimensione={13} />
              <span class="pezzo documento">
                <span class="cifre">{l.pdf.length}</span> PDF
              </span>
              {#if l.altri.length > 0}
                <span class="pezzo altro">
                  <span class="cifre">{l.altri.length}</span> altri
                </span>
              {/if}
            </span>

            <span class="chevron" class:aperto>
              <Icona nome="freccia" dimensione={16} ruota={aperto ? 90 : 0} />
            </span>
          </button>
        {/snippet}

        {#if aperto}
          <div class="dettaglio">
            {#if caricandoDettaglio && !dettaglio}
              <p class="testo-piccolo testo-secondario attesa">Lettura del lotto…</p>
            {:else}
              <!-- Tracciati -->
              <section class="blocco">
                <h3 class="titolo-blocco">Tracciato di partenza</h3>
                {#if corpo.tracciati.length === 0}
                  <p class="nota">
                    Questo lotto non ha un tracciato indicizzato: ci sono solo i
                    documenti prodotti.
                  </p>
                {:else}
                  {#each corpo.tracciati as t (t.id)}
                    <RigaFile
                      file={t}
                      compatta
                      selezionata={tracciatoScelto?.id === t.id}
                      onseleziona={() => scegliTracciato(t)}
                      onapri={() => apri(t.path)}
                    >
                      <div class="badge-riga">
                        <Badge tipo={t.tipo} />
                        {#if t.contesto}<Badge testo={t.contesto} variante="accento" />{/if}
                        <span class="testo-piccolo testo-tenue">
                          {tracciatoScelto?.id === t.id
                            ? "sfoglia i record qui sotto"
                            : "clicca per sfogliare i record"}
                        </span>
                      </div>
                      {#snippet azioni()}
                        <Bottone
                          variante="secondario"
                          dimensione="sm"
                          caricamento={rilevando && candidatoDi === t.path}
                          onclick={(e) => {
                            // La riga sotto è cliccabile: il bottone non deve
                            // anche cambiare il tracciato selezionato.
                            e.stopPropagation();
                            rileva(t);
                          }}
                        >
                          Rileva layout
                        </Bottone>
                      {/snippet}
                    </RigaFile>

                    {#if candidato && candidatoDi === t.path}
                      <div class="candidato">
                        <div class="riga-sparsa">
                          <p class="titolo-candidato">Layout candidato</p>
                          <Bottone
                            variante="fantasma"
                            dimensione="sm"
                            icona="chiudi"
                            soloIcona
                            titolo="Chiudi il candidato"
                            onclick={() => (candidato = null)}
                          />
                        </div>

                        <div class="numeri-candidato">
                          <div class="numero">
                            <span class="cifra cifre">{candidato.lunghezza_record}</span>
                            <span class="etichetta">caratteri per record</span>
                          </div>
                          <div class="numero">
                            <span class="cifra cifre">
                              {formattaNumero(candidato.numero_record)}
                            </span>
                            <span class="etichetta">record nel file</span>
                          </div>
                          <div class="numero">
                            <span class="cifra cifre">
                              {candidato.colonne_stabili.length}
                            </span>
                            <span class="etichetta">colonne stabili</span>
                          </div>
                        </div>

                        <Progresso
                          valore={candidato.confidenza * 100}
                          etichetta="Confidenza del rilevamento"
                          tono={candidato.confidenza > 0.7
                            ? "successo"
                            : candidato.confidenza > 0.4
                              ? "avviso"
                              : "pericolo"}
                        />

                        {#if candidato.colonne_stabili.length > 0}
                          <div class="colonne">
                            {#each candidato.colonne_stabili as [inizio, fine], i (i)}
                              <span class="colonna mono">{inizio}–{fine}</span>
                            {/each}
                          </div>
                          <p class="nota">
                            Sono gli intervalli di caratteri identici in tutti i
                            record: separatori fissi, codici costanti, riempimenti.
                            I campi veri stanno negli spazi fra un intervallo e
                            l'altro.
                          </p>
                        {:else}
                          <p class="nota">
                            Nessuna colonna risulta costante su tutti i record: il
                            file è probabilmente a lunghezza variabile.
                          </p>
                        {/if}
                      </div>
                    {/if}
                  {/each}
                {/if}
              </section>

              <!-- Record del tracciato ------------------------------------ -->
              {#if tracciatoScelto}
                <section class="blocco">
                  <div class="riga-sparsa">
                    <h3 class="titolo-blocco">
                      Record di {tracciatoScelto.nome}
                    </h3>
                    <div class="riga">
                      <label class="scelta-layout">
                        <span class="testo-piccolo testo-secondario">Layout</span>
                        <select
                          value={layoutScelto ?? ""}
                          onchange={(e) => {
                            const v = e.currentTarget.value;
                            layoutScelto = v === "" ? null : Number(v);
                            da = 0;
                          }}
                        >
                          <option value="">automatico</option>
                          {#each layout as y (y.id)}
                            <option value={y.id}>
                              {y.nome} ({y.lunghezza_record})
                            </option>
                          {/each}
                        </select>
                      </label>
                    </div>
                  </div>

                  {#if caricandoRecord && !record}
                    <p class="testo-piccolo testo-secondario attesa">
                      Lettura dei record…
                    </p>
                  {:else if record && record.genere === "record" && record.record}
                    <div class="tabella-record">
                      <table>
                        {#if record.intestazioni}
                          <thead>
                            <tr>
                              <th class="riga-numero">#</th>
                              {#each record.intestazioni as h, i (i)}
                                <th>{h}</th>
                              {/each}
                            </tr>
                          </thead>
                        {/if}
                        <tbody>
                          {#each record.record as riga, i (i)}
                            <tr>
                              <td class="riga-numero cifre">{da + i + 1}</td>
                              {#each riga as cella, j (j)}
                                <td class="mono">{cella}</td>
                              {/each}
                            </tr>
                          {/each}
                        </tbody>
                      </table>
                    </div>

                    <div class="sfoglia">
                      <Bottone
                        variante="secondario"
                        dimensione="sm"
                        disabled={da === 0}
                        onclick={() => (da = Math.max(0, da - PAGINA_RECORD))}
                      >
                        Precedenti
                      </Bottone>
                      <span class="testo-piccolo testo-secondario">
                        {record.messaggio ??
                          `record ${formattaNumero(da + 1)}–${formattaNumero(
                            da + (record.record?.length ?? 0),
                          )}`}
                      </span>
                      <Bottone
                        variante="secondario"
                        dimensione="sm"
                        iconaDestra="freccia"
                        disabled={(record.record?.length ?? 0) < PAGINA_RECORD}
                        onclick={() => (da = da + PAGINA_RECORD)}
                      >
                        Successivi
                      </Bottone>
                    </div>
                  {:else}
                    <p class="nota">
                      {record?.messaggio ??
                        "Da questo tracciato non è stato possibile ricavare dei record."}
                    </p>
                  {/if}
                </section>
              {/if}

              <!-- PDF generati -------------------------------------------- -->
              <section class="blocco">
                <h3 class="titolo-blocco">
                  Documenti generati ({corpo.pdf.length})
                </h3>
                {#if corpo.pdf.length === 0}
                  <p class="nota">
                    Nessun PDF indicizzato per questo lotto: o la composizione non
                    è ancora stata prodotta, o i documenti stanno fuori dalle
                    sorgenti dichiarate.
                  </p>
                {:else}
                  {#each corpo.pdf as p (p.id)}
                    <RigaFile
                      file={p}
                      compatta
                      onapri={() => apri(p.path)}
                      dettaglio={p.pagine ? `${p.pagine} pag.` : undefined}
                    />
                  {/each}
                {/if}
              </section>

              <!-- Altri file ---------------------------------------------- -->
              {#if corpo.altri.length > 0}
                <section class="blocco">
                  <h3 class="titolo-blocco">
                    Altri file nel lotto ({corpo.altri.length})
                  </h3>
                  {#each corpo.altri as a (a.id)}
                    <RigaFile file={a} compatta onapri={() => apri(a.path)} />
                  {/each}
                </section>
              {/if}
            {/if}
          </div>
        {/if}
      </Card>
    {/each}
  {/if}

  <!-- Layout salvati ---------------------------------------------------- -->
  <Card
    titolo="Layout salvati"
    sottotitolo="Profili di record riutilizzabili fra lotti diversi"
    padding="nessuna"
  >
    <div class="elenco-layout">
      {#if layout.length === 0}
        <Vuoto
          compatto
          icona="tracciati"
          titolo="Nessun layout salvato"
          messaggio="Apri un lotto e usa «Rileva layout» su un tracciato: Setaccio misura la lunghezza dei record e ti dice quali colonne restano costanti."
        />
      {:else}
        {#each layout as y (y.id)}
          <div class="riga-layout">
            <span class="tile-layout"><Icona nome="tracciati" dimensione={16} /></span>
            <div class="crescente">
              <p class="nome-layout">{y.nome}</p>
              <p class="testo-piccolo testo-secondario">
                {y.lunghezza_record} caratteri per record · {y.campi.length}
                {y.campi.length === 1 ? "campo" : "campi"}
              </p>
            </div>
            <div class="campi">
              {#each y.campi.slice(0, 6) as c (c.nome)}
                <span class="campo-badge mono" title="offset {c.offset}, lunghezza {c.lunghezza}">
                  {c.nome}
                </span>
              {/each}
              {#if y.campi.length > 6}
                <span class="campo-badge">+{y.campi.length - 6}</span>
              {/if}
            </div>
            <Bottone
              variante="fantasma"
              dimensione="sm"
              icona="cestino"
              soloIcona
              titolo="Elimina il layout {y.nome}"
              onclick={() => eliminaLayout(y.id)}
            />
          </div>
        {/each}
      {/if}
    </div>
  </Card>
</div>

<style>
  .conta {
    padding: 0 var(--sp-1);
  }

  /* Testa del lotto --------------------------------------------------- */
  .testa-lotto {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    width: 100%;
    text-align: left;
    min-width: 0;
  }

  .tile-lotto {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 38px;
    height: 38px;
    flex: 0 0 auto;
    border-radius: var(--raggio);
    background: var(--tracciato-bg);
    color: var(--tracciato);
  }

  .titoli-lotto {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .codice {
    font-size: var(--corpo);
    font-weight: var(--peso-grasso);
    letter-spacing: -0.01em;
  }

  .cartella {
    font-size: var(--micro);
    color: var(--testo-3);
  }

  .correlazione {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    flex: 0 0 auto;
    color: var(--testo-3);
    flex-wrap: wrap;
  }

  .pezzo {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
    padding: 3px var(--sp-2);
    border-radius: var(--raggio-pillola);
    font-size: var(--micro);
    font-weight: var(--peso-medio);
  }

  .pezzo .cifre {
    font-weight: var(--peso-grasso);
  }

  .pezzo.tracciato {
    background: var(--tracciato-bg);
    color: var(--tracciato);
  }

  .pezzo.documento {
    background: var(--documento-bg);
    color: var(--documento);
  }

  .pezzo.altro {
    background: var(--altro-bg);
    color: var(--altro);
  }

  .chevron {
    display: flex;
    flex: 0 0 auto;
    color: var(--testo-3);
  }

  /* Dettaglio ---------------------------------------------------------- */
  .dettaglio {
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
    padding: 0 var(--sp-4) var(--sp-5);
  }

  .blocco {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding: var(--sp-4);
    border-radius: var(--raggio-lg);
    background: var(--superficie-2);
    min-width: 0;
  }

  .titolo-blocco {
    font-size: var(--micro);
    font-weight: var(--peso-forte);
    color: var(--testo-3);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .nota {
    font-size: var(--piccolo);
    color: var(--testo-2);
    line-height: var(--riga-larga);
  }

  .attesa {
    padding: var(--sp-5) 0;
    text-align: center;
  }

  .badge-riga {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
  }

  /* Candidato ---------------------------------------------------------- */
  .candidato {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    margin: var(--sp-2) 0 var(--sp-2) var(--sp-5);
    padding: var(--sp-4);
    border-radius: var(--raggio);
    background: var(--superficie);
    border: 1px solid var(--bordo);
  }

  .titolo-candidato {
    font-size: var(--piccolo);
    font-weight: var(--peso-grasso);
  }

  .numeri-candidato {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-3);
  }

  .numero {
    flex: 1 1 120px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .cifra {
    font-size: var(--grande);
    font-weight: var(--peso-grasso);
    letter-spacing: -0.02em;
  }

  .etichetta {
    font-size: var(--micro);
    color: var(--testo-2);
  }

  .colonne {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-1);
  }

  .colonna {
    padding: 2px var(--sp-2);
    border-radius: var(--raggio-sm);
    background: var(--superficie-2);
    color: var(--testo-2);
    font-size: var(--micro);
  }

  /* Record -------------------------------------------------------------- */
  .scelta-layout {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
  }

  select {
    height: 30px;
    padding: 0 var(--sp-2);
    border-radius: var(--raggio-sm);
    border: 1px solid var(--bordo);
    background: var(--superficie);
    color: var(--testo);
    font-size: var(--minuto);
  }

  .tabella-record {
    overflow-x: auto;
    border-radius: var(--raggio);
    border: 1px solid var(--bordo);
    background: var(--superficie);
    max-height: 420px;
  }

  .tabella-record table {
    width: 100%;
    font-size: var(--micro);
  }

  .tabella-record th {
    position: sticky;
    top: 0;
    z-index: 1;
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

  .riga-numero {
    color: var(--testo-3);
    text-align: right;
    width: 60px;
  }

  .sfoglia {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--sp-3);
    padding-top: var(--sp-2);
  }

  /* Layout salvati ------------------------------------------------------ */
  .elenco-layout {
    display: flex;
    flex-direction: column;
    padding: 0 var(--sp-4) var(--sp-3);
  }

  .riga-layout {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-3) 0;
    border-bottom: 1px solid var(--bordo);
    min-width: 0;
  }

  .riga-layout:last-child {
    border-bottom: none;
  }

  .tile-layout {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    flex: 0 0 auto;
    border-radius: var(--raggio-sm);
    background: var(--tracciato-bg);
    color: var(--tracciato);
  }

  .nome-layout {
    font-size: var(--corpo);
    font-weight: var(--peso-forte);
  }

  .campi {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-1);
    justify-content: flex-end;
    max-width: 40%;
  }

  .campo-badge {
    padding: 2px var(--sp-2);
    border-radius: var(--raggio-sm);
    background: var(--superficie-2);
    color: var(--testo-2);
    font-size: var(--micro);
    white-space: nowrap;
  }

  .allarme {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    color: var(--pericolo);
    font-size: var(--piccolo);
  }
</style>
