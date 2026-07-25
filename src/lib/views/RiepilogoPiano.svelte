<script lang="ts">
  /**
   * Il riepilogo che sta fra il piano e il disco.
   *
   * Setaccio non tocca niente finché questo pannello non viene confermato:
   * qui si legge quante mosse partiranno davvero, quante sono state saltate e
   * perché, e quanto spazio si libera. È lo stesso pannello per la quarantena
   * dei duplicati e per l'organizzazione delle cartelle.
   */
  import {
    accorciaPath,
    formattaByte,
    formattaNumero,
    type EsitoOperazioni,
    type Mossa,
    type PianoOperazioni,
  } from "../api";
  import Badge from "../ui/Badge.svelte";
  import Bottone from "../ui/Bottone.svelte";
  import Card from "../ui/Card.svelte";
  import Icona from "../ui/Icona.svelte";

  interface Props {
    piano: PianoOperazioni;
    /** Esito dell'esecuzione: quando arriva, il pannello smette di chiedere. */
    esito?: EsitoOperazioni | null;
    inCorso?: boolean;
    /** Frase che spiega cosa succede ai file. */
    spiegazione?: string;
    testoConferma?: string;
    /**
     * Il riquadro dello spazio liberato. Va spento per i piani che non
     * recuperano byte — la pulizia delle cartelle vuote — dove uno zero in
     * grande sembrerebbe un'operazione inutile invece che una di riordino.
     */
    mostraSpazio?: boolean;
    /**
     * L'operazione perde qualcosa: la nota diventa rossa e il bottone di
     * conferma smette di essere quello «primario» verde, che qui vorrebbe
     * dire la cosa sbagliata.
     */
    pericolo?: boolean;
    /**
     * Parola da digitare per sbloccare la conferma. Va usata solo per ciò che
     * non si può disfare: un attrito messo dove non serve insegna a
     * ignorarlo, e allora non protegge più dove serve davvero.
     */
    parolaChiave?: string;
    onconferma: () => void;
    onchiudi: () => void;
  }

  let {
    piano,
    esito = null,
    inCorso = false,
    spiegazione = "I file non vengono cancellati: vengono spostati in quarantena, e ogni batch resta annullabile.",
    testoConferma = "Conferma ed esegui",
    mostraSpazio = true,
    pericolo = false,
    parolaChiave,
    onconferma,
    onchiudi,
  }: Props = $props();

  const saltate = $derived(piano.mosse.filter((m) => !m.eseguibile));
  const eseguibili = $derived(piano.mosse.filter((m) => m.eseguibile));

  /** Quanto l'utente ha digitato nella casella della parola chiave. */
  let digitato = $state("");

  // Cambiando piano la conferma riparte da zero: la parola digitata per il
  // piano precedente non deve sbloccare quello nuovo.
  $effect(() => {
    void piano.batch;
    digitato = "";
  });

  const sbloccato = $derived(
    !parolaChiave || digitato.trim().toUpperCase() === parolaChiave.toUpperCase(),
  );

  /** Le mosse saltate raggruppate per avviso: gli errori si ripetono. */
  const motiviSaltate = $derived.by(() => {
    const mappa = new Map<string, Mossa[]>();
    for (const m of saltate) {
      const chiave = m.avviso ?? "motivo non specificato";
      const gia = mappa.get(chiave);
      if (gia) gia.push(m);
      else mappa.set(chiave, [m]);
    }
    return [...mappa.entries()];
  });
</script>

<Card titolo={esito ? "Operazione eseguita" : "Anteprima del piano"}>
  {#snippet azioni()}
    <Bottone
      variante="fantasma"
      dimensione="sm"
      icona="chiudi"
      soloIcona
      titolo="Chiudi il riepilogo"
      onclick={onchiudi}
    />
  {/snippet}

  <div class="impila">
    {#if esito}
      <div class="nota" class:successo={!pericolo} class:pericolo>
        <Icona nome={pericolo ? "info" : "check"} dimensione={16} />
        <p>
          {formattaNumero(esito.eseguite)} mosse eseguite,
          {formattaNumero(esito.fallite)} fallite. Batch
          <span class="mono">{esito.batch}</span>:
          <!-- Promettere un annulla che non esiste è peggio che non averlo:
               qui si dice dove cercare, o che non c'è più niente da cercare. -->
          {#if pericolo}
            resta nell'elenco come traccia di cosa è stato tolto, ma da qui non
            si torna indietro.
          {:else}
            puoi annullarlo dall'elenco dei batch.
          {/if}
        </p>
      </div>

      {#if esito.errori.length > 0}
        <ul class="motivi">
          {#each esito.errori as e, i (i)}
            <li class="motivo">
              <Icona nome="avviso" dimensione={14} />
              <span>{e}</span>
            </li>
          {/each}
        </ul>
      {/if}
    {:else}
      <div class="numeri">
        <div class="numero">
          <span class="cifra cifre">{formattaNumero(piano.eseguibili)}</span>
          <span class="etichetta">mosse eseguibili</span>
        </div>
        <div class="numero">
          <span class="cifra cifre" class:tenue={piano.saltate === 0}>
            {formattaNumero(piano.saltate)}
          </span>
          <span class="etichetta">saltate</span>
        </div>
        {#if mostraSpazio}
          <div class="numero">
            <span class="cifra cifre">{formattaByte(piano.spazio_liberato)}</span>
            <span class="etichetta">spazio liberato</span>
          </div>
        {/if}
      </div>

      <div class="nota" class:pericolo>
        <Icona nome={pericolo ? "avviso" : "info"} dimensione={16} />
        <p>{spiegazione}</p>
      </div>

      {#if motiviSaltate.length > 0}
        <div class="blocco">
          <p class="titolo-blocco">Perché {piano.saltate} mosse sono saltate</p>
          <ul class="motivi">
            {#each motiviSaltate as [avviso, mosse] (avviso)}
              <li class="motivo">
                <Icona nome="avviso" dimensione={14} />
                <span class="crescente">{avviso}</span>
                <Badge testo={String(mosse.length)} variante="avviso" />
              </li>
            {/each}
          </ul>
        </div>
      {/if}

      {#if eseguibili.length > 0}
        <div class="blocco">
          <p class="titolo-blocco">Le prime mosse che verranno eseguite</p>
          <ul class="mosse">
            <!-- La chiave è l'origine e non il `file_id`: le mosse che tolgono
                 una cartella non hanno un file dietro e arrivano tutte con
                 zero, mentre il percorso di partenza è unico in ogni piano. -->
            {#each eseguibili.slice(0, 8) as m (m.origine)}
              <li class="mossa">
                <span class="genere">{m.genere.replace(/_/g, " ")}</span>
                <span class="mono troncato" title={m.origine}>
                  {accorciaPath(m.origine, 58)}
                </span>
                <!-- Una destinazione vuota non è un percorso da mostrare a
                     metà: senza bersaglio spariscono anche la freccia e la
                     seconda colonna. -->
                {#if m.destinazione}
                  <Icona nome="freccia" dimensione={13} />
                  <span class="mono troncato" title={m.destinazione}>
                    {accorciaPath(m.destinazione, 58)}
                  </span>
                {/if}
              </li>
            {/each}
          </ul>
          {#if eseguibili.length > 8}
            <p class="resto testo-piccolo testo-tenue">
              …e altre {formattaNumero(eseguibili.length - 8)} mosse dello stesso genere.
            </p>
          {/if}
        </div>
      {/if}

      {#if parolaChiave && piano.eseguibili > 0}
        <label class="lucchetto">
          <span class="titolo-blocco">
            Scrivi «{parolaChiave}» per sbloccare la conferma
          </span>
          <input
            bind:value={digitato}
            placeholder={parolaChiave}
            autocomplete="off"
            spellcheck="false"
            aria-label="Scrivi {parolaChiave} per confermare"
          />
        </label>
      {/if}

      <div class="piede">
        <Bottone variante="secondario" onclick={onchiudi}>Annulla</Bottone>
        <Bottone
          variante={pericolo ? "pericolo" : "primario"}
          icona={pericolo ? "cestino" : "check"}
          caricamento={inCorso}
          disabled={piano.eseguibili === 0 || !sbloccato}
          onclick={onconferma}
        >
          {testoConferma}
        </Bottone>
      </div>
    {/if}
  </div>
</Card>

<style>
  .numeri {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-3);
  }

  .numero {
    flex: 1 1 140px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: var(--sp-4);
    border-radius: var(--raggio-lg);
    background: var(--superficie-2);
  }

  .cifra {
    font-size: var(--grande);
    font-weight: var(--peso-grasso);
    letter-spacing: -0.02em;
  }

  .cifra.tenue {
    color: var(--testo-3);
  }

  .etichetta {
    font-size: var(--minuto);
    color: var(--testo-2);
  }

  .nota {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-2);
    padding: var(--sp-3) var(--sp-4);
    border-radius: var(--raggio);
    background: var(--info-bg);
    color: var(--info);
    font-size: var(--piccolo);
    line-height: var(--riga-larga);
  }

  .nota.successo {
    background: var(--successo-bg);
    color: var(--successo);
  }

  .nota.pericolo {
    background: var(--pericolo-bg);
    color: var(--pericolo);
  }

  /* Il lucchetto della conferma testuale ------------------------------- */
  .lucchetto {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .lucchetto input {
    width: 100%;
    max-width: 260px;
    height: 38px;
    padding: 0 var(--sp-3);
    border-radius: var(--raggio);
    border: 1px solid var(--pericolo);
    background: var(--superficie);
    color: var(--testo);
    font-family: var(--famiglia-mono);
    font-size: var(--corpo);
    letter-spacing: 0.08em;
  }

  .lucchetto input:focus-visible {
    outline: 2px solid var(--pericolo);
    outline-offset: 1px;
  }

  .blocco {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .titolo-blocco {
    font-size: var(--minuto);
    font-weight: var(--peso-forte);
    color: var(--testo-2);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .motivi,
  .mosse {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
  }

  .motivo {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-2) var(--sp-3);
    border-radius: var(--raggio-sm);
    background: var(--avviso-bg);
    color: var(--avviso);
    font-size: var(--piccolo);
  }

  .mossa {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-2) var(--sp-3);
    border-radius: var(--raggio-sm);
    background: var(--superficie-2);
    color: var(--testo-2);
    min-width: 0;
  }

  .mossa .mono {
    flex: 1 1 0;
  }

  .genere {
    flex: 0 0 auto;
    font-size: var(--micro);
    font-weight: var(--peso-forte);
    color: var(--testo-3);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .piede {
    display: flex;
    justify-content: flex-end;
    gap: var(--sp-2);
  }
</style>
