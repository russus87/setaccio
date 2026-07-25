<script lang="ts">
  /**
   * L'albero di cartelle che nascerà dall'organizzazione.
   *
   * Da quando i contesti sono gerarchici — `lavoro/PAM`, `lavoro/DevOps` — la
   * domanda vera prima di premere il bottone non è «quanti file sposto» ma
   * «che forma prende la cartella». Qui la si vede: una sola `lavoro/` con
   * dentro le due sottocartelle, non due cartelle sorelle.
   *
   * È un disegno costruito dai contesti selezionati, non una lettura del
   * disco: i conteggi vengono dalle faccette, il piano vero lo fa il backend.
   */
  import { accorciaPath, formattaByte, formattaNumero, type ConteggioEtichetta } from "../api";
  import Badge from "../ui/Badge.svelte";
  import Icona from "../ui/Icona.svelte";

  interface Props {
    /** Cartella di destinazione: la radice dell'albero. */
    radice: string;
    /** I contesti scelti, con i loro conteggi. */
    voci: ConteggioEtichetta[];
  }

  let { radice, voci }: Props = $props();

  interface Nodo {
    nome: string;
    figli: Nodo[];
    quanti: number;
    byte: number;
    /** Vero quando almeno un contesto finisce esattamente qui. */
    finale: boolean;
  }

  /**
   * Le stesse esclusioni di `contesto_sicuro()` in Rust: un contesto con
   * `..`, un carattere di unità o troppo profondo non diventa una cartella,
   * diventa una mossa saltata con avviso.
   */
  function contestoValido(contesto: string): boolean {
    const segmenti = contesto
      .trim()
      .replace(/\\/g, "/")
      .split("/")
      .map((s) => s.trim())
      .filter((s) => s !== "" && s !== ".");
    if (segmenti.length === 0 || segmenti.length > 8) return false;
    return !segmenti.some((s) => s === ".." || s.includes(":"));
  }

  function segmentiDi(contesto: string): string[] {
    return contesto
      .trim()
      .replace(/\\/g, "/")
      .split("/")
      .map((s) => s.trim())
      .filter((s) => s !== "" && s !== ".");
  }

  const validi = $derived(voci.filter((v) => contestoValido(v.etichetta)));
  const invalidi = $derived(voci.filter((v) => !contestoValido(v.etichetta)));

  const albero = $derived.by(() => {
    const radici: Nodo[] = [];
    for (const v of validi) {
      let livello = radici;
      const segmenti = segmentiDi(v.etichetta);
      segmenti.forEach((seg, i) => {
        let nodo = livello.find((n) => n.nome === seg);
        if (!nodo) {
          nodo = { nome: seg, figli: [], quanti: 0, byte: 0, finale: false };
          livello.push(nodo);
        }
        // I conteggi risalgono: una cartella intermedia mostra il totale di
        // quello che le finirà dentro.
        nodo.quanti += v.quanti;
        nodo.byte += v.byte;
        if (i === segmenti.length - 1) nodo.finale = true;
        livello = nodo.figli;
      });
    }
    const ordina = (n: Nodo[]) => {
      n.sort((a, b) => a.nome.localeCompare(b.nome, "it"));
      for (const x of n) ordina(x.figli);
    };
    ordina(radici);
    return radici;
  });

  const totale = $derived(validi.reduce((s, v) => s + v.quanti, 0));
</script>

{#snippet ramo(nodo: Nodo, profondita: number)}
  <li class="nodo" style="padding-left: {profondita * 20}px">
    <span class="cartella" class:intermedia={!nodo.finale}>
      <Icona nome="cartella" dimensione={15} />
      <span class="nome-cartella">{nodo.nome}/</span>
    </span>
    <span class="misura cifre">
      {formattaNumero(nodo.quanti)} file · {formattaByte(nodo.byte)}
    </span>
  </li>
  {#each nodo.figli as f (f.nome)}
    {@render ramo(f, profondita + 1)}
  {/each}
{/snippet}

<div class="albero">
  <p class="titolo-albero">
    Cartelle che verranno create
    <span class="testo-tenue">— disegnate dai contesti scelti</span>
  </p>

  <ul class="lista">
    <li class="nodo radice">
      <span class="cartella">
        <Icona nome="cartella" dimensione={15} />
        <span class="nome-cartella mono" title={radice}>
          {radice ? accorciaPath(radice, 52) : "(destinazione da scegliere)"}
        </span>
      </span>
      <span class="misura cifre">{formattaNumero(totale)} file in tutto</span>
    </li>
    {#each albero as n (n.nome)}
      {@render ramo(n, 1)}
    {/each}
  </ul>

  {#if invalidi.length > 0}
    <div class="invalidi">
      <Icona nome="avviso" dimensione={15} />
      <p>
        {#each invalidi as v, i (v.etichetta)}<span class="mono">{v.etichetta}</span
          >{i < invalidi.length - 1 ? ", " : ""}{/each}
        {invalidi.length === 1 ? "non è un percorso valido" : "non sono percorsi validi"}
        (usa <span class="mono">..</span>, un percorso assoluto o
        <span class="mono">:</span>): il piano le mostrerà come mosse saltate, con
        l'avviso del backend.
      </p>
      <Badge testo={String(invalidi.length)} variante="avviso" />
    </div>
  {/if}
</div>

<style>
  .albero {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding: var(--sp-4);
    border-radius: var(--raggio-lg);
    background: var(--superficie-2);
  }

  .titolo-albero {
    font-size: var(--micro);
    font-weight: var(--peso-forte);
    color: var(--testo-3);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .titolo-albero span {
    font-weight: var(--peso-normale);
    text-transform: none;
    letter-spacing: 0;
  }

  .lista {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .nodo {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-width: 0;
    padding-top: 3px;
    padding-bottom: 3px;
  }

  .cartella {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    min-width: 0;
    color: var(--testo);
    font-size: var(--piccolo);
    font-weight: var(--peso-forte);
  }

  .cartella.intermedia {
    color: var(--testo-2);
    font-weight: var(--peso-medio);
  }

  .nome-cartella {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .radice .cartella {
    color: var(--accento-testo);
  }

  .misura {
    flex: 0 0 auto;
    margin-left: auto;
    font-size: var(--micro);
    color: var(--testo-3);
  }

  .invalidi {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-2);
    margin-top: var(--sp-1);
    padding: var(--sp-2) var(--sp-3);
    border-radius: var(--raggio-sm);
    background: var(--avviso-bg);
    color: var(--avviso);
    font-size: var(--minuto);
    line-height: var(--riga-larga);
  }

  .invalidi p {
    flex: 1 1 auto;
  }
</style>
