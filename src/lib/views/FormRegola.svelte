<script lang="ts">
  /**
   * Il form per scrivere una regola a mano.
   *
   * Fino a ieri una regola nasceva solo dalla coda di revisione, cliccando un
   * pattern già proposto. Qui la si scrive da zero, e allora il compito del
   * form è dire prima le due cose che si sbagliano sempre: che un pattern con
   * la barra si confronta col percorso e non col nome, e che il contesto ora è
   * gerarchico (`lavoro/PAM`), perché quella barra diventerà una cartella
   * dentro l'altra quando si organizza.
   */
  import {
    accorciaPath,
    formattaNumero,
    regolaAggiungi,
    regolaStima,
    TIPI,
    type ConteggioEtichetta,
    type Regola,
  } from "../api";
  import Bottone from "../ui/Bottone.svelte";
  import Campo from "../ui/Campo.svelte";
  import Icona from "../ui/Icona.svelte";
  import { messaggioErrore } from "./comuni";

  interface Props {
    /** I contesti già in uso, per proporli come completamento. */
    contesti: ConteggioEtichetta[];
    /** Le regole esistenti, per accorgersi dei doppioni. */
    regole: Regola[];
    /** Chiamato dopo un salvataggio riuscito: la lista va riletta. */
    onsalvata: () => void;
    /** Chiamato quando si chiude il form senza salvare. */
    onchiudi: () => void;
  }

  let { contesti, regole, onsalvata, onchiudi }: Props = $props();

  /** Le builtin stanno fra 10 e 30: una regola scritta a mano parte dopo. */
  const PRIORITA_PREDEFINITA = "50";
  /** Quanto si aspetta, dopo l'ultimo tasto, prima di chiedere il conteggio. */
  const ATTESA_CONTEGGIO = 350;

  let nome = $state("");
  let asse = $state<"contesto" | "tipo">("contesto");
  let pattern = $state("");
  let valore = $state("");
  let priorita = $state(PRIORITA_PREDEFINITA);
  let salvando = $state(false);
  let errore = $state<string | null>(null);
  let provato = $state(false);

  /** Conteggio esatto e percorsi di esempio, come li dà il backend. */
  let colpiti = $state<{ quanti: number; esempi: string[] } | null>(null);
  let contando = $state(false);
  /**
   * Il messaggio del backend quando il pattern non è compilabile. È anche la
   * validazione del campo: chi sa se un glob è valido è il motore che lo usa.
   */
  let errorePattern = $state<string | null>(null);

  const idContesti = $props.id();

  /**
   * La stessa scelta che fa `combacia()` in Rust. Qui serve solo a scrivere la
   * frase giusta sotto il campo: a contare e a decidere è il backend.
   */
  const sulPath = $derived(pattern.includes("/") || pattern.includes("**"));

  const doppione = $derived(
    regole.find((r) => r.pattern === pattern.trim() && r.asse === asse) ?? null,
  );

  const prioritaNumero = $derived(Number.parseInt(priorita, 10));
  const prioritaValida = $derived(
    Number.isFinite(prioritaNumero) && prioritaNumero >= 0 && prioritaNumero <= 9999,
  );

  const valoreValido = $derived(
    asse === "tipo"
      ? (TIPI as readonly string[]).includes(valore.trim())
      : valore.trim().length > 0,
  );

  const completo = $derived(
    nome.trim().length > 0 &&
      pattern.trim().length > 0 &&
      valoreValido &&
      prioritaValida &&
      errorePattern === null,
  );

  /* ---- Quanti file colpisce -------------------------------------------- */

  // Il conteggio segue quello che si sta scrivendo, con una pausa perché non
  // parta una query a ogni tasto.
  $effect(() => {
    const p = pattern.trim();
    colpiti = null;
    if (!p) {
      errorePattern = null;
      contando = false;
      return;
    }
    contando = true;
    let vivo = true;
    const attesa = setTimeout(() => {
      regolaStima(p)
        .then(([quanti, esempi]) => {
          if (!vivo) return;
          colpiti = { quanti, esempi };
          errorePattern = null;
        })
        .catch((e) => {
          if (!vivo) return;
          colpiti = null;
          errorePattern = messaggioErrore(e);
        })
        .finally(() => {
          if (vivo) contando = false;
        });
    }, ATTESA_CONTEGGIO);
    return () => {
      vivo = false;
      clearTimeout(attesa);
    };
  });

  /* ---- Salvataggio ------------------------------------------------------ */

  async function salva() {
    // Prima si accende la segnalazione, poi si controlla: un bottone che non
    // fa niente e non dice perché è la cosa peggiore di un form.
    provato = true;
    if (!completo || salvando) return;
    salvando = true;
    try {
      // Chi salva prima che la pausa sia scaduta non deve poter infilare un
      // pattern che il motore scarterebbe: si controlla una volta di più.
      if (!colpiti) {
        try {
          const [quanti, esempi] = await regolaStima(pattern.trim());
          colpiti = { quanti, esempi };
          errorePattern = null;
        } catch (e) {
          errorePattern = messaggioErrore(e);
          return;
        }
      }
      await regolaAggiungi(
        nome.trim(),
        asse,
        pattern.trim(),
        valore.trim(),
        prioritaNumero,
      );
      errore = null;
      azzera();
      onsalvata();
    } catch (e) {
      errore = messaggioErrore(e);
    } finally {
      salvando = false;
    }
  }

  function azzera() {
    nome = "";
    pattern = "";
    valore = "";
    priorita = PRIORITA_PREDEFINITA;
    provato = false;
  }

  function scegliAsse(a: "contesto" | "tipo") {
    if (asse === a) return;
    asse = a;
    valore = "";
  }
</script>

<div class="form">
  <div class="regola-match">
    <span class="icona-nota"><Icona nome="info" dimensione={17} /></span>
    <p>
      Come combacia un pattern: se contiene <span class="mono">/</span> oppure
      <span class="mono">**</span> viene confrontato con il
      <strong>percorso completo</strong> del file, altrimenti con il
      <strong>solo nome</strong>. Perciò <span class="mono">CV_*</span> prende i
      file che si chiamano così, mentre per prendere tutto quello che sta in una
      cartella serve <span class="mono">**/libri/**</span>.
    </p>
  </div>

  <div class="griglia-form">
    <Campo
      bind:valore={nome}
      etichetta="Nome della regola"
      segnaposto="Fatture del commercialista"
      descrizione="Serve solo a te, per ritrovarla in questo elenco."
      errore={provato && !nome.trim() ? "Il nome non può restare vuoto." : undefined}
      autocomplete="off"
    />

    <div class="campo-asse">
      <span class="etichetta-campo" id="asse-{idContesti}">Asse</span>
      <div class="fasce" role="group" aria-labelledby="asse-{idContesti}">
        <button
          class="fascia"
          class:acceso={asse === "contesto"}
          aria-pressed={asse === "contesto"}
          onclick={() => scegliAsse("contesto")}
        >
          contesto
        </button>
        <button
          class="fascia"
          class:acceso={asse === "tipo"}
          aria-pressed={asse === "tipo"}
          onclick={() => scegliAsse("tipo")}
        >
          tipo
        </button>
      </div>
      <p class="aiuto">
        {asse === "contesto"
          ? "Dove appartiene il file: è l'asse che decide le cartelle in Organizza."
          : "Cosa è il file. Il valore deve essere uno dei tipi conosciuti, altrimenti il motore lo legge come «altro»."}
      </p>
    </div>
  </div>

  <Campo
    bind:valore={pattern}
    etichetta="Pattern (glob)"
    segnaposto={asse === "contesto" ? "**/T1Q*" : "*.bak"}
    icona="filtro"
    azzerabile
    autocomplete="off"
    spellcheck={false}
    errore={provato && !pattern.trim()
      ? "Senza pattern la regola non aggancerebbe niente."
      : (errorePattern ?? undefined)}
    descrizione={!errorePattern && pattern.trim()
      ? sulPath
        ? "Contiene / oppure **: verrà confrontato con il percorso completo."
        : "Nessuna barra: verrà confrontato con il solo nome del file."
      : undefined}
  />

  {#if asse === "contesto"}
    <Campo
      bind:valore
      etichetta="Contesto da assegnare"
      segnaposto="lavoro/PAM"
      azzerabile
      autocomplete="off"
      spellcheck={false}
      list="contesti-{idContesti}"
      errore={provato && !valore.trim() ? "Serve un contesto da assegnare." : undefined}
      descrizione="La barra è gerarchica: «lavoro/PAM» e «lavoro/DevOps» finiranno dentro la stessa cartella «lavoro»."
    />
    <datalist id="contesti-{idContesti}">
      {#each contesti as c (c.etichetta)}
        <option value={c.etichetta}></option>
      {/each}
    </datalist>

    {#if contesti.length > 0}
      <div class="chip-riga">
        <span class="etichetta-inline">Già in uso:</span>
        {#each contesti.slice(0, 12) as c (c.etichetta)}
          <button
            class="chip"
            class:acceso={valore.trim() === c.etichetta}
            onclick={() => (valore = c.etichetta)}
          >
            {c.etichetta}
            <span class="conta cifre">{formattaNumero(c.quanti)}</span>
          </button>
        {/each}
      </div>
    {/if}
  {:else}
    <div class="campo-asse">
      <span class="etichetta-campo">Tipo da assegnare</span>
      <div class="chip-riga">
        {#each TIPI as t (t)}
          <button
            class="chip"
            class:acceso={valore === t}
            aria-pressed={valore === t}
            onclick={() => (valore = t)}
          >
            {t}
          </button>
        {/each}
      </div>
      {#if provato && !valoreValido}
        <p class="aiuto errore">Scegli uno dei tipi conosciuti.</p>
      {/if}
    </div>
  {/if}

  <div class="griglia-form">
    <Campo
      bind:valore={priorita}
      etichetta="Priorità"
      type="number"
      min="0"
      max="9999"
      descrizione="Più bassa = valutata prima. Le regole di serie stanno fra 10 e 30: 50 lascia vincere loro."
      errore={provato && !prioritaValida ? "Serve un numero fra 0 e 9999." : undefined}
    />
  </div>

  {#if doppione}
    <div class="avvertenza">
      <Icona nome="avviso" dimensione={16} />
      <p>
        Esiste già una regola con lo stesso pattern sullo stesso asse:
        <strong>{doppione.nome}</strong> → {doppione.valore} (priorità
        {doppione.priorita}). Puoi salvare lo stesso, ma vincerà quella con la
        priorità più bassa e l'altra resterà lì a non fare niente.
      </p>
    </div>
  {/if}

  <!-- Quanti file colpisce ---------------------------------------------- -->
  {#if pattern.trim() && !errorePattern}
    <div class="colpiti">
      {#if contando && !colpiti}
        <p class="aiuto">Conto i file…</p>
      {:else if colpiti}
        <p class="numero-colpiti">
          <span class="cifre grosso">{formattaNumero(colpiti.quanti)}</span>
          <span>
            {colpiti.quanti === 1 ? "file colpito" : "file colpiti"}
            <span class="testo-tenue">
              — su tutti i file indicizzati, artefatti inclusi
            </span>
          </span>
        </p>
        {#if colpiti.esempi.length > 0}
          <ul class="esempi">
            {#each colpiti.esempi as e (e)}
              <li class="mono troncato" title={e}>{accorciaPath(e, 68)}</li>
            {/each}
          </ul>
        {:else}
          <p class="aiuto">
            Nessun file dell'indice combacia con questo pattern: controlla se
            volevi scriverlo sul percorso — con <span class="mono">**/</span> davanti
            — invece che sul nome.
          </p>
        {/if}
      {/if}
    </div>
  {/if}

  {#if errore}
    <p class="aiuto errore" role="alert">{errore}</p>
  {/if}

  <div class="piede">
    <Bottone variante="fantasma" onclick={onchiudi}>Chiudi</Bottone>
    <Bottone variante="primario" icona="piu" caricamento={salvando} onclick={salva}>
      Crea la regola
    </Bottone>
  </div>
</div>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    padding: var(--sp-4);
    border-radius: var(--raggio-lg);
    background: var(--superficie-2);
  }

  .griglia-form {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--sp-4);
    align-items: start;
  }

  @media (max-width: 820px) {
    .griglia-form {
      grid-template-columns: minmax(0, 1fr);
    }
  }

  .campo-asse {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
  }

  .etichetta-campo {
    font-size: var(--minuto);
    font-weight: var(--peso-forte);
    color: var(--testo-2);
  }

  .aiuto {
    font-size: var(--minuto);
    color: var(--testo-3);
    line-height: var(--riga-larga);
  }

  .aiuto.errore {
    color: var(--pericolo);
  }

  .fasce {
    display: inline-flex;
    gap: 2px;
    padding: 3px;
    border-radius: var(--raggio-pillola);
    background: var(--superficie);
    border: 1px solid var(--bordo);
    align-self: flex-start;
  }

  .fascia {
    height: 28px;
    padding: 0 var(--sp-4);
    border-radius: var(--raggio-pillola);
    font-size: var(--minuto);
    font-weight: var(--peso-forte);
    color: var(--testo-2);
  }

  .fascia.acceso {
    background: var(--sfondo);
    color: var(--testo);
    box-shadow: var(--ombra-1);
  }

  .regola-match {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
    padding: var(--sp-3) var(--sp-4);
    border-radius: var(--raggio);
    background: var(--info-bg);
    color: var(--info);
    font-size: var(--piccolo);
    line-height: var(--riga-larga);
  }

  .icona-nota {
    display: flex;
    flex: 0 0 auto;
    margin-top: 2px;
  }

  .chip-riga {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
  }

  .etichetta-inline {
    font-size: var(--micro);
    color: var(--testo-3);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    max-width: 100%;
    height: 28px;
    padding: 0 var(--sp-3);
    border-radius: var(--raggio-pillola);
    border: 1px solid var(--bordo);
    background: var(--superficie);
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

  .conta {
    font-size: var(--micro);
    color: var(--testo-3);
  }

  .chip.acceso .conta {
    color: inherit;
  }

  .avvertenza {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-2);
    padding: var(--sp-3) var(--sp-4);
    border-radius: var(--raggio);
    background: var(--avviso-bg);
    color: var(--avviso);
    font-size: var(--piccolo);
    line-height: var(--riga-larga);
  }

  .colpiti {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding: var(--sp-3) var(--sp-4);
    border-radius: var(--raggio);
    background: var(--superficie);
  }

  .numero-colpiti {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: var(--sp-2);
    font-size: var(--minuto);
    color: var(--testo-2);
  }

  .grosso {
    font-size: var(--grande);
    font-weight: var(--peso-grasso);
    color: var(--testo);
    letter-spacing: -0.02em;
  }

  .esempi {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: var(--micro);
    color: var(--testo-3);
  }

  .piede {
    display: flex;
    justify-content: flex-end;
    gap: var(--sp-2);
  }
</style>
