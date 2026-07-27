/**
 * Disposizione squarificata per le treemap.
 *
 * L'algoritmo è quello di Bruls, Huizing e van Wijk: si riempie il rettangolo
 * una fila alla volta, e a ogni passo si aggiunge un elemento alla fila finché
 * il rapporto d'aspetto *peggiore* della fila continua a migliorare. Appena
 * peggiora, la fila si chiude e si ricomincia sullo spazio che resta.
 *
 * Il motivo per non usare la disposizione ingenua «taglia e affetta» è
 * pratico: quella produce schegge lunghe e sottili, in cui l'area non si
 * confronta più a occhio e l'etichetta non ci sta. Qui le tessere restano
 * vicine al quadrato, che è l'unica forma in cui il confronto visivo fra aree
 * funziona davvero.
 *
 * Il modulo è puro — nessun DOM, nessuna dipendenza — così si può provare a
 * mano e riusare per qualsiasi grafico ad area.
 */

/** Un rettangolo in coordinate del contenitore, in pixel. */
export interface Rettangolo {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Un elemento da disporre: il dato che porti dietro e quanto pesa. */
export interface NodoTreemap<T> {
  dato: T;
  /** Il peso. I valori nulli o negativi vengono ignorati. */
  valore: number;
}

/** Un elemento disposto: il suo dato e il rettangolo che gli tocca. */
export interface PiastraTreemap<T> extends Rettangolo {
  dato: T;
  valore: number;
}

/**
 * Il rapporto d'aspetto peggiore di una fila, dato l'insieme delle aree già
 * scalate, la loro somma e il lato corto su cui la fila si appoggia. Più è
 * basso, più le tessere sono vicine al quadrato.
 */
function peggiore(aree: number[], somma: number, lato: number): number {
  let max = -Infinity;
  let min = Infinity;
  for (const a of aree) {
    if (a > max) max = a;
    if (a < min) min = a;
  }
  if (min <= 0 || somma <= 0 || lato <= 0) return Infinity;
  const s2 = somma * somma;
  const l2 = lato * lato;
  return Math.max((l2 * max) / s2, s2 / (l2 * min));
}

/**
 * Dispone i nodi dentro il rettangolo dato.
 *
 * L'ordine di uscita non è quello di entrata: i nodi vengono ordinati per
 * peso decrescente, che è ciò che rende squadrata la disposizione. Chi chiama
 * si tenga il proprio dato dentro `dato` per ritrovarsi.
 */
export function squarifica<T>(
  nodi: readonly NodoTreemap<T>[],
  x: number,
  y: number,
  w: number,
  h: number,
): PiastraTreemap<T>[] {
  const esito: PiastraTreemap<T>[] = [];
  if (w <= 0 || h <= 0) return esito;

  const items = nodi
    .filter((n) => Number.isFinite(n.valore) && n.valore > 0)
    .slice()
    .sort((a, b) => b.valore - a.valore);
  if (items.length === 0) return esito;

  let rx = x;
  let ry = y;
  let rw = w;
  let rh = h;
  let i = 0;

  // La guardia su rw/rh evita di girare a vuoto quando lo spazio residuo si è
  // ridotto sotto il mezzo pixel: lì non c'è più niente di disegnabile.
  while (i < items.length && rw > 0.5 && rh > 0.5) {
    let totaleRimasto = 0;
    for (let k = i; k < items.length; k++) totaleRimasto += items[k].valore;
    if (totaleRimasto <= 0) break;

    const scala = (rw * rh) / totaleRimasto;
    const lato = Math.min(rw, rh);

    const fila: NodoTreemap<T>[] = [];
    let areaFila = 0;
    let migliore = Infinity;

    while (i < items.length) {
      const area = items[i].valore * scala;
      const aree = fila.map((n) => n.valore * scala);
      aree.push(area);
      const rapporto = peggiore(aree, areaFila + area, lato);
      // Il primo elemento entra sempre: una fila vuota non ha rapporto.
      if (fila.length === 0 || rapporto <= migliore) {
        fila.push(items[i]);
        areaFila += area;
        migliore = rapporto;
        i++;
      } else break;
    }

    if (rw >= rh) {
      // Fila verticale, appoggiata al bordo sinistro dello spazio residuo.
      const larghezza = areaFila / rh;
      let cy = ry;
      for (const n of fila) {
        const altezza = (n.valore * scala) / larghezza;
        esito.push({ dato: n.dato, valore: n.valore, x: rx, y: cy, w: larghezza, h: altezza });
        cy += altezza;
      }
      rx += larghezza;
      rw -= larghezza;
    } else {
      // Fila orizzontale, appoggiata al bordo superiore.
      const altezza = areaFila / rw;
      let cx = rx;
      for (const n of fila) {
        const larghezza = (n.valore * scala) / altezza;
        esito.push({ dato: n.dato, valore: n.valore, x: cx, y: ry, w: larghezza, h: altezza });
        cx += larghezza;
      }
      ry += altezza;
      rh -= altezza;
    }
  }

  return esito;
}
