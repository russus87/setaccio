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
    cartelleVuotePiano,
    faccette,
    FASCE,
    formattaByte,
    formattaNumero,
    operazioniEsegui,
    organizzaDestinazione,
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
  import Card from "../ui/Card.svelte";
  import Icona from "../ui/Icona.svelte";
  import Interruttore from "../ui/Interruttore.svelte";
  import Vuoto from "../ui/Vuoto.svelte";
  import AlberoContesti from "./AlberoContesti.svelte";
  import FormRegola from "./FormRegola.svelte";
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
  let formRegolaAperto = $state(false);

  // ---- Organizza ---------------------------------------------------------
  /** Quella proposta dal backend: la sorgente che contiene già quei file. */
  let destinazionePredefinita = $state<string | null>(null);
  /** Solo se l'utente insiste e ne sceglie un'altra a mano. */
  let destinazioneManuale = $state<string | null>(null);
  let cercandoDestinazione = $state(false);
  let contestiScelti = $state<string[]>([]);
  let piano = $state<PianoOperazioni | null>(null);
  let esito = $state<EsitoOperazioni | null>(null);
  let inCorso = $state(false);

  const destinazione = $derived(destinazioneManuale ?? destinazionePredefinita ?? "");

  // ---- Pulizia delle cartelle vuote --------------------------------------
  /** `null` = tutte le sorgenti attive più la quarantena, il caso normale. */
  let radicePulizia = $state<string | null>(null);
  let pianoPulizia = $state<PianoOperazioni | null>(null);
  let esitoPulizia = $state<EsitoOperazioni | null>(null);
  let inCorsoPulizia = $state(false);

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

  // La destinazione la propone il backend, e cambia con i contesti scelti:
  // quasi sempre è la sorgente dove quei file stanno già, e cercarla a mano
  // sarebbe far fare all'utente un lavoro che il database sa fare da solo.
  $effect(() => {
    const scelti = [...contestiScelti];
    void ricarica;
    let vivo = true;
    cercandoDestinazione = true;
    organizzaDestinazione(scelti)
      .then((d) => {
        if (vivo) destinazionePredefinita = d;
      })
      .catch((e) => {
        if (vivo) errore = messaggioErrore(e);
      })
      .finally(() => {
        if (vivo) cercandoDestinazione = false;
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
      if (p) destinazioneManuale = p;
    } catch (e) {
      errore = messaggioErrore(e);
    }
  }

  /** I contesti scelti, con i conteggi delle faccette accanto. */
  const vociScelte = $derived(
    contesti.filter((c) => contestiScelti.includes(c.etichetta)),
  );

  const fileScelti = $derived(vociScelte.reduce((s, v) => s + v.quanti, 0));

  async function preparaOrganizza() {
    if (contestiScelti.length === 0) return;
    inCorso = true;
    esito = null;
    try {
      // Con la destinazione vuota è il backend a dedurla, con la stessa
      // logica del suggerimento: non serve inventarne una qui.
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

  async function cercaCartelleVuote() {
    inCorsoPulizia = true;
    esitoPulizia = null;
    try {
      pianoPulizia = await cartelleVuotePiano(radicePulizia ? [radicePulizia] : []);
      errore = null;
    } catch (e) {
      errore = messaggioErrore(e);
    } finally {
      inCorsoPulizia = false;
    }
  }

  async function eseguiPulizia() {
    if (!pianoPulizia) return;
    inCorsoPulizia = true;
    try {
      esitoPulizia = await operazioniEsegui(pianoPulizia);
      ricarica += 1;
    } catch (e) {
      errore = messaggioErrore(e);
    } finally {
      inCorsoPulizia = false;
    }
  }

  const sorgentiAttive = $derived(sorgenti.filter((s) => s.attiva));

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
    {#snippet azioni()}
      <Bottone
        variante={formRegolaAperto ? "fantasma" : "secondario"}
        icona={formRegolaAperto ? "chiudi" : "piu"}
        onclick={() => (formRegolaAperto = !formRegolaAperto)}
      >
        {formRegolaAperto ? "Chiudi il form" : "Nuova regola"}
      </Bottone>
    {/snippet}

    {#if formRegolaAperto}
      <div class="zona-form">
        <FormRegola
          {contesti}
          {regole}
          onsalvata={() => {
            ricarica += 1;
          }}
          onchiudi={() => (formRegolaAperto = false)}
        />
      </div>
    {/if}

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
            innocua, la seconda cambia il disco. Qui scegli <em>quali</em>
            contesti raccogliere; la cartella di destinazione te la propone
            Setaccio, ed è quella dove quei file già stanno. Vedrai comunque il
            piano prima che succeda qualcosa, e l'operazione resta annullabile
            dai batch nella sezione Duplicati.
          </p>
        </div>
      </div>

      <div class="scelta-contesti">
        <div class="capo-contesti">
          <p class="nota-titolo">
            Contesti da spostare
            <span class="testo-tenue">— scegli cosa organizzare, il resto resta dov'è</span>
          </p>
          {#if contesti.length > 0}
            <div class="riga">
              <Bottone
                variante="fantasma"
                dimensione="sm"
                onclick={() => (contestiScelti = contesti.map((c) => c.etichetta))}
              >
                Tutti
              </Bottone>
              <Bottone
                variante="fantasma"
                dimensione="sm"
                disabled={contestiScelti.length === 0}
                onclick={() => (contestiScelti = [])}
              >
                Nessuno
              </Bottone>
            </div>
          {/if}
        </div>

        {#if contesti.length === 0}
          <p class="nota-testo">
            Non c'è ancora nessun contesto: assegnali dalla sezione Revisione, o
            scrivi una regola qui sopra.
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
                <span class="conta cifre">
                  {formattaNumero(c.quanti)} · {formattaByte(c.byte)}
                </span>
              </button>
            {/each}
          </div>
        {/if}
      </div>

      <div class="destinazione">
        <span class="tile-dest">
          <Icona nome="cartella" dimensione={17} />
        </span>
        <div class="crescente">
          <p class="etichetta-dest">Destinazione</p>
          <p class="path-dest mono troncato" title={destinazione}>
            {#if destinazione}
              {accorciaPath(destinazione, 62)}
            {:else if cercandoDestinazione}
              sto guardando dove stanno già questi file…
            {:else}
              nessuna cartella da proporre
            {/if}
          </p>
          <p class="nota-testo">
            {#if destinazioneManuale}
              L'hai scelta tu.
            {:else if destinazionePredefinita}
              Proposta da Setaccio: è la sorgente che contiene già la maggior
              parte dei file di questi contesti, quindi quasi sempre è dove li
              vuoi.
            {:else}
              L'indice non ha ancora niente da cui dedurla: scegli una cartella
              a mano.
            {/if}
          </p>
        </div>
        <div class="azioni-dest">
          <Bottone
            variante="fantasma"
            dimensione="sm"
            icona="cartella"
            onclick={scegliDestinazione}
          >
            Cambia cartella…
          </Bottone>
          {#if destinazioneManuale}
            <Bottone
              variante="fantasma"
              dimensione="sm"
              icona="aggiorna"
              onclick={() => (destinazioneManuale = null)}
            >
              Ripristina la predefinita
            </Bottone>
          {/if}
        </div>
      </div>

      {#if vociScelte.length > 0}
        <AlberoContesti radice={destinazione} voci={vociScelte} />
      {/if}

      <div class="riga-azione">
        {#if contestiScelti.length === 0}
          <span class="testo-piccolo testo-tenue">
            Scegli almeno un contesto: senza selezione non c'è niente da spostare.
          </span>
        {:else}
          <span class="testo-piccolo testo-tenue">
            {formattaNumero(contestiScelti.length)}
            {contestiScelti.length === 1 ? "contesto" : "contesti"},
            {formattaNumero(fileScelti)} file da vagliare.
          </span>
        {/if}
        <Bottone
          variante="secondario"
          icona="lotti"
          disabled={contestiScelti.length === 0}
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
          spiegazione="Le mosse spostano i file sotto la cartella di destinazione, una cartella per contesto: se il contesto ha una barra — «lavoro/PAM» — diventano cartelle annidate. Niente viene sovrascritto: se a destinazione c'è già un file con quel nome la mossa viene saltata e te lo diciamo."
          onconferma={eseguiOrganizza}
          onchiudi={() => {
            piano = null;
            esito = null;
          }}
        />
      {/if}
    </div>
  </Card>

  <!-- Pulizia ----------------------------------------------------------- -->
  <Card
    titolo="Pulizia"
    sottotitolo="Toglie le cartelle rimaste vuote dopo aver spostato o messo in quarantena"
  >
    <div class="impila">
      <div class="repo-guard">
        <span class="repo-icona"><Icona nome="avviso" dimensione={18} /></span>
        <div>
          <p class="nota-titolo">L'unica cosa che Setaccio cancella davvero</p>
          <p class="nota-testo">
            Qui non si spostano file: si tolgono <strong>contenitori vuoti</strong>,
            e solo quelli. La rimozione usa <span class="mono">remove_dir</span>,
            mai <span class="mono">remove_dir_all</span>: se dentro è rimasto
            qualcosa è il sistema operativo a rifiutare, quindi la garanzia non
            dipende da un controllo nostro che potrebbe sbagliare.
          </p>
          <ul class="elenco-nota">
            <li>
              <span class="pallino-nota" aria-hidden="true"></span>
              <span>
                <strong>Scende a cascata</strong>: una cartella che conteneva
                solo cartelle vuote resta vuota a sua volta e viene tolta anche
                lei, nella stessa passata.
              </span>
            </li>
            <li>
              <span class="pallino-nota" aria-hidden="true"></span>
              <span>
                <strong>Le radici delle sorgenti non si toccano mai</strong>:
                svuotare <span class="mono">~/Scaricati</span> non fa sparire
                <span class="mono">~/Scaricati</span>.
              </span>
            </li>
            <li>
              <span class="pallino-nota" aria-hidden="true"></span>
              <span>
                <strong>È annullabile</strong> come tutto il resto: l'annulla
                del batch ricrea le cartelle, dall'elenco nella sezione
                Duplicati.
              </span>
            </li>
            <li>
              <span class="pallino-nota" aria-hidden="true"></span>
              <span>
                Prima vedi l'elenco completo, poi confermi: niente sparisce
                senza che tu l'abbia letto.
              </span>
            </li>
          </ul>
        </div>
      </div>

      <div class="scelta-contesti">
        <p class="nota-titolo">
          Dove guardare
          <span class="testo-tenue">— di norma ovunque Setaccio abbia messo mano</span>
        </p>
        <div class="chip-riga">
          <button
            class="chip"
            class:acceso={radicePulizia === null}
            aria-pressed={radicePulizia === null}
            onclick={() => {
              radicePulizia = null;
              pianoPulizia = null;
              esitoPulizia = null;
            }}
          >
            Tutte le sorgenti attive + quarantena
          </button>
          {#each sorgentiAttive as s (s.id)}
            <button
              class="chip"
              class:acceso={radicePulizia === s.path}
              aria-pressed={radicePulizia === s.path}
              title={s.path}
              onclick={() => {
                radicePulizia = s.path;
                pianoPulizia = null;
                esitoPulizia = null;
              }}
            >
              <span class="mono troncato">{accorciaPath(s.path, 34)}</span>
            </button>
          {/each}
        </div>
      </div>

      <div class="riga-azione">
        <Bottone
          variante="secondario"
          icona="cestino"
          caricamento={inCorsoPulizia && !pianoPulizia}
          onclick={cercaCartelleVuote}
        >
          Cerca le cartelle vuote
        </Bottone>
      </div>

      {#if pianoPulizia && pianoPulizia.mosse.length === 0}
        <Vuoto
          compatto
          icona="check"
          titolo="Non c'è niente da pulire"
          messaggio="Sotto le cartelle guardate non è rimasto nessun contenitore vuoto: l'albero è già in ordine."
          testoAzione="Controlla di nuovo"
          iconaAzione="aggiorna"
          onazione={cercaCartelleVuote}
        />
      {:else if pianoPulizia}
        <RiepilogoPiano
          piano={pianoPulizia}
          esito={esitoPulizia}
          inCorso={inCorsoPulizia}
          mostraSpazio={false}
          testoConferma="Rimuovi le cartelle vuote"
          spiegazione="Vengono tolte solo le cartelle vuote qui elencate, e nessun file: se una di queste nel frattempo si riempie, il sistema operativo rifiuta la rimozione e la mossa risulta fallita. Il batch resta annullabile e l'annulla ricrea le cartelle."
          onconferma={eseguiPulizia}
          onchiudi={() => {
            pianoPulizia = null;
            esitoPulizia = null;
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

  .elenco-nota {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    margin-top: var(--sp-2);
  }

  .elenco-nota li {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-2);
    font-size: var(--minuto);
    color: var(--testo-2);
    line-height: var(--riga-larga);
  }

  .pallino-nota {
    width: 4px;
    height: 4px;
    margin-top: 9px;
    flex: 0 0 auto;
    border-radius: 50%;
    background: var(--testo-3);
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

  .zona-form {
    padding: 0 var(--sp-4);
  }

  /* Organizza ------------------------------------------------------------ */
  .destinazione {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
    padding: var(--sp-4);
    border-radius: var(--raggio-lg);
    background: var(--superficie-2);
    min-width: 0;
  }

  .tile-dest {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    flex: 0 0 auto;
    border-radius: var(--raggio);
    background: var(--superficie);
    color: var(--accento-testo);
  }

  .etichetta-dest {
    font-size: var(--micro);
    font-weight: var(--peso-forte);
    color: var(--testo-3);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .path-dest {
    margin-top: 2px;
    font-size: var(--piccolo);
    font-weight: var(--peso-forte);
    color: var(--testo);
  }

  .azioni-dest {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: var(--sp-1);
    flex: 0 0 auto;
  }

  .scelta-contesti {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .capo-contesti {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-3);
    flex-wrap: wrap;
  }

  .riga-azione {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--sp-3);
    flex-wrap: wrap;
  }

  @media (max-width: 820px) {
    .destinazione {
      flex-wrap: wrap;
    }

    .azioni-dest {
      align-items: flex-start;
      flex-direction: row;
      flex-wrap: wrap;
    }
  }

  .allarme {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    color: var(--pericolo);
    font-size: var(--piccolo);
  }
</style>
