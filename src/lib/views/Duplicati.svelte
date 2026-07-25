<script lang="ts">
  /**
   * Gruppi di file con contenuto identico, e i modi per liberarne lo spazio.
   *
   * Il patto con l'utente è che niente sparisce *di soppiatto*: si seleziona,
   * si guarda il piano, si conferma. Dove finiscono i file lo decide lui —
   * quarantena, cestino di sistema o cancellazione — e le tre strade sono
   * tenute a distanze diverse apposta, perché non si disfano allo stesso modo.
   */
  import { openPath } from "@tauri-apps/plugin-opener";
  import {
    duplicati,
    formattaByte,
    formattaDataOra,
    formattaNumero,
    operazioniAnnulla,
    operazioniBatch,
    type Batch,
    type GruppoDuplicati,
  } from "../api";
  import Badge from "../ui/Badge.svelte";
  import Bottone from "../ui/Bottone.svelte";
  import Card from "../ui/Card.svelte";
  import Icona from "../ui/Icona.svelte";
  import Vuoto from "../ui/Vuoto.svelte";
  import RiepilogoPiano from "./RiepilogoPiano.svelte";
  import RigaFile from "./RigaFile.svelte";
  import { AzioniFile, AZIONI, type GenereAzione } from "./azioni.svelte";
  import { messaggioErrore } from "./comuni";

  interface Props {
    aggiornamento?: number;
  }

  let { aggiornamento = 0 }: Props = $props();

  let gruppi = $state<GruppoDuplicati[]>([]);
  let batch = $state<Batch[]>([]);
  let caricando = $state(true);
  let errore = $state<string | null>(null);
  /** Incrementato dopo ogni operazione: rilegge gruppi e batch. */
  let ricarica = $state(0);

  let scelti = $state<number[]>([]);
  let annullando = $state<string | null>(null);

  const azioni = new AzioniFile(() => {
    scelti = [];
    ricarica += 1;
  });

  $effect(() => {
    void aggiornamento;
    void ricarica;
    let vivo = true;
    caricando = true;
    Promise.all([duplicati(), operazioniBatch()])
      .then(([g, b]) => {
        if (!vivo) return;
        gruppi = g;
        batch = b;
        errore = null;
        // Le selezioni che non esistono più non devono restare nel piano.
        const vivi = new Set(g.flatMap((x) => x.duplicati.map((d) => d.id)));
        scelti = scelti.filter((id) => vivi.has(id));
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

  const spazioTotale = $derived(
    gruppi.reduce((s, g) => s + g.spazio_recuperabile, 0),
  );

  const byteScelti = $derived.by(() => {
    const per = new Map<number, number>();
    for (const g of gruppi) for (const d of g.duplicati) per.set(d.id, d.size);
    return scelti.reduce((s, id) => s + (per.get(id) ?? 0), 0);
  });

  function commutaFile(id: number) {
    scelti = scelti.includes(id)
      ? scelti.filter((x) => x !== id)
      : [...scelti, id];
  }

  function commutaGruppo(g: GruppoDuplicati) {
    const ids = g.duplicati.map((d) => d.id);
    const tutti = ids.every((id) => scelti.includes(id));
    scelti = tutti
      ? scelti.filter((id) => !ids.includes(id))
      : [...scelti, ...ids.filter((id) => !scelti.includes(id))];
  }

  function selezionaTutti() {
    scelti = gruppi.flatMap((g) => g.duplicati.map((d) => d.id));
  }

  function avvia(genere: GenereAzione) {
    void azioni.prepara(genere, scelti);
  }

  async function annulla(codice: string) {
    annullando = codice;
    try {
      await operazioniAnnulla(codice);
      ricarica += 1;
    } catch (e) {
      errore = messaggioErrore(e);
    } finally {
      annullando = null;
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
  <!-- Il patto: niente parte senza che tu l'abbia visto --------------- -->
  <Card padding="stretta">
    <div class="patto">
      <span class="patto-icona"><Icona nome="archivio" dimensione={18} /></span>
      <div class="crescente">
        <p class="patto-titolo">Niente parte senza che tu abbia visto il piano</p>
        <p class="patto-testo">
          Le copie ridondanti si possono <strong>mettere in quarantena</strong>
          — spostate in una cartella dedicata, con il percorso originale
          ricostruibile e il batch annullabile in blocco —, oppure
          <strong>mandare nel cestino</strong> di sistema, o
          <strong>cancellare</strong>. Le ultime due liberano spazio subito ma
          non si disfano da qui: la scelta è tua e il piano te la mostra prima.
          Il <strong>canonico non si tocca mai</strong>.
        </p>
      </div>
    </div>
  </Card>

  {#if errore || azioni.errore}
    <Card>
      <div class="allarme" role="alert">
        <Icona nome="avviso" dimensione={16} />
        <span>{errore ?? azioni.errore}</span>
      </div>
    </Card>
  {/if}

  {#if azioni.piano}
    <RiepilogoPiano
      piano={azioni.piano}
      esito={azioni.esito}
      inCorso={azioni.inCorso}
      testoConferma={azioni.descrizione.conferma}
      spiegazione={azioni.descrizione.spiegazione}
      pericolo={azioni.descrizione.pericolo}
      parolaChiave={azioni.descrizione.parolaChiave}
      onconferma={() => azioni.esegui()}
      onchiudi={() => azioni.chiudi()}
    />
  {/if}

  {#if !caricando && gruppi.length === 0}
    <Card>
      <Vuoto
        icona="duplicati"
        titolo="Nessuna copia ridondante"
        messaggio="Non ci sono file con lo stesso contenuto fra quelli indicizzati. Se hai appena aggiunto una sorgente, lancia una scansione: i duplicati si trovano confrontando gli hash e il testo estratto."
      />
    </Card>
  {:else}
    <!-- Barra di selezione -------------------------------------------- -->
    <Card padding="stretta">
      <div class="barra">
        <div class="conti">
          <span class="conto">
            <span class="cifre forte">{formattaNumero(gruppi.length)}</span>
            {gruppi.length === 1 ? "gruppo" : "gruppi"}
          </span>
          <span class="separa" aria-hidden="true">·</span>
          <span class="conto">
            <span class="cifre forte">{formattaByte(spazioTotale)}</span>
            recuperabili in tutto
          </span>
          {#if scelti.length > 0}
            <span class="separa" aria-hidden="true">·</span>
            <span class="conto acceso">
              <span class="cifre forte">{formattaNumero(scelti.length)}</span>
              selezionati ({formattaByte(byteScelti)})
            </span>
          {/if}
        </div>

        <div class="azioni-barra">
          <Bottone variante="fantasma" dimensione="sm" onclick={selezionaTutti}>
            Seleziona tutti i duplicati
          </Bottone>
          {#if scelti.length > 0}
            <Bottone variante="fantasma" dimensione="sm" onclick={() => (scelti = [])}>
              Deseleziona
            </Bottone>
          {/if}
          <Bottone
            variante="secondario"
            dimensione="sm"
            icona="archivio"
            disabled={scelti.length === 0}
            caricamento={azioni.inCorso && !azioni.piano}
            onclick={() => avvia("quarantena")}
          >
            {AZIONI.quarantena.bottone}
          </Bottone>
          <Bottone
            variante="secondario"
            dimensione="sm"
            icona="cestino"
            disabled={scelti.length === 0}
            onclick={() => avvia("cestino")}
          >
            {AZIONI.cestino.bottone}
          </Bottone>
          <Bottone
            variante="pericolo"
            dimensione="sm"
            icona="cestino"
            titolo="Cancella dal disco senza passare dal cestino"
            disabled={scelti.length === 0}
            onclick={() => avvia("elimina")}
          >
            {AZIONI.elimina.bottone}
          </Bottone>
        </div>
      </div>
    </Card>

    <!-- I gruppi ------------------------------------------------------ -->
    {#each gruppi as g (g.chiave)}
      {@const ids = g.duplicati.map((d) => d.id)}
      {@const tuttiScelti = ids.length > 0 && ids.every((id) => scelti.includes(id))}
      <Card padding="nessuna">
        {#snippet intestazione()}
          <div class="testa-gruppo">
            <label class="scelta-gruppo">
              <input
                type="checkbox"
                checked={tuttiScelti}
                onchange={() => commutaGruppo(g)}
              />
              <span class="testo-piccolo">
                {g.duplicati.length}
                {g.duplicati.length === 1 ? "copia" : "copie"}
              </span>
            </label>

            <div class="crescente">
              <div class="badge-riga">
                <Badge
                  testo={g.genere === "esatto" ? "stesso hash" : "stesso testo"}
                  variante={g.genere === "esatto" ? "info" : "avviso"}
                />
                <span class="chiave mono troncato" title={g.chiave}>{g.chiave}</span>
              </div>
            </div>

            <div class="recuperabile">
              <span class="cifre forte">{formattaByte(g.spazio_recuperabile)}</span>
              <span class="testo-piccolo testo-tenue">recuperabili</span>
            </div>
          </div>
        {/snippet}

        <div class="elenco">
          <RigaFile
            file={g.canonico}
            compatta
            onapri={() => apri(g.canonico.path)}
            valore={formattaByte(g.canonico.size)}
            dettaglio={formattaDataOra(g.canonico.mtime)}
          >
            <div class="badge-riga">
              <Badge testo="canonico" variante="successo" punto />
              <span class="testo-piccolo testo-tenue">
                resta dov'è: è la copia che Setaccio considera l'originale
              </span>
            </div>
            {#snippet prima()}
              <span class="segna-canonico" title="Il canonico non si tocca">
                <Icona nome="check" dimensione={14} />
              </span>
            {/snippet}
          </RigaFile>

          {#each g.duplicati as d (d.id)}
            <RigaFile
              file={d}
              compatta
              selezionata={scelti.includes(d.id)}
              onseleziona={() => commutaFile(d.id)}
              onapri={() => apri(d.path)}
              valore={formattaByte(d.size)}
              dettaglio={formattaDataOra(d.mtime)}
            >
              <div class="badge-riga">
                <Badge stato={d.stato} />
                {#if d.contesto}<Badge testo={d.contesto} variante="accento" />{/if}
              </div>
              {#snippet prima()}
                <input
                  type="checkbox"
                  checked={scelti.includes(d.id)}
                  onchange={() => commutaFile(d.id)}
                  aria-label="Metti in quarantena {d.nome}"
                />
              {/snippet}
            </RigaFile>
          {/each}
        </div>
      </Card>
    {/each}
  {/if}

  <!-- Cronologia dei batch --------------------------------------------- -->
  <Card
    titolo="Operazioni eseguite"
    sottotitolo="Gli spostamenti si annullano e i file tornano al loro posto; cestino ed eliminazione restano qui solo come traccia"
    padding="nessuna"
  >
    <div class="elenco-batch">
      {#if batch.length === 0}
        <Vuoto
          compatto
          icona="aggiorna"
          titolo="Nessuna operazione finora"
          messaggio="Quando confermerai un piano, il batch comparirà qui con il suo pulsante Annulla."
        />
      {:else}
        {#each batch as b (b.batch)}
          <div class="riga-batch" class:annullato={b.annullato}>
            <span class="tile-batch" class:definitivo={!b.annullabile}>
              <Icona
                nome={!b.annullabile
                  ? "cestino"
                  : b.annullato
                    ? "aggiorna"
                    : "archivio"}
                dimensione={16}
              />
            </span>
            <div class="crescente">
              <p class="mono troncato">{b.batch}</p>
              <p class="testo-piccolo testo-secondario">
                {b.genere} · {formattaNumero(b.quante)}
                {b.quante === 1 ? "mossa" : "mosse"} · {formattaDataOra(b.eseguito_il)}
              </p>
            </div>
            <!-- Un pulsante Annulla che fallirebbe è peggio della sua
                 assenza: qui si dice dove cercare, o che non c'è più nulla
                 da cercare. -->
            {#if !b.annullabile}
              <Badge
                testo={b.genere === "cestino"
                  ? "nel cestino di sistema"
                  : "eliminato"}
                variante={b.genere === "cestino" ? "avviso" : "pericolo"}
              />
            {:else if b.annullato}
              <Badge testo="annullato" variante="neutro" />
            {:else}
              <Bottone
                variante="secondario"
                dimensione="sm"
                icona="aggiorna"
                caricamento={annullando === b.batch}
                onclick={() => annulla(b.batch)}
              >
                Annulla
              </Bottone>
            {/if}
          </div>
        {/each}
      {/if}
    </div>
  </Card>
</div>

<style>
  .patto {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
  }

  .patto-icona {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    flex: 0 0 auto;
    border-radius: var(--raggio);
    background: var(--successo-bg);
    color: var(--successo);
  }

  .patto-titolo {
    font-size: var(--corpo);
    font-weight: var(--peso-grasso);
  }

  .patto-testo {
    margin-top: 2px;
    font-size: var(--piccolo);
    color: var(--testo-2);
    line-height: var(--riga-larga);
  }

  /* Barra di selezione ----------------------------------------------- */
  .barra {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-3);
    flex-wrap: wrap;
  }

  .conti {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    font-size: var(--piccolo);
    color: var(--testo-2);
    flex-wrap: wrap;
  }

  .conto.acceso {
    color: var(--accento-testo);
  }

  .forte {
    font-weight: var(--peso-grasso);
    color: var(--testo);
  }

  .conto.acceso .forte {
    color: inherit;
  }

  .separa {
    color: var(--testo-3);
  }

  .azioni-barra {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    flex-wrap: wrap;
  }

  /* Gruppi ------------------------------------------------------------ */
  .testa-gruppo {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    width: 100%;
    min-width: 0;
  }

  .scelta-gruppo {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    flex: 0 0 auto;
    color: var(--testo-2);
    cursor: pointer;
  }

  .chiave {
    font-size: var(--micro);
    color: var(--testo-3);
    max-width: 22ch;
  }

  .recuperabile {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    flex: 0 0 auto;
    line-height: 1.2;
  }

  .recuperabile .forte {
    font-size: var(--corpo);
  }

  .badge-riga {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
    min-width: 0;
  }

  .elenco {
    display: flex;
    flex-direction: column;
    padding: 0 var(--sp-2) var(--sp-2);
  }

  .segna-canonico {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    border-radius: 4px;
    background: var(--successo-bg);
    color: var(--successo);
  }

  input[type="checkbox"] {
    width: 16px;
    height: 16px;
    accent-color: var(--accento-scuro);
    cursor: pointer;
  }

  /* Batch -------------------------------------------------------------- */
  .elenco-batch {
    display: flex;
    flex-direction: column;
    padding: 0 var(--sp-4) var(--sp-3);
  }

  .riga-batch {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-3) 0;
    border-bottom: 1px solid var(--bordo);
    min-width: 0;
  }

  .riga-batch:last-child {
    border-bottom: none;
  }

  .riga-batch.annullato {
    opacity: 0.6;
  }

  .tile-batch {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    flex: 0 0 auto;
    border-radius: var(--raggio-sm);
    background: var(--superficie-2);
    color: var(--testo-2);
  }

  /* I batch da cui non si torna indietro si riconoscono dalla riga, non
     solo dalla pillola in fondo. */
  .tile-batch.definitivo {
    background: var(--pericolo-bg);
    color: var(--pericolo);
  }

  .allarme {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    color: var(--pericolo);
    font-size: var(--piccolo);
  }
</style>
