<script lang="ts">
  /**
   * Dove sono finiti i gigabyte, e cosa farne.
   *
   * La domanda «il disco è pieno, di cosa?» non si risponde con un solo
   * elenco. Un elenco per dimensione mostra i file grossi; quasi sempre
   * quello che serve sapere è *quale cartella* pesa, perché trecento file da
   * 40 MB nello stesso posto contano più di un singolo file da 4 GB e in un
   * elenco per dimensione non compaiono nemmeno. Qui le due letture stanno
   * una accanto all'altra, sugli stessi filtri, più la ripartizione per
   * estensione che dice *che roba è*.
   *
   * Dalla selezione partono le tre azioni di `azioni.svelte.ts`, con la
   * distanza fra loro che meritano: la quarantena è a un clic, il cestino
   * chiede conferma sul piano, l'eliminazione definitiva chiede anche di
   * scrivere una parola.
   */
  import { openPath } from "@tauri-apps/plugin-opener";
  import {
    filtriVuoti,
    formattaByte,
    formattaDataOra,
    formattaNumero,
    ingombro as leggiIngombro,
    TIPI,
    type CartellaPesante,
    type Filtri,
    type Ingombro,
  } from "../api";
  import Badge from "../ui/Badge.svelte";
  import Bottone from "../ui/Bottone.svelte";
  import Card from "../ui/Card.svelte";
  import Icona from "../ui/Icona.svelte";
  import Interruttore from "../ui/Interruttore.svelte";
  import StatTile from "../ui/StatTile.svelte";
  import Vuoto from "../ui/Vuoto.svelte";
  import RiepilogoPiano from "./RiepilogoPiano.svelte";
  import RigaFile from "./RigaFile.svelte";
  import { AzioniFile, AZIONI, type GenereAzione } from "./azioni.svelte";
  import { cartellaDi, messaggioErrore, tipoAlPlurale } from "./comuni";

  interface Props {
    /** Cambia a fine scansione: fa rileggere i dati. */
    aggiornamento?: number;
  }

  let { aggiornamento = 0 }: Props = $props();

  const MB = 1024 * 1024;

  /** Soglie di partenza. La prima è quella su cui si apre la vista. */
  const SOGLIE = [
    { byte: 100 * MB, etichetta: "> 100 MB" },
    { byte: 10 * MB, etichetta: "> 10 MB" },
    { byte: 1024 * MB, etichetta: "> 1 GB" },
    { byte: 0, etichetta: "Tutti" },
  ] as const;

  /** Quanti file elencare. I totali restano su tutto: li calcola il backend. */
  const QUANTI = 100;

  let soglia = $state<number>(100 * MB);
  let tipiSel = $state<string[]>([]);
  let includiArtefatti = $state(false);

  let dati = $state<Ingombro | null>(null);
  let caricando = $state(true);
  let errore = $state<string | null>(null);
  /** Incrementato dopo ogni operazione: rilegge i dati. */
  let ricarica = $state(0);

  let scelti = $state<number[]>([]);

  const azioni = new AzioniFile(() => {
    scelti = [];
    ricarica += 1;
  });

  const filtri = $derived<Filtri>({
    ...filtriVuoti(),
    tipi: tipiSel,
    size_min: soglia > 0 ? soglia : null,
    includi_artefatti: includiArtefatti,
    ordine: "dimensione",
    limite: QUANTI,
  });

  $effect(() => {
    const f = filtri;
    void aggiornamento;
    void ricarica;
    let vivo = true;
    caricando = true;
    leggiIngombro(f, QUANTI)
      .then((d) => {
        if (!vivo) return;
        dati = d;
        errore = null;
        // Le selezioni che non sono più nell'elenco non devono restare nel
        // piano: sono file che i filtri hanno escluso o che non ci sono più.
        const vivi = new Set(d.file.map((x) => x.id));
        scelti = scelti.filter((id) => vivi.has(id));
      })
      .catch((e) => {
        if (!vivo) return;
        dati = null;
        errore = messaggioErrore(e);
      })
      .finally(() => {
        if (vivo) caricando = false;
      });
    return () => {
      vivo = false;
    };
  });

  // ---- Numeri di testa ---------------------------------------------------

  /** Quanto pesano i file elencati sul totale che passa i filtri. */
  const quotaMostrata = $derived(
    dati && dati.byte_totali > 0
      ? Math.round((dati.byte_mostrati / dati.byte_totali) * 100)
      : 0,
  );

  const byteScelti = $derived.by(() => {
    if (!dati) return 0;
    const per = new Map(dati.file.map((f) => [f.id, f.size]));
    return scelti.reduce((s, id) => s + (per.get(id) ?? 0), 0);
  });

  /** Il file più grande fa da fondo scala alle barre dell'elenco. */
  const massimoFile = $derived(dati?.file[0]?.size ?? 0);
  const massimoCartella = $derived(dati?.cartelle[0]?.byte ?? 0);
  const massimoEstensione = $derived(dati?.per_estensione[0]?.byte ?? 0);

  function percentuale(valore: number, massimo: number): number {
    return massimo > 0 ? Math.max(1, Math.round((valore / massimo) * 100)) : 0;
  }

  // ---- Selezione ---------------------------------------------------------

  function commuta(id: number) {
    scelti = scelti.includes(id)
      ? scelti.filter((x) => x !== id)
      : [...scelti, id];
  }

  /** Seleziona tutti i file di una cartella fra quelli elencati. */
  function selezionaCartella(c: CartellaPesante) {
    if (!dati) return;
    const dentro = dati.file
      .filter((f) => f.path.startsWith(`${c.path}/`) || f.path.startsWith(`${c.path}\\`))
      .map((f) => f.id);
    const tutti = dentro.length > 0 && dentro.every((id) => scelti.includes(id));
    scelti = tutti
      ? scelti.filter((id) => !dentro.includes(id))
      : [...scelti, ...dentro.filter((id) => !scelti.includes(id))];
  }

  function commutaTipo(t: string) {
    tipiSel = tipiSel.includes(t)
      ? tipiSel.filter((x) => x !== t)
      : [...tipiSel, t];
  }

  async function apri(path: string) {
    try {
      await openPath(path);
    } catch (e) {
      errore = messaggioErrore(e);
    }
  }

  function avvia(genere: GenereAzione) {
    void azioni.prepara(genere, scelti);
  }
</script>

<div class="impila">
  <!-- Numeri di testa --------------------------------------------------- -->
  <div class="tiles">
    <StatTile
      valore={formattaByte(dati?.byte_totali ?? 0)}
      etichetta={soglia > 0
        ? `indicizzati oltre ${formattaByte(soglia)}`
        : "indicizzati in tutto"}
      icona="ingombro"
      tono="documento"
    />
    <StatTile
      valore={formattaNumero(dati?.quanti_totali ?? 0)}
      unita="file"
      etichetta="passano questi filtri"
      icona="filtro"
    />
    <StatTile
      valore={quotaMostrata}
      unita="%"
      etichetta="del peso sta nei primi {formattaNumero(dati?.file.length ?? 0)}"
      icona="documento"
      tono="media"
    />
    <StatTile
      valore={formattaByte(dati?.cartelle[0]?.byte ?? 0)}
      etichetta={dati?.cartelle[0]
        ? `nella cartella ${dati.cartelle[0].nome}`
        : "nessuna cartella"}
      icona="cartella"
      tono="archivio"
    />
  </div>

  <!-- Filtri ------------------------------------------------------------ -->
  <Card padding="stretta">
    <div class="barra-filtri">
      <div class="soglie" role="group" aria-label="Dimensione minima">
        {#each SOGLIE as s (s.byte)}
          <button
            class="pillola"
            class:acceso={soglia === s.byte}
            aria-pressed={soglia === s.byte}
            onclick={() => (soglia = s.byte)}
          >
            {s.etichetta}
          </button>
        {/each}
      </div>

      <!-- `artefatto` è fra i tipi scegliibili: gli output di build sono
           spesso la voce più grossa del disco, e qui si viene apposta a
           guardare cosa pesa. -->
      <div class="tipi" role="group" aria-label="Tipo di file">
        {#each TIPI as t (t)}
          <button
            class="pillola minuta"
            class:acceso={tipiSel.includes(t)}
            aria-pressed={tipiSel.includes(t)}
            onclick={() => commutaTipo(t)}
          >
            {tipoAlPlurale(t)}
          </button>
        {/each}
      </div>

      <!-- Scelto un tipo, l'interruttore non avrebbe più niente da fare: un
           file è artefatto *oppure* documento, mai tutti e due, quindi il
           filtro per tipo decide già da solo. Meglio spegnerlo e dirlo che
           lasciarlo cliccabile senza effetto. -->
      <Interruttore
        bind:attivo={includiArtefatti}
        dimensione="sm"
        etichetta="Anche gli artefatti"
        disabilitato={tipiSel.length > 0}
        titolo={tipiSel.length > 0
          ? "Hai già scelto uno o più tipi: sono quelli a decidere"
          : "Includi gli output di build che stanno dentro repository di codice"}
      />
    </div>
  </Card>

  {#if errore}
    <Card>
      <div class="allarme" role="alert">
        <Icona nome="avviso" dimensione={16} />
        <span>{errore}</span>
      </div>
    </Card>
  {/if}

  {#if azioni.errore}
    <Card>
      <div class="allarme" role="alert">
        <Icona nome="avviso" dimensione={16} />
        <span>{azioni.errore}</span>
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

  <!-- Barra della selezione --------------------------------------------- -->
  {#if scelti.length > 0}
    <Card padding="stretta">
      <div class="barra-scelta">
        <div class="conti">
          <span class="cifre forte">{formattaNumero(scelti.length)}</span>
          {scelti.length === 1 ? "file scelto" : "file scelti"} ·
          <span class="cifre forte">{formattaByte(byteScelti)}</span>
        </div>
        <div class="azioni-barra">
          <Bottone variante="fantasma" dimensione="sm" onclick={() => (scelti = [])}>
            Deseleziona
          </Bottone>
          <Bottone
            variante="secondario"
            dimensione="sm"
            icona="archivio"
            caricamento={azioni.inCorso && !azioni.piano}
            onclick={() => avvia("quarantena")}
          >
            {AZIONI.quarantena.bottone}
          </Bottone>
          <Bottone
            variante="secondario"
            dimensione="sm"
            icona="cestino"
            onclick={() => avvia("cestino")}
          >
            {AZIONI.cestino.bottone}
          </Bottone>
          <Bottone
            variante="pericolo"
            dimensione="sm"
            icona="cestino"
            titolo="Cancella dal disco senza passare dal cestino"
            onclick={() => avvia("elimina")}
          >
            {AZIONI.elimina.bottone}
          </Bottone>
        </div>
      </div>
    </Card>
  {/if}

  {#if !caricando && dati && dati.quanti_totali === 0}
    <Card>
      <Vuoto
        icona="ingombro"
        titolo="Niente sopra questa soglia"
        messaggio="Abbassa la dimensione minima qui sopra, oppure lancia una scansione: l'ingombro si calcola su ciò che è già nell'indice."
      />
    </Card>
  {:else if dati}
    <div class="colonne">
      <!-- I file più grandi --------------------------------------------- -->
      <Card
        titolo="I file più grandi"
        sottotitolo="Doppio clic per aprire · la barra è in scala sul più pesante"
        padding="nessuna"
      >
        <div class="elenco">
          {#each dati.file as f (f.id)}
            <RigaFile
              file={f}
              compatta
              selezionata={scelti.includes(f.id)}
              onseleziona={() => commuta(f.id)}
              onapri={() => apri(f.path)}
              valore={formattaByte(f.size)}
              dettaglio={formattaDataOra(f.mtime)}
            >
              <div class="sotto-riga">
                <div class="barra-peso" aria-hidden="true">
                  <span style="width: {percentuale(f.size, massimoFile)}%"></span>
                </div>
                <div class="badge-riga">
                  <Badge tipo={f.tipo} />
                  {#if f.stato !== "canonico"}<Badge stato={f.stato} />{/if}
                  {#if f.contesto}<Badge testo={f.contesto} variante="accento" />{/if}
                </div>
              </div>
              {#snippet prima()}
                <input
                  type="checkbox"
                  checked={scelti.includes(f.id)}
                  onchange={() => commuta(f.id)}
                  aria-label="Scegli {f.nome}"
                />
              {/snippet}
            </RigaFile>
          {/each}
        </div>
      </Card>

      <div class="impila">
        <!-- Le cartelle più pesanti ------------------------------------- -->
        <Card
          titolo="Le cartelle che pesano di più"
          sottotitolo="Con dentro tutto ciò che hanno sotto"
          padding="nessuna"
        >
          {#if dati.cartelle.length === 0}
            <div class="niente">
              <Vuoto
                compatto
                icona="cartella"
                titolo="Nessuna cartella da mostrare"
                messaggio="Con questi filtri resta troppo poco per dire dove sta il peso."
              />
            </div>
          {:else}
            <ul class="elenco-cartelle">
              {#each dati.cartelle as c (c.path)}
                <!-- Scegliere e aprire sono due pulsanti separati, non un
                     clic e un doppio clic sullo stesso: il doppio clic fa
                     scattare due volte anche l'`onclick`, e una selezione
                     che si commuta due volte torna al punto di partenza
                     portandosi via quello che l'utente aveva già scelto. -->
                <li class="voce-cartella">
                  <button
                    class="cartella"
                    title="Scegli i file di questa cartella fra quelli elencati&#10;{c.path}"
                    onclick={() => selezionaCartella(c)}
                  >
                    <span class="riga-cartella">
                      <Icona nome="cartella" dimensione={14} />
                      <span class="nome-cartella troncato">{c.nome}</span>
                      <span class="peso cifre">{formattaByte(c.byte)}</span>
                    </span>
                    <span class="barra-peso" aria-hidden="true">
                      <span style="width: {percentuale(c.byte, massimoCartella)}%"></span>
                    </span>
                    <span class="dove troncato">{cartellaDi(c.path)}</span>
                    <span class="dettagli">
                      {formattaNumero(c.quanti)} file
                      {#if c.byte_diretti > 0}
                        · {formattaByte(c.byte_diretti)} qui
                      {:else}
                        · tutto più in basso
                      {/if}
                    </span>
                  </button>
                  <Bottone
                    variante="fantasma"
                    dimensione="sm"
                    icona="esterno"
                    soloIcona
                    titolo="Apri «{c.nome}» nel gestore file"
                    onclick={() => apri(c.path)}
                  />
                </li>
              {/each}
            </ul>
          {/if}
        </Card>

        <!-- Per estensione ---------------------------------------------- -->
        <Card titolo="Per estensione" padding="nessuna">
          <ul class="elenco-cartelle">
            {#each dati.per_estensione as e (e.etichetta)}
              <li>
                <div class="cartella statica">
                  <span class="riga-cartella">
                    <span class="nome-cartella mono troncato">{e.etichetta}</span>
                    <span class="peso cifre">{formattaByte(e.byte)}</span>
                  </span>
                  <span class="barra-peso" aria-hidden="true">
                    <span style="width: {percentuale(e.byte, massimoEstensione)}%"></span>
                  </span>
                  <span class="dettagli">{formattaNumero(e.quanti)} file</span>
                </div>
              </li>
            {/each}
          </ul>
        </Card>
      </div>
    </div>
  {/if}
</div>

<style>
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: var(--sp-3);
  }

  /* Filtri ------------------------------------------------------------- */
  .barra-filtri {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    flex-wrap: wrap;
  }

  .soglie,
  .tipi {
    display: inline-flex;
    gap: 2px;
    padding: 3px;
    border-radius: var(--raggio-pillola);
    background: var(--superficie-2);
    border: 1px solid var(--bordo);
    flex-wrap: wrap;
  }

  .pillola {
    height: 28px;
    padding: 0 var(--sp-4);
    border-radius: var(--raggio-pillola);
    font-size: var(--minuto);
    font-weight: var(--peso-forte);
    color: var(--testo-2);
    white-space: nowrap;
    transition:
      background var(--transizione),
      color var(--transizione);
  }

  .pillola.minuta {
    padding: 0 var(--sp-3);
    font-size: var(--micro);
  }

  .pillola:hover {
    color: var(--testo);
  }

  .pillola.acceso {
    background: var(--superficie);
    color: var(--testo);
    box-shadow: var(--ombra-1);
  }

  /* Barra della selezione ---------------------------------------------- */
  .barra-scelta {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-3);
    flex-wrap: wrap;
  }

  .conti {
    font-size: var(--piccolo);
    color: var(--testo-2);
  }

  .forte {
    font-weight: var(--peso-grasso);
    color: var(--testo);
  }

  .azioni-barra {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    flex-wrap: wrap;
  }

  /* Le due colonne ------------------------------------------------------ */
  .colonne {
    display: grid;
    grid-template-columns: minmax(0, 1.55fr) minmax(0, 1fr);
    align-items: start;
    gap: var(--sp-4);
  }

  @media (max-width: 1100px) {
    .colonne {
      grid-template-columns: minmax(0, 1fr);
    }
  }

  .elenco {
    display: flex;
    flex-direction: column;
    padding: 0 var(--sp-2) var(--sp-2);
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

  /* La barra in scala: è il segnale che si legge prima del numero. */
  .barra-peso {
    display: block;
    height: 4px;
    width: 100%;
    border-radius: var(--raggio-pillola);
    background: var(--superficie-3);
    overflow: hidden;
  }

  .barra-peso span {
    display: block;
    height: 100%;
    border-radius: var(--raggio-pillola);
    background: var(--gradiente-accento);
  }

  /* Cartelle ed estensioni ---------------------------------------------- */
  .elenco-cartelle {
    display: flex;
    flex-direction: column;
    padding: 0 var(--sp-3) var(--sp-3);
  }

  .voce-cartella {
    display: flex;
    align-items: center;
    gap: var(--sp-1);
    min-width: 0;
  }

  /* Il pulsante «apri» resta discreto finché non si passa sulla riga: la
     lista si legge come un elenco di pesi, non come una barra di comandi. */
  .voce-cartella :global(button.fantasma) {
    opacity: 0;
    flex: 0 0 auto;
    transition: opacity var(--transizione);
  }

  .voce-cartella:hover :global(button.fantasma),
  .voce-cartella :global(button.fantasma:focus-visible) {
    opacity: 1;
  }

  .cartella {
    display: flex;
    flex-direction: column;
    gap: 5px;
    flex: 1 1 auto;
    min-width: 0;
    padding: var(--sp-2) var(--sp-3);
    border-radius: var(--raggio);
    text-align: left;
    transition: background var(--transizione);
  }

  .cartella:not(.statica):hover {
    background: var(--superficie-2);
    cursor: pointer;
  }

  .riga-cartella {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    min-width: 0;
    color: var(--testo-2);
  }

  .nome-cartella {
    flex: 1 1 auto;
    min-width: 0;
    font-size: var(--piccolo);
    font-weight: var(--peso-forte);
    color: var(--testo);
  }

  .peso {
    flex: 0 0 auto;
    font-size: var(--piccolo);
    font-weight: var(--peso-forte);
    color: var(--testo);
    white-space: nowrap;
  }

  .dove,
  .dettagli {
    display: block;
    font-size: var(--micro);
    color: var(--testo-3);
    min-width: 0;
  }

  .dove {
    font-family: var(--famiglia-mono);
  }

  .niente {
    padding: var(--sp-3);
  }

  input[type="checkbox"] {
    width: 16px;
    height: 16px;
    accent-color: var(--accento-scuro);
    cursor: pointer;
  }

  .allarme {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    color: var(--pericolo);
    font-size: var(--piccolo);
  }
</style>
