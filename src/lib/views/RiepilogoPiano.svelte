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
    onconferma: () => void;
    onchiudi: () => void;
  }

  let {
    piano,
    esito = null,
    inCorso = false,
    spiegazione = "I file non vengono cancellati: vengono spostati in quarantena, e ogni batch resta annullabile.",
    testoConferma = "Conferma ed esegui",
    onconferma,
    onchiudi,
  }: Props = $props();

  const saltate = $derived(piano.mosse.filter((m) => !m.eseguibile));
  const eseguibili = $derived(piano.mosse.filter((m) => m.eseguibile));

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
      <div class="nota successo">
        <Icona nome="check" dimensione={16} />
        <p>
          {formattaNumero(esito.eseguite)} mosse eseguite,
          {formattaNumero(esito.fallite)} fallite. Batch
          <span class="mono">{esito.batch}</span>: puoi annullarlo dall'elenco
          dei batch.
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
        <div class="numero">
          <span class="cifra cifre">{formattaByte(piano.spazio_liberato)}</span>
          <span class="etichetta">spazio liberato</span>
        </div>
      </div>

      <div class="nota">
        <Icona nome="info" dimensione={16} />
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
            {#each eseguibili.slice(0, 8) as m (m.file_id)}
              <li class="mossa">
                <span class="genere">{m.genere}</span>
                <span class="mono troncato" title={m.origine}>
                  {accorciaPath(m.origine, 58)}
                </span>
                <Icona nome="freccia" dimensione={13} />
                <span class="mono troncato" title={m.destinazione}>
                  {accorciaPath(m.destinazione, 58)}
                </span>
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

      <div class="piede">
        <Bottone variante="secondario" onclick={onchiudi}>Annulla</Bottone>
        <Bottone
          variante="primario"
          icona="check"
          caricamento={inCorso}
          disabled={piano.eseguibili === 0}
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
