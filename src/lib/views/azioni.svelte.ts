/**
 * Cosa si può fare a una selezione di file, in un posto solo.
 *
 * Le tre operazioni non sono varianti della stessa cosa e la differenza va
 * tenuta visibile fino all'ultimo clic:
 *
 * - **quarantena** sposta in una cartella dedicata, ed è annullabile da qui;
 * - **cestino** consegna al cestino di sistema: si recupera dal gestore file,
 *   non da Setaccio;
 * - **elimina** cancella, e non c'è recupero da nessuna parte.
 *
 * Ogni vista che elenca file istanzia una `AzioniFile` e disegna i bottoni
 * dove le serve: il flusso piano → conferma → esito sta scritto qui una volta
 * sola, così non può divergere fra una schermata e l'altra.
 */

import {
  cestinoPiano,
  duplicatiPiano,
  eliminaPiano,
  operazioniEsegui,
  type EsitoOperazioni,
  type PianoOperazioni,
} from "../api";
import { messaggioErrore } from "./comuni";

export type GenereAzione = "quarantena" | "cestino" | "elimina";

interface Descrizione {
  /** Testo del bottone che chiede il piano. */
  bottone: string;
  /** Testo del bottone che conferma l'esecuzione. */
  conferma: string;
  /** Frase che spiega cosa succederà ai file, mostrata nel riepilogo. */
  spiegazione: string;
  /** Colora il riepilogo di rosso: l'operazione perde qualcosa. */
  pericolo: boolean;
  /**
   * Parola da digitare per sbloccare la conferma. Solo per ciò che non si può
   * disfare: mettere un attrito dove non serve insegna a ignorarlo.
   */
  parolaChiave?: string;
}

export const AZIONI: Record<GenereAzione, Descrizione> = {
  quarantena: {
    bottone: "Metti in quarantena",
    conferma: "Metti in quarantena",
    spiegazione:
      "I file non vengono cancellati: vengono spostati nella cartella di quarantena, dentro i dati dell'applicazione. Se qualcosa non torna, annulla il batch e tornano al loro posto.",
    pericolo: false,
  },
  cestino: {
    bottone: "Sposta nel cestino",
    conferma: "Sposta nel cestino",
    spiegazione:
      "I file vanno nel cestino di sistema: restano recuperabili dal gestore file, ma non dall'annulla di Setaccio. Escono dall'indice, quindi smettono di comparire nelle ricerche e nei conti dello spazio.",
    pericolo: true,
  },
  elimina: {
    bottone: "Elimina",
    conferma: "Elimina definitivamente",
    spiegazione:
      "I file vengono cancellati dal disco senza passare dal cestino. Non c'è modo di riportarli indietro, né da Setaccio né dal sistema. Usalo quando il cestino non basta perché il disco è pieno davvero.",
    pericolo: true,
    parolaChiave: "ELIMINA",
  },
};

export class AzioniFile {
  /** Il piano in attesa di conferma. `null` quando non c'è niente da decidere. */
  piano = $state<PianoOperazioni | null>(null);
  /** Esito dell'ultima esecuzione: quando c'è, il riepilogo smette di chiedere. */
  esito = $state<EsitoOperazioni | null>(null);
  /** Quale operazione ha prodotto il piano aperto. */
  genere = $state<GenereAzione>("quarantena");
  inCorso = $state(false);
  errore = $state<string | null>(null);

  /** Chiamata dopo un'esecuzione andata a buon fine: la vista rilegge i dati. */
  #onfatto: () => void;

  constructor(onfatto: () => void = () => {}) {
    this.#onfatto = onfatto;
  }

  get descrizione(): Descrizione {
    return AZIONI[this.genere];
  }

  /** Costruisce il piano. Non tocca il disco: serve solo a farlo vedere. */
  async prepara(genere: GenereAzione, fileIds: number[]) {
    if (fileIds.length === 0) return;
    this.inCorso = true;
    this.esito = null;
    this.genere = genere;
    try {
      const ids = [...fileIds];
      this.piano =
        genere === "cestino"
          ? await cestinoPiano(ids)
          : genere === "elimina"
            ? await eliminaPiano(ids)
            : await duplicatiPiano(ids);
      this.errore = null;
    } catch (e) {
      this.piano = null;
      this.errore = messaggioErrore(e);
    } finally {
      this.inCorso = false;
    }
  }

  /** Esegue il piano già mostrato. Da qui in poi il disco cambia davvero. */
  async esegui() {
    if (!this.piano) return;
    this.inCorso = true;
    try {
      this.esito = await operazioniEsegui(this.piano);
      this.errore = null;
      this.#onfatto();
    } catch (e) {
      this.errore = messaggioErrore(e);
    } finally {
      this.inCorso = false;
    }
  }

  chiudi() {
    this.piano = null;
    this.esito = null;
  }
}
