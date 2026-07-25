<script lang="ts">
  /**
   * Sorgenti, regole e organizzazione.
   *
   * È la vista in cui si spiegano le due scelte che sorprendono di più chi
   * apre Setaccio per la prima volta: perché una cartella va dichiarata di
   * fascia «documenti» o «tracciati», e perché di default quello che sta
   * dentro un repository di codice non viene indicizzato.
   */
  import { open } from "@tauri-apps/plugin-dialog";
  import {
    accorciaPath,
    faccette,
    FASCE,
    formattaNumero,
    operazioniEsegui,
    organizzaPiano,
    regolaAttiva,
    regolaElimina,
    regoleLista,
    sorgenteAggiorna,
    sorgenteAggiungi,
    sorgenteRimuovi,
    sorgentiLista,
    sorgentiSuggerite,
    type ConteggioEtichetta,
    type EsitoOperazioni,
    type Fascia,
    type PianoOperazioni,
    type Regola,
    type Sorgente,
  } from "../api";
  import Badge from "../ui/Badge.svelte";
  import Bottone from "../ui/Bottone.svelte";
  import Campo from "../ui/Campo.svelte";
  import Card from "../ui/Card.svelte";
  import Icona from "../ui/Icona.svelte";
  import Interruttore from "../ui/Interruttore.svelte";
  import Vuoto from "../ui/Vuoto.svelte";
  import RiepilogoPiano from "./RiepilogoPiano.svelte";
  import { messaggioErrore } from "./comuni";

  let sorgenti = $state<Sorgente[]>([]);
  let suggerite = $state<[string, string][]>([]);
  let regole = $state<Regola[]>([]);
  let contesti = $state<ConteggioEtichetta[]>([]);
  let errore = $state<string | null>(null);
  let caricando = $state(true);
  let ricarica = $state(0);

  let fasciaNuova = $state<Fascia>("documenti");
  let aggiungendo = $state(false);

  // ---- Organizza ---------------------------------------------------------
  let destinazione = $state("");
  let contestiScelti = $state<string[]>([]);
  let piano = $state<PianoOperazioni | null>(null);
  let esito = $state<EsitoOperazioni | null>(null);
  let inCorso = $state(false);

  $effect(() => {
    void ricarica;
    let vivo = true;
    caricando = true;
    Promise.all([sorgentiLista(), regoleLista(), sorgentiSuggerite(), faccette()])
      .then(([s, r, g, f]) => {
        if (!vivo) return;
        sorgenti = s;
        regole = r;
        suggerite = g;
        contesti = f;
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

  /** Le suggerite che non sono già fra le sorgenti attive. */
  const daProporre = $derived(
    suggerite.filter(([p]) => !sorgenti.some((s) => s.path === p)),
  );

  async function scegliCartella(titolo: string): Promise<string | null> {
    const scelta = await open({ directory: true, multiple: false, title: titolo });
    return typeof scelta === "string" ? scelta : null;
  }

  async function aggiungiSorgente(path?: string, fascia?: Fascia) {
    aggiungendo = true;
    try {
      const p = path ?? (await scegliCartella("Scegli la cartella da indicizzare"));
      if (!p) return;
      await sorgenteAggiungi(p, fascia ?? fasciaNuova);
      errore = null;
      ricarica += 1;
    } catch (e) {
      errore = messaggioErrore(e);
    } finally {
      aggiungendo = false;
    }
  }

  async function aggiorna(s: Sorgente, modifica: Partial<Sorgente>) {
    try {
      await sorgenteAggiorna({ ...s, ...modifica });
      errore = null;
      ricarica += 1;
    } catch (e) {
      errore = messaggioErrore(e);
    }
  }

  async function rimuovi(id: number) {
    try {
      await sorgenteRimuovi(id);
      ricarica += 1;
    } catch (e) {
      errore = messaggioErrore(e);
    }
  }

  async function commutaRegola(r: Regola, attiva: boolean) {
    try {
      await regolaAttiva(r.id, attiva);
      ricarica += 1;
    } catch (e) {
      errore = messaggioErrore(e);
    }
  }

  async function eliminaRegola(id: number) {
    try {
      await regolaElimina(id);
      ricarica += 1;
    } catch (e) {
      errore = messaggioErrore(e);
    }
  }

  function commutaContesto(c: string) {
    contestiScelti = contestiScelti.includes(c)
      ? contestiScelti.filter((x) => x !== c)
      : [...contestiScelti, c];
  }

  async function scegliDestinazione() {
    try {
      const p = await scegliCartella("Scegli dove organizzare i file");
      if (p) destinazione = p;
    } catch (e) {
      errore = messaggioErrore(e);
    }
  }

  async function preparaOrganizza() {
    if (!destinazione.trim()) return;
    inCorso = true;
    esito = null;
    try {
      piano = await organizzaPiano(destinazione.trim(), [...contestiScelti]);
      errore = null;
    } catch (e) {
      errore = messaggioErrore(e);
    } finally {
      inCorso = false;
    }
  }

  async function eseguiOrganizza() {
    if (!piano) return;
    inCorso = true;
    try {
      esito = await operazioniEsegui(piano);
      ricarica += 1;
    } catch (e) {
      errore = messaggioErrore(e);
    } finally {
      inCorso = false;
    }
  }

  const regoleBuiltin = $derived(regole.filter((r) => r.builtin));
  const regoleMie = $derived(regole.filter((r) => !r.builtin));
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

  <!-- Sorgenti ---------------------------------------------------------- -->
  <Card
    titolo="Sorgenti"
    sottotitolo="Le cartelle che Setaccio percorre a ogni scansione"
  >
    {#snippet azioni()}
      <div class="riga">
        <div class="fasce" role="group" aria-label="Fascia della cartella da aggiungere">
          {#each FASCE as f (f)}
            <button
              class="fascia"
              class:acceso={fasciaNuova === f}
              aria-pressed={fasciaNuova === f}
              onclick={() => (fasciaNuova = f)}
            >
              {f}
            </button>
          {/each}
        </div>
        <Bottone
          variante="secondario"
          icona="piu"
          caricamento={aggiungendo}
          onclick={() => aggiungiSorgente()}
        >
          Aggiungi cartella
        </Bottone>
      </div>
    {/snippet}

    <div class="impila">
      <div class="due-note">
        <div class="nota-fascia">
          <span class="pallino documento" aria-hidden="true"></span>
          <div>
            <p class="nota-titolo">Fascia documenti</p>
            <p class="nota-testo">
              Cartelle di roba scritta: PDF, testi, fogli, presentazioni. Da qui
              Setaccio estrae il testo per la ricerca full-text e tiene fuori
              gli artefatti di build.
            </p>
          </div>
        </div>
        <div class="nota-fascia">
          <span class="pallino tracciato" aria-hidden="true"></span>
          <div>
            <p class="nota-titolo">Fascia tracciati</p>
            <p class="nota-testo">
              Cartelle di lavorazione: file a record fissi e i documenti che ne
              derivano. Qui Setaccio indicizza anche i record e ricostruisce i
              lotti di composizione.
            </p>
          </div>
        </div>
      </div>

      {#if sorgenti.length === 0 && !caricando}
        <Vuoto
          icona="cartella"
          titolo="Nessuna cartella da indicizzare"
          messaggio="Scegli almeno una cartella: Setaccio non tocca il resto del disco. Qui sotto ci sono quelle che ha trovato guardando le tue directory più abitate."
        />
      {:else}
        <div class="elenco">
          {#each sorgenti as s (s.id)}
            <div class="sorgente" class:spenta={!s.attiva}>
              <div class="capo">
                <span class="tile-sorgente {s.fascia}">
                  <Icona nome={s.fascia === "tracciati" ? "tracciati" : "documento"} dimensione={17} />
                </span>
                <div class="crescente">
                  <p class="path-sorgente mono troncato" title={s.path}>
                    {accorciaPath(s.path, 72)}
                  </p>
                  <div class="badge-riga">
                    <Badge testo={s.fascia} variante={s.fascia === "tracciati" ? "info" : "neutro"} />
                    {#if !s.attiva}<Badge testo="disattivata" variante="avviso" />{/if}
                  </div>
                </div>
                <Bottone
                  variante="fantasma"
                  dimensione="sm"
                  icona="cestino"
                  soloIcona
                  titolo="Rimuovi la sorgente"
                  onclick={() => rimuovi(s.id)}
                />
              </div>

              <div class="interruttori">
                <Interruttore
                  dimensione="sm"
                  attivo={s.attiva}
                  etichetta="Attiva"
                  descrizione="La scansione la percorre"
                  onchange={(v) => aggiorna(s, { attiva: v })}
                />
                <Interruttore
                  dimensione="sm"
                  attivo={s.ricorsiva}
                  etichetta="Ricorsiva"
                  descrizione="Scende anche nelle sottocartelle"
                  onchange={(v) => aggiorna(s, { ricorsiva: v })}
                />
                <Interruttore
                  dimensione="sm"
                  attivo={s.ignora_repo_guard}
                  etichetta="Scavalca il repo-guard"
                  descrizione="Indicizza anche dentro i repository di codice"
                  onchange={(v) => aggiorna(s, { ignora_repo_guard: v })}
                />
              </div>
            </div>
          {/each}
        </div>
      {/if}

      <div class="repo-guard">
        <span class="repo-icona"><Icona nome="artefatto" dimensione={18} /></span>
        <div>
          <p class="nota-titolo">Perché esiste il repo-guard</p>
          <p class="nota-testo">
            Se una cartella contiene un <span class="mono">.git</span>, Setaccio
            si ferma: dentro un repository ci sono migliaia di file generati che
            renderebbero la ricerca inutilizzabile. L'interruttore
            <strong>Scavalca il repo-guard</strong> serve al caso opposto — una
            cartella di documenti veri che per ragioni storiche vive dentro un
            repository. Accendilo solo lì, non ovunque.
          </p>
        </div>
      </div>

      {#if daProporre.length > 0}
        <div class="suggerite">
          <p class="nota-titolo">Cartelle che potrebbero interessarti</p>
          <div class="chip-riga">
            {#each daProporre as [path, fascia] (path)}
              <button
                class="chip"
                title="Aggiungi {path} in fascia {fascia}"
                onclick={() =>
                  aggiungiSorgente(path, fascia === "tracciati" ? "tracciati" : "documenti")}
              >
                <Icona nome="piu" dimensione={13} />
                <span class="mono troncato">{accorciaPath(path, 44)}</span>
                <span class="fascia-chip">{fascia}</span>
              </button>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  </Card>

  <!-- Regole ------------------------------------------------------------ -->
  <Card
    titolo="Regole di categorizzazione"
    sottotitolo="Valutate dalla priorità più bassa alla più alta; la prima che aggancia vince"
    padding="nessuna"
  >
    <div class="elenco-regole">
      {#if regoleMie.length > 0}
        <p class="sezione-regole">Le tue regole</p>
        {#each regoleMie as r (r.id)}
          <div class="regola" class:spenta={!r.attiva}>
            <Interruttore
              dimensione="sm"
              attivo={r.attiva}
              titolo="Attiva la regola {r.nome}"
              onchange={(v) => commutaRegola(r, v)}
            />
            <div class="crescente">
              <p class="nome-regola troncato">{r.nome}</p>
              <p class="corpo-regola">
                <span class="mono">{r.pattern}</span>
                <Icona nome="freccia" dimensione={12} />
                <span class="valore-regola">{r.valore}</span>
              </p>
            </div>
            <Badge testo={r.asse} variante="neutro" />
            <span class="priorita cifre" title="Priorità: più bassa = valutata prima">
              {r.priorita}
            </span>
            <Bottone
              variante="fantasma"
              dimensione="sm"
              icona="cestino"
              soloIcona
              titolo="Elimina la regola {r.nome}"
              onclick={() => eliminaRegola(r.id)}
            />
          </div>
        {/each}
      {/if}

      <p class="sezione-regole">
        Regole di serie
        <span class="testo-tenue">— disattivabili, non cancellabili</span>
      </p>
      {#each regoleBuiltin as r (r.id)}
        <div class="regola" class:spenta={!r.attiva}>
          <Interruttore
            dimensione="sm"
            attivo={r.attiva}
            titolo="Attiva la regola {r.nome}"
            onchange={(v) => commutaRegola(r, v)}
          />
          <div class="crescente">
            <p class="nome-regola troncato">{r.nome}</p>
            <p class="corpo-regola">
              <span class="mono">{r.pattern}</span>
              <Icona nome="freccia" dimensione={12} />
              <span class="valore-regola">{r.valore}</span>
            </p>
          </div>
          <Badge testo="di serie" variante="info" />
          <span class="priorita cifre" title="Priorità: più bassa = valutata prima">
            {r.priorita}
          </span>
          <span class="posto-bottone" aria-hidden="true"></span>
        </div>
      {/each}

      {#if regole.length === 0 && !caricando}
        <Vuoto
          compatto
          icona="filtro"
          titolo="Nessuna regola caricata"
          messaggio="Le regole di serie arrivano con il database: se qui non c'è niente, la prima scansione non è ancora avvenuta."
        />
      {/if}
    </div>
  </Card>

  <!-- Organizza --------------------------------------------------------- -->
  <Card
    titolo="Organizza"
    sottotitolo="Sposta i file in cartelle per contesto — solo se lo chiedi tu"
  >
    <div class="impila">
      <div class="repo-guard">
        <span class="repo-icona neutro"><Icona nome="info" dimensione={18} /></span>
        <div>
          <p class="nota-titolo">Di default Setaccio non sposta niente</p>
          <p class="nota-testo">
            Indicizzare e riordinare sono due cose diverse: la prima è
            innocua, la seconda cambia il disco. Qui puoi chiedere di
            raccogliere i file di uno o più contesti sotto una cartella di
            destinazione. Vedrai comunque il piano prima che succeda qualcosa, e
            l'operazione resta annullabile dai batch nella sezione Duplicati.
          </p>
        </div>
      </div>

      <div class="destinazione">
        <Campo
          bind:valore={destinazione}
          etichetta="Cartella di destinazione"
          segnaposto="/home/…/Archivio"
          icona="cartella"
          azzerabile
          autocomplete="off"
          spellcheck={false}
        />
        <Bottone variante="secondario" icona="cartella" onclick={scegliDestinazione}>
          Scegli…
        </Bottone>
      </div>

      <div class="scelta-contesti">
        <p class="nota-titolo">
          Contesti da spostare
          <span class="testo-tenue">
            — nessuno selezionato significa «tutti quelli che hanno un contesto»
          </span>
        </p>
        {#if contesti.length === 0}
          <p class="nota-testo">
            Non c'è ancora nessun contesto: assegnali dalla sezione Revisione.
          </p>
        {:else}
          <div class="chip-riga">
            {#each contesti as c (c.etichetta)}
              <button
                class="chip"
                class:acceso={contestiScelti.includes(c.etichetta)}
                aria-pressed={contestiScelti.includes(c.etichetta)}
                onclick={() => commutaContesto(c.etichetta)}
              >
                {c.etichetta}
                <span class="conta cifre">{formattaNumero(c.quanti)}</span>
              </button>
            {/each}
          </div>
        {/if}
      </div>

      <div class="riga-azione">
        <Bottone
          variante="secondario"
          icona="lotti"
          disabled={!destinazione.trim()}
          caricamento={inCorso && !piano}
          onclick={preparaOrganizza}
        >
          Anteprima del piano
        </Bottone>
      </div>

      {#if piano}
        <RiepilogoPiano
          {piano}
          {esito}
          {inCorso}
          testoConferma="Sposta i file"
          spiegazione="Le mosse spostano i file sotto la cartella di destinazione, una sottocartella per contesto. Niente viene sovrascritto: se a destinazione c'è già un file con quel nome la mossa viene saltata e te lo diciamo."
          onconferma={eseguiOrganizza}
          onchiudi={() => {
            piano = null;
            esito = null;
          }}
        />
      {/if}
    </div>
  </Card>
</div>

<style>
  /* Sorgenti ------------------------------------------------------------ */
  .fasce {
    display: inline-flex;
    gap: 2px;
    padding: 3px;
    border-radius: var(--raggio-pillola);
    background: var(--superficie-2);
    border: 1px solid var(--bordo);
  }

  .fascia {
    height: 28px;
    padding: 0 var(--sp-3);
    border-radius: var(--raggio-pillola);
    font-size: var(--minuto);
    font-weight: var(--peso-forte);
    color: var(--testo-2);
  }

  .fascia.acceso {
    background: var(--superficie);
    color: var(--testo);
    box-shadow: var(--ombra-1);
  }

  .due-note {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--sp-3);
  }

  @media (max-width: 820px) {
    .due-note {
      grid-template-columns: minmax(0, 1fr);
    }
  }

  .nota-fascia {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
    padding: var(--sp-3) var(--sp-4);
    border-radius: var(--raggio-lg);
    background: var(--superficie-2);
  }

  .pallino {
    width: 10px;
    height: 10px;
    margin-top: 5px;
    border-radius: 50%;
    flex: 0 0 auto;
  }

  .pallino.documento {
    background: var(--documento);
  }

  .pallino.tracciato {
    background: var(--tracciato);
  }

  .nota-titolo {
    font-size: var(--piccolo);
    font-weight: var(--peso-grasso);
    color: var(--testo);
  }

  .nota-testo {
    margin-top: 2px;
    font-size: var(--minuto);
    color: var(--testo-2);
    line-height: var(--riga-larga);
  }

  .elenco {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .sorgente {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding: var(--sp-4);
    border-radius: var(--raggio-lg);
    background: var(--superficie-2);
    min-width: 0;
  }

  .sorgente.spenta {
    opacity: 0.6;
  }

  .capo {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-width: 0;
  }

  .tile-sorgente {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    flex: 0 0 auto;
    border-radius: var(--raggio);
    background: var(--documento-bg);
    color: var(--documento);
  }

  .tile-sorgente.tracciati {
    background: var(--tracciato-bg);
    color: var(--tracciato);
  }

  .path-sorgente {
    font-size: var(--piccolo);
    font-weight: var(--peso-forte);
    color: var(--testo);
  }

  .badge-riga {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-1);
    margin-top: 3px;
  }

  .interruttori {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--sp-3);
    padding-top: var(--sp-3);
    border-top: 1px solid var(--bordo);
  }

  @media (max-width: 900px) {
    .interruttori {
      grid-template-columns: minmax(0, 1fr);
    }
  }

  .repo-guard {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
    padding: var(--sp-4);
    border-radius: var(--raggio-lg);
    background: var(--avviso-bg);
  }

  .repo-icona {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    flex: 0 0 auto;
    border-radius: var(--raggio);
    background: var(--superficie);
    color: var(--avviso);
  }

  .repo-icona.neutro {
    color: var(--info);
  }

  .suggerite {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .chip-riga {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    max-width: 100%;
    height: 30px;
    padding: 0 var(--sp-3);
    border-radius: var(--raggio-pillola);
    border: 1px solid var(--bordo);
    background: var(--superficie-2);
    color: var(--testo-2);
    font-size: var(--minuto);
    font-weight: var(--peso-medio);
    transition:
      background var(--transizione),
      color var(--transizione),
      border-color var(--transizione);
  }

  .chip:hover {
    border-color: var(--accento-bordo);
    color: var(--testo);
  }

  .chip.acceso {
    background: var(--accento-tenue);
    border-color: var(--accento-bordo);
    color: var(--accento-testo);
    font-weight: var(--peso-forte);
  }

  .fascia-chip,
  .conta {
    flex: 0 0 auto;
    font-size: var(--micro);
    color: var(--testo-3);
  }

  .chip.acceso .conta {
    color: inherit;
  }

  /* Regole -------------------------------------------------------------- */
  .elenco-regole {
    display: flex;
    flex-direction: column;
    padding: 0 var(--sp-4) var(--sp-3);
  }

  .sezione-regole {
    padding: var(--sp-4) 0 var(--sp-2);
    font-size: var(--micro);
    font-weight: var(--peso-forte);
    color: var(--testo-3);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .sezione-regole span {
    font-weight: var(--peso-normale);
    text-transform: none;
    letter-spacing: 0;
  }

  .regola {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-3) 0;
    border-bottom: 1px solid var(--bordo);
    min-width: 0;
  }

  .regola:last-child {
    border-bottom: none;
  }

  .regola.spenta {
    opacity: 0.55;
  }

  .nome-regola {
    font-size: var(--piccolo);
    font-weight: var(--peso-forte);
  }

  .corpo-regola {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    margin-top: 2px;
    font-size: var(--micro);
    color: var(--testo-2);
    min-width: 0;
    flex-wrap: wrap;
  }

  .valore-regola {
    color: var(--accento-testo);
    font-weight: var(--peso-forte);
  }

  .priorita {
    flex: 0 0 auto;
    width: 34px;
    text-align: right;
    font-size: var(--minuto);
    color: var(--testo-3);
  }

  .posto-bottone {
    width: 30px;
    flex: 0 0 auto;
  }

  /* Organizza ------------------------------------------------------------ */
  .destinazione {
    display: flex;
    align-items: flex-end;
    gap: var(--sp-2);
  }

  .destinazione :global(.campo) {
    flex: 1 1 auto;
  }

  .scelta-contesti {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .riga-azione {
    display: flex;
    justify-content: flex-end;
  }

  .allarme {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    color: var(--pericolo);
    font-size: var(--piccolo);
  }
</style>
