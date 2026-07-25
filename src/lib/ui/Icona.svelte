<script module lang="ts">
  /**
   * Tutte le icone dell'app, disegnate a mano su una griglia 24×24 con tratto
   * sottile e `currentColor`: nessuna dipendenza da set esterni.
   */
  export const NOMI_ICONA = [
    // navigazione
    "dashboard",
    "ricerca",
    "novita",
    "ingombro",
    "duplicati",
    "tracciati",
    "revisione",
    "impostazioni",
    "lotti",
    // tipi di file
    "cartella",
    "documento",
    "archivio",
    "media",
    "immagine",
    "installer",
    "artefatto",
    "altro",
    // azioni e segnali
    "chiudi",
    "freccia",
    "occhio",
    "avviso",
    "check",
    "info",
    "piu",
    "cestino",
    "aggiorna",
    "filtro",
    "esterno",
    "avvia",
    "ferma",
    "esci",
    // tema
    "tema",
    "sole",
    "luna",
    "monitor",
  ] as const;

  export type NomeIcona = (typeof NOMI_ICONA)[number];
</script>

<script lang="ts">
  interface Props {
    /** Quale icona disegnare. */
    nome: NomeIcona;
    /** Lato del quadrato in pixel. */
    dimensione?: number;
    /** Spessore del tratto; 1.5 è il valore di sistema. */
    spessore?: number;
    /** Rotazione in gradi: utile per riusare `freccia` in quattro versi. */
    ruota?: number;
    /** Testo alternativo. Se assente l'icona è decorativa e viene nascosta. */
    titolo?: string;
    classe?: string;
  }

  let {
    nome,
    dimensione = 20,
    spessore = 1.5,
    ruota = 0,
    titolo,
    classe = "",
  }: Props = $props();
</script>

<svg
  class="icona {classe}"
  width={dimensione}
  height={dimensione}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={spessore}
  stroke-linecap="round"
  stroke-linejoin="round"
  role={titolo ? "img" : "presentation"}
  aria-hidden={titolo ? undefined : "true"}
  aria-label={titolo}
  style={ruota ? `transform: rotate(${ruota}deg)` : undefined}
>
  {#if titolo}<title>{titolo}</title>{/if}

  {#if nome === "dashboard"}
    <rect x="3" y="3" width="7.5" height="7.5" rx="2.2" />
    <rect x="13.5" y="3" width="7.5" height="7.5" rx="2.2" />
    <rect x="3" y="13.5" width="7.5" height="7.5" rx="2.2" />
    <rect x="13.5" y="13.5" width="7.5" height="7.5" rx="2.2" />
  {:else if nome === "ricerca"}
    <circle cx="11" cy="11" r="7" />
    <path d="m16.2 16.2 4.3 4.3" />
  {:else if nome === "novita"}
    <circle cx="12" cy="12" r="8.5" />
    <path d="M12 7.2V12l3.2 2" />
  {:else if nome === "ingombro"}
    <!-- Una mappa ad aree: un blocco grande e due piccoli. Dice «quanto
         occupa cosa» senza somigliare né a `dashboard` (quattro quadrati
         uguali) né a `filtro` (righe decrescenti centrate). -->
    <rect x="3" y="3" width="18" height="18" rx="3" />
    <path d="M12.6 3v18M12.6 12H21" />
  {:else if nome === "duplicati"}
    <rect x="8.5" y="3.5" width="12" height="12" rx="3" />
    <path d="M15.5 19.2a2.3 2.3 0 0 1-2.3 1.3H6.5a3 3 0 0 1-3-3V9.8a2.3 2.3 0 0 1 1.3-2.3" />
  {:else if nome === "tracciati"}
    <rect x="3" y="4.5" width="18" height="15" rx="3" />
    <path d="M3 9.6h18M3 14.4h18M9.5 9.6v9.9" />
  {:else if nome === "lotti"}
    <path d="M3 8.2 12 3.6l9 4.6-9 4.6z" />
    <path d="m3 12.4 9 4.6 9-4.6M3 16.4 12 21l9-4.6" />
  {:else if nome === "revisione"}
    <path d="M9.5 4.2h5a1.8 1.8 0 0 1 0 3.6h-5a1.8 1.8 0 0 1 0-3.6z" />
    <path d="M16.5 6h1.3A2.2 2.2 0 0 1 20 8.2v10.6a2.2 2.2 0 0 1-2.2 2.2H6.2A2.2 2.2 0 0 1 4 18.8V8.2A2.2 2.2 0 0 1 6.2 6h1.3" />
    <path d="m8.8 14.2 2 2 4.4-4.4" />
  {:else if nome === "impostazioni"}
    <path d="M4 7.2h9.2M18.4 7.2H20M4 16.8h4.4M13.6 16.8H20" />
    <circle cx="15.8" cy="7.2" r="2.4" />
    <circle cx="11" cy="16.8" r="2.4" />
  {:else if nome === "cartella"}
    <path
      d="M3 7.6A2.6 2.6 0 0 1 5.6 5h3.1a2 2 0 0 1 1.63.84l.98 1.37a2 2 0 0 0 1.63.84h5.46A2.6 2.6 0 0 1 21 10.65v5.75A2.6 2.6 0 0 1 18.4 19H5.6A2.6 2.6 0 0 1 3 16.4z"
    />
  {:else if nome === "documento"}
    <path d="M14 3.2H7.6A2.6 2.6 0 0 0 5 5.8v12.4A2.6 2.6 0 0 0 7.6 20.8h8.8a2.6 2.6 0 0 0 2.6-2.6V8.2z" />
    <path d="M13.8 3.4v4.6h4.9" />
    <path d="M9 13h6M9 16.6h4" />
  {:else if nome === "archivio"}
    <path d="M3 8.4 12 4l9 4.4v7.2L12 20l-9-4.4z" />
    <path d="m3 8.4 9 4.4 9-4.4M12 12.8V20" />
  {:else if nome === "media"}
    <circle cx="12" cy="12" r="8.5" />
    <path d="m10.2 8.6 5.6 3.4-5.6 3.4z" />
  {:else if nome === "immagine"}
    <rect x="3.2" y="4.6" width="17.6" height="14.8" rx="3" />
    <circle cx="9" cy="10" r="1.6" />
    <path d="m4.2 17.4 4.4-4.2a2 2 0 0 1 2.7 0l5.1 4.6M14.5 14.4l1.7-1.5a2 2 0 0 1 2.6 0l1 .9" />
  {:else if nome === "installer"}
    <path d="M12 3.4v10.2m0 0 3.6-3.6M12 13.6 8.4 10" />
    <path d="M4.2 16.6v1.6A2.6 2.6 0 0 0 6.8 20.8h10.4a2.6 2.6 0 0 0 2.6-2.6v-1.6" />
  {:else if nome === "artefatto"}
    <path
      d="M6.4 3.5H9M15 3.5h2.6M20.5 6.4V9M20.5 15v2.6M17.6 20.5H15M9 20.5H6.4M3.5 17.6V15M3.5 9V6.4"
    />
    <path d="M9.6 12h4.8M12 9.6v4.8" />
  {:else if nome === "altro"}
    <circle cx="5.6" cy="12" r="1.3" fill="currentColor" stroke="none" />
    <circle cx="12" cy="12" r="1.3" fill="currentColor" stroke="none" />
    <circle cx="18.4" cy="12" r="1.3" fill="currentColor" stroke="none" />
  {:else if nome === "chiudi"}
    <path d="m6.4 6.4 11.2 11.2M17.6 6.4 6.4 17.6" />
  {:else if nome === "freccia"}
    <path d="m9.5 4.8 7.2 7.2-7.2 7.2" />
  {:else if nome === "occhio"}
    <path d="M2.6 12S6.2 5.6 12 5.6 21.4 12 21.4 12 17.8 18.4 12 18.4 2.6 12 2.6 12z" />
    <circle cx="12" cy="12" r="3.1" />
  {:else if nome === "avviso"}
    <path
      d="M10.3 4.4 2.7 17.5a2 2 0 0 0 1.7 3h15.2a2 2 0 0 0 1.7-3L13.7 4.4a2 2 0 0 0-3.4 0z"
    />
    <path d="M12 9.6v4.2M12 17.2h.01" />
  {:else if nome === "check"}
    <path d="m5 12.6 4.6 4.6L19 6.8" />
  {:else if nome === "info"}
    <circle cx="12" cy="12" r="8.5" />
    <path d="M12 11v5.4M12 7.8h.01" />
  {:else if nome === "piu"}
    <path d="M12 5.2v13.6M5.2 12h13.6" />
  {:else if nome === "cestino"}
    <path d="M4 7h16" />
    <path d="M9.6 7V5.6A1.6 1.6 0 0 1 11.2 4h1.6a1.6 1.6 0 0 1 1.6 1.6V7" />
    <path d="m6.6 7 .78 11.6A2.2 2.2 0 0 0 9.57 20.6h4.86a2.2 2.2 0 0 0 2.19-2L17.4 7" />
    <path d="M10.4 11v5.4M13.6 11v5.4" />
  {:else if nome === "aggiorna"}
    <path d="M20 12a8 8 0 1 1-2.34-5.66" />
    <path d="M20.2 4v4.4h-4.4" />
  {:else if nome === "filtro"}
    <path d="M4 6.4h16M7 12h10M10 17.6h4" />
  {:else if nome === "esterno"}
    <path d="M14 4h6v6M20 4l-8.6 8.6" />
    <path d="M18 14.4v3.8A2.8 2.8 0 0 1 15.2 21H6.8A2.8 2.8 0 0 1 4 18.2V9.8A2.8 2.8 0 0 1 6.8 7h3.8" />
  {:else if nome === "avvia"}
    <circle cx="12" cy="12" r="8.5" />
    <path d="m10.3 8.4 5.4 3.6-5.4 3.6z" />
  {:else if nome === "ferma"}
    <circle cx="12" cy="12" r="8.5" />
    <rect x="9.2" y="9.2" width="5.6" height="5.6" rx="1.4" />
  {:else if nome === "esci"}
    <path d="M12 3.4v8.4" />
    <path d="M7.6 6.5a7.6 7.6 0 1 0 8.8 0" />
  {:else if nome === "tema"}
    <circle cx="12" cy="12" r="8.5" />
    <path d="M12 3.5a8.5 8.5 0 0 1 0 17z" fill="currentColor" stroke="none" />
  {:else if nome === "sole"}
    <circle cx="12" cy="12" r="4.2" />
    <path
      d="M12 2.6v2.2M12 19.2v2.2M21.4 12h-2.2M4.8 12H2.6M18.6 5.4l-1.6 1.6M7 17l-1.6 1.6M18.6 18.6 17 17M7 7 5.4 5.4"
    />
  {:else if nome === "luna"}
    <path d="M20.3 14.6A8.6 8.6 0 0 1 9.4 3.7a8.6 8.6 0 1 0 10.9 10.9z" />
  {:else if nome === "monitor"}
    <rect x="3" y="4.4" width="18" height="12.4" rx="2.6" />
    <path d="M9 20.4h6M12 16.8v3.6" />
  {/if}
</svg>

<style>
  .icona {
    flex: 0 0 auto;
    transition: transform var(--transizione);
  }
</style>
