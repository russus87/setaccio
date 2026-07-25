<script lang="ts">
  import type { HTMLInputAttributes } from "svelte/elements";
  import Icona, { type NomeIcona } from "./Icona.svelte";

  interface Props extends Omit<HTMLInputAttributes, "value" | "size"> {
    /** Il valore, associabile con `bind:valore`. */
    valore?: string;
    etichetta?: string;
    /** Testo di aiuto sotto il campo. */
    descrizione?: string;
    /** Messaggio d'errore: sostituisce la descrizione e colora il bordo. */
    errore?: string;
    segnaposto?: string;
    /** Icona a sinistra, dentro il campo. */
    icona?: NomeIcona;
    /** Mostra la crocetta per svuotare il campo quando contiene qualcosa. */
    azzerabile?: boolean;
    dimensione?: "md" | "lg";
    /** Elimina il fondo e il bordo: per campi dentro barre già delimitate. */
    nudo?: boolean;
    /** Chiamato quando l'utente preme Invio. */
    oninvio?: (valore: string) => void;
    /** Chiamato dopo lo svuotamento con la crocetta. */
    onazzera?: () => void;
  }

  let {
    valore = $bindable(""),
    etichetta,
    descrizione,
    errore,
    segnaposto,
    icona,
    azzerabile = false,
    dimensione = "md",
    nudo = false,
    oninvio,
    onazzera,
    id,
    type = "text",
    disabled = false,
    ...resto
  }: Props = $props();

  // Un id serve comunque per legare <label> e <input>: se non arriva da fuori
  // se ne genera uno stabile e unico per istanza.
  const idAuto = $props.id();
  const idCampo = $derived(id ?? `campo-${idAuto}`);
  const idAiuto = $derived(`${idCampo}-aiuto`);
  const conAiuto = $derived(!!(errore || descrizione));

  function suTasto(e: KeyboardEvent) {
    if (e.key === "Enter") oninvio?.(valore);
  }

  function azzera() {
    valore = "";
    onazzera?.();
  }
</script>

<div class="campo {dimensione}" class:nudo class:errato={!!errore} class:inattivo={disabled}>
  {#if etichetta}
    <label class="etichetta" for={idCampo}>{etichetta}</label>
  {/if}

  <div class="guscio">
    {#if icona}
      <span class="icona-sx"><Icona nome={icona} dimensione={dimensione === "lg" ? 20 : 17} /></span>
    {/if}

    <input
      id={idCampo}
      {type}
      {disabled}
      class="input"
      placeholder={segnaposto}
      bind:value={valore}
      onkeydown={suTasto}
      aria-invalid={errore ? "true" : undefined}
      aria-describedby={conAiuto ? idAiuto : undefined}
      {...resto}
    />

    {#if azzerabile && valore}
      <button class="azzera" type="button" onclick={azzera} aria-label="Svuota il campo">
        <Icona nome="chiudi" dimensione={15} />
      </button>
    {/if}
  </div>

  {#if conAiuto}
    <p id={idAiuto} class="aiuto" class:errore={!!errore}>{errore ?? descrizione}</p>
  {/if}
</div>

<style>
  .campo {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
  }

  .etichetta {
    font-size: var(--minuto);
    font-weight: var(--peso-forte);
    color: var(--testo-2);
  }

  .guscio {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    height: var(--altezza-controllo);
    padding: 0 var(--sp-3);
    background: var(--superficie-2);
    border: 1px solid var(--bordo);
    border-radius: var(--raggio-pillola);
    transition:
      border-color var(--transizione),
      background var(--transizione),
      box-shadow var(--transizione);
  }

  .lg .guscio {
    height: var(--altezza-controllo-lg);
    padding: 0 var(--sp-5);
    gap: var(--sp-3);
  }

  .guscio:hover {
    border-color: var(--bordo-forte);
  }

  .guscio:focus-within {
    border-color: var(--accento-bordo);
    box-shadow: 0 0 0 3px var(--accento-tenue);
  }

  .nudo .guscio {
    background: transparent;
    border-color: transparent;
  }

  .nudo .guscio:focus-within {
    box-shadow: none;
    border-color: var(--accento-bordo);
  }

  .errato .guscio {
    border-color: var(--pericolo);
  }

  .inattivo .guscio {
    opacity: 0.55;
  }

  .icona-sx {
    display: flex;
    color: var(--testo-3);
    flex: 0 0 auto;
  }

  .guscio:focus-within .icona-sx {
    color: var(--testo-2);
  }

  .input {
    flex: 1 1 auto;
    min-width: 0;
    height: 100%;
    font-size: var(--corpo);
    color: var(--testo);
    outline: none;
  }

  .lg .input {
    font-size: var(--medio);
  }

  .input::placeholder {
    color: var(--testo-3);
  }

  .azzera {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    color: var(--testo-3);
    flex: 0 0 auto;
    transition:
      background var(--transizione),
      color var(--transizione);
  }

  .azzera:hover {
    background: var(--superficie-3);
    color: var(--testo);
  }

  .aiuto {
    font-size: var(--minuto);
    color: var(--testo-3);
  }

  .aiuto.errore {
    color: var(--pericolo);
  }
</style>
