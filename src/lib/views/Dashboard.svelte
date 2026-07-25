<script lang="ts">
  /**
   * Lo stato dell'indice a colpo d'occhio.
   *
   * L'unico elemento squillante della schermata è la card lime: ci sta sopra
   * il numero che racconta il prodotto, cioè quanti artefatti di build sono
   * stati tenuti fuori dalle ricerche. Tutto il resto è neutro.
   */
  import {
    formattaByte,
    formattaDataOra,
    formattaNumero,
    statistiche,
    type Statistiche,
  } from "../api";
  import BarChart from "../ui/BarChart.svelte";
  import Bottone from "../ui/Bottone.svelte";
  import Card from "../ui/Card.svelte";
  import Gauge from "../ui/Gauge.svelte";
  import Icona from "../ui/Icona.svelte";
  import StatTile from "../ui/StatTile.svelte";
  import Vuoto from "../ui/Vuoto.svelte";
  import type { Sezione } from "../ui/Sidebar.svelte";
  import { messaggioErrore, tipoAlPlurale } from "./comuni";

  interface Props {
    onnaviga: (s: Sezione) => void;
    /** Cambia valore a ogni fine scansione: serve a rileggere le statistiche. */
    aggiornamento?: number;
  }

  let { onnaviga, aggiornamento = 0 }: Props = $props();

  let dati = $state<Statistiche | null>(null);
  let errore = $state<string | null>(null);
  let caricando = $state(true);
  /** Incrementato dal pulsante «Riprova»: rilancia l'effetto di lettura. */
  let ricarica = $state(0);

  $effect(() => {
    // Leggere `aggiornamento` e `ricarica` qui dentro è ciò che rende
    // l'effetto sensibile alla fine di una scansione e al pulsante.
    void aggiornamento;
    void ricarica;
    let vivo = true;
    caricando = true;
    statistiche()
      .then((s) => {
        if (!vivo) return;
        dati = s;
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

  const vuoto = $derived(!caricando && !errore && (dati?.file_totali ?? 0) === 0);

  /** I tipi con almeno un file, dal più numeroso: le barre a zero non dicono nulla. */
  const perTipo = $derived(
    (dati?.per_tipo ?? [])
      .filter((c) => c.quanti > 0)
      .slice()
      .sort((a, b) => b.quanti - a.quanti),
  );

  const contesti = $derived(
    (dati?.per_contesto ?? []).slice().sort((a, b) => b.byte - a.byte),
  );

  /**
   * La quota di spazio indicizzato occupata da copie ridondanti. Il backend
   * non conosce la capacità del disco, quindi il fondo scala del gauge è lo
   * spazio indicizzato, non quello del volume.
   */
  const quotaDuplicati = $derived(
    dati && dati.byte_totali > 0 ? dati.spazio_duplicati / dati.byte_totali : 0,
  );
</script>

{#if errore}
  <Card>
    <div class="allarme" role="alert">
      <Icona nome="avviso" dimensione={18} />
      <p class="crescente">{errore}</p>
      <Bottone
        variante="secondario"
        dimensione="sm"
        icona="aggiorna"
        onclick={() => (ricarica += 1)}
      >
        Riprova
      </Bottone>
    </div>
  </Card>
{:else if vuoto}
  <Card>
    <Vuoto
      icona="cartella"
      titolo="L'indice è ancora vuoto"
      messaggio="Aggiungi almeno una cartella da indicizzare nelle impostazioni, poi lancia una scansione con il pulsante Scansiona qui sopra."
      testoAzione="Aggiungi una sorgente"
      iconaAzione="piu"
      onazione={() => onnaviga("impostazioni")}
    />
  </Card>
{:else if dati}
  <div class="pagina">
    <!-- Colonna principale -------------------------------------------- -->
    <div class="principale">
      <div class="griglia griglia-3">
        <!-- L'unico gradiente della schermata. -->
        <Card accento padding="larga">
          <div class="spicco-dentro">
            <span class="spicco-icona"><Icona nome="artefatto" dimensione={20} /></span>
            <p class="spicco-cifra cifre">{formattaNumero(dati.artefatti_esclusi)}</p>
            <p class="spicco-titolo">artefatti tenuti fuori</p>
            <p class="spicco-testo">
              Fixture, output di build e cartelle di dipendenze: sono indicizzati
              ma non inquinano più le ricerche.
            </p>
          </div>
        </Card>

        <StatTile
          etichetta="File indicizzati"
          valore={formattaNumero(dati.file_totali)}
          icona="dashboard"
          periodo={dati.ultima_scansione
            ? `ultima scansione ${formattaDataOra(dati.ultima_scansione)}`
            : "mai scansionato"}
        />

        <StatTile
          etichetta="Spazio indicizzato"
          valore={formattaByte(dati.byte_totali)}
          icona="ingombro"
          tono="archivio"
          periodo="apri per vedere dove sta il peso"
          onclick={() => onnaviga("ingombro")}
        />
      </div>

      <div class="griglia griglia-4">
        <StatTile
          etichetta="Documenti"
          valore={formattaNumero(dati.documenti)}
          icona="documento"
          tono="documento"
        />
        <StatTile
          etichetta="Tracciati"
          valore={formattaNumero(dati.tracciati)}
          icona="tracciati"
          tono="tracciato"
        />
        <StatTile
          etichetta="Gruppi duplicati"
          valore={formattaNumero(dati.gruppi_duplicati)}
          icona="duplicati"
          tono="duplicato"
          onclick={() => onnaviga("duplicati")}
        />
        <StatTile
          etichetta="Non classificati"
          valore={formattaNumero(dati.non_classificati)}
          icona="revisione"
          tono="avviso"
          onclick={() => onnaviga("revisione")}
        />
      </div>

      <Card
        titolo="Distribuzione per tipo"
        sottotitolo="Quanti file per ciascun asse deterministico"
        padding="nessuna"
      >
        <div class="grafico">
          <BarChart
            etichette={perTipo.map((c) => tipoAlPlurale(c.etichetta))}
            serie={[{ nome: "File", valori: perTipo.map((c) => c.quanti) }]}
            formato={(n) => formattaNumero(Math.round(n))}
            legenda={false}
            altezza={240}
            messaggioVuoto="Nessun file classificato: lancia una scansione."
          />
        </div>
      </Card>
    </div>

    <!-- Colonna di destra: lo spazio ------------------------------------ -->
    <div class="laterale">
      <Card titolo="Spazio" sottotitolo="Quanto pesa l'indice, e quanto è ridondante">
        <div class="impila">
          <Gauge
            valore={dati.spazio_duplicati}
            massimo={Math.max(dati.byte_totali, 1)}
            testo={formattaByte(dati.spazio_duplicati)}
            etichetta="recuperabile dai duplicati"
            dimensione={200}
            titolo="Spazio recuperabile: {formattaByte(
              dati.spazio_duplicati,
            )} su {formattaByte(dati.byte_totali)} indicizzati"
          />

          <div class="coppia">
            <div class="voce-numero">
              <span class="numero cifre">{formattaByte(dati.byte_totali)}</span>
              <span class="etichetta-numero">spazio indicizzato</span>
            </div>
            <div class="voce-numero">
              <span class="numero cifre">
                {(quotaDuplicati * 100).toFixed(1).replace(".", ",")}%
              </span>
              <span class="etichetta-numero">quota ridondante</span>
            </div>
          </div>

          {#if dati.gruppi_duplicati > 0}
            <Bottone
              variante="secondario"
              piena
              iconaDestra="freccia"
              onclick={() => onnaviga("duplicati")}
            >
              Vedi i {formattaNumero(dati.gruppi_duplicati)} gruppi
            </Bottone>
          {/if}
        </div>
      </Card>

      <Card titolo="Contesti" sottotitolo="Dove stanno i file, per volume" padding="nessuna">
        <div class="lista">
          {#if contesti.length === 0}
            <Vuoto
              compatto
              icona="filtro"
              titolo="Nessun contesto assegnato"
              messaggio="Insegna a Setaccio dove mettere le cose dalla sezione Revisione: bastano pochi clic per creare le prime regole."
            />
          {:else}
            {#each contesti as c (c.etichetta)}
              <div class="voce-contesto">
                <span class="tile-contesto"><Icona nome="cartella" dimensione={17} /></span>
                <div class="testi">
                  <p class="nome troncato">{c.etichetta}</p>
                  <p class="sotto">{formattaNumero(c.quanti)} file</p>
                </div>
                <span class="valore cifre">{formattaByte(c.byte)}</span>
              </div>
            {/each}
          {/if}
        </div>
      </Card>
    </div>
  </div>
{:else}
  <Card>
    <p class="testo-secondario">Lettura delle statistiche…</p>
  </Card>
{/if}

<style>
  .pagina {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 340px;
    gap: var(--sp-4);
    align-items: start;
  }

  .principale,
  .laterale {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    min-width: 0;
  }

  @media (max-width: 1180px) {
    .pagina {
      grid-template-columns: minmax(0, 1fr);
    }
  }

  /* Card di spicco -------------------------------------------------- */
  .spicco-dentro {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    height: 100%;
  }

  .spicco-icona {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    margin-bottom: var(--sp-2);
    border-radius: var(--raggio);
    background: rgba(16, 27, 4, 0.14);
    color: var(--su-accento);
  }

  .spicco-cifra {
    font-size: var(--cifra);
    font-weight: var(--peso-grasso);
    line-height: 1.05;
    letter-spacing: -0.02em;
    color: var(--su-accento);
  }

  .spicco-titolo {
    font-size: var(--corpo);
    font-weight: var(--peso-forte);
    color: var(--su-accento);
  }

  .spicco-testo {
    margin-top: var(--sp-2);
    font-size: var(--minuto);
    line-height: var(--riga-larga);
    color: rgba(16, 27, 4, 0.72);
  }

  .grafico {
    padding: 0 var(--sp-4) var(--sp-4);
  }

  /* Colonna di destra ------------------------------------------------ */
  .coppia {
    display: flex;
    gap: var(--sp-3);
  }

  .voce-numero {
    flex: 1 1 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: var(--sp-3) var(--sp-4);
    border-radius: var(--raggio-lg);
    background: var(--superficie-2);
    min-width: 0;
  }

  .numero {
    font-size: var(--medio);
    font-weight: var(--peso-grasso);
    letter-spacing: -0.01em;
  }

  .etichetta-numero {
    font-size: var(--micro);
    color: var(--testo-2);
  }

  .lista {
    display: flex;
    flex-direction: column;
    padding: 0 var(--sp-3) var(--sp-3);
    max-height: 420px;
    overflow-y: auto;
  }

  .voce-contesto {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-2);
    border-radius: var(--raggio);
    min-width: 0;
  }

  .voce-contesto:hover {
    background: var(--superficie-2);
  }

  .tile-contesto {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    flex: 0 0 auto;
    border-radius: var(--raggio-sm);
    background: var(--superficie-2);
    color: var(--testo-2);
  }

  .testi {
    flex: 1 1 auto;
    min-width: 0;
  }

  .nome {
    font-size: var(--piccolo);
    font-weight: var(--peso-forte);
  }

  .sotto {
    font-size: var(--micro);
    color: var(--testo-2);
  }

  .valore {
    flex: 0 0 auto;
    font-size: var(--piccolo);
    font-weight: var(--peso-forte);
    white-space: nowrap;
  }

  .allarme {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    color: var(--pericolo);
    font-size: var(--piccolo);
  }
</style>
