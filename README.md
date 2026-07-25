<div align="center">

# Setaccio

**Indicizza, categorizza e cerca i file locali — separando i documenti veri dagli artefatti di build.**

Rust · Tauri 2 · Svelte 5

</div>

---

## Cosa fa

- **Indicizza** gli alberi di cartelle che gli indichi: nome, percorso, dimensione,
  date, hash del contenuto.
- **Categorizza** su assi ortogonali (tipo, contesto, stato) e per ogni file
  registra il *motivo* della classificazione, così il risultato è verificabile
  invece che magico.
- **Estrae il testo** da PDF, documenti Office, EPUB, ODT e dal contenuto degli
  archivi, e lo rende cercabile full-text in modo trasversale al formato.
- **Distingue i documenti dagli artefatti**: quello che c'è dentro un albero di
  codice sorgente non viene mescolato ai documenti veri.
- **Trova i duplicati** confrontando l'hash del contenuto, non il nome.
- **Dice dove sono finiti i gigabyte**: la sezione **Ingombro** mette accanto i
  file più grandi, le **cartelle che pesano di più** e la ripartizione per
  estensione. Da lì i file si mandano in quarantena, nel cestino di sistema o
  si cancellano — sempre passando dall'anteprima del piano.

## Perché esiste

Il punto di partenza è una misura, non un'intuizione. Un censimento su un
filesystem di sviluppo reale ha contato **3.341 documenti** (`pdf`, `docx`,
`xlsx`, `epub`, `odt`).
Di questi, **2.951 — l'88% — stanno dentro alberi di progetti di codice**: sono
fixture di test e output di esecuzioni, non documenti che qualcuno vuole
ritrovare. Qualsiasi ricerca desktop che li tratta alla pari restituisce
risultati per nove decimi inutili.

Le euristiche basate sul nome della cartella (`test`, `build`, `output`, `temp`…)
non bastano: su quei 2.951 file ne intercettano **33**. Il criterio che funziona
è un altro — **risalire gli antenati del file finché non si trova un marker di
repository**: `.git`, `.csproj`, `pom.xml`, `pubspec.yaml`, `Cargo.toml`,
`package.json`. Se il marker c'è, il file è un artefatto e lo si etichetta come
tale, citando la radice trovata.

Il secondo motivo è più di nicchia ma non meno concreto: i flussi di
composizione documentale producono **tracciati a record fissi** — file senza
estensione, a lunghezza di record costante, che nessuno strumento generico legge.
Setaccio li interpreta e li **correla per lotto**: tracciato ↔ PDF generati ↔ XML
di accompagnamento, così un lotto si guarda come un'unica cosa invece che come
tre mucchi di file scollegati.

## Sicurezza: niente parte senza che tu abbia visto il piano

L'ordine vive nell'indice, non nel filesystem: Setaccio in condizioni normali
è **sola lettura**, e nessuna operazione parte da sola.

- La modalità **Organizza** è **opt-in**, va attivata esplicitamente.
- Ogni operazione passa da un'**anteprima obbligatoria**: si vede prima cosa
  verrebbe toccato, quanto spazio si libera, e quali mosse verrebbero saltate
  e perché.
- Non si **sovrascrive mai**: una destinazione occupata è una mossa saltata con
  un avviso, non un conflitto risolto d'ufficio.

Dove finiscono i file lo decidi tu, e le tre strade non si disfano allo stesso
modo — la differenza resta visibile fino all'ultimo clic:

| | Cosa fa | Come si torna indietro |
|---|---|---|
| **Quarantena** | Sposta in una cartella dedicata dentro i dati dell'app, ricostruendo il percorso di origine | **Annulla** dall'elenco dei batch: i file tornano al loro posto |
| **Cestino** | Consegna al cestino di sistema | Dal gestore file, **non** da Setaccio |
| **Elimina** | Cancella dal disco | **Mai.** Per questo chiede anche di scrivere `ELIMINA` |

Le due operazioni distruttive vivono in un modulo a parte (`src/cestino.rs`):
è l'unico file del progetto in cui si perde qualcosa, ed è l'unico da rileggere
quando ci si chiede cosa possa distruggere dei dati. Il **canonico di un gruppo
di duplicati non si tocca mai**, e ciò che sta *dentro* un archivio non si
cancella dal disco: va tolto dall'archivio, che è un'altra operazione.

## Installazione

### Arch Linux

Scarica il `.pkg.tar.zst` dalla pagina
[Releases](https://github.com/russus87/setaccio/releases) e installalo:

```bash
sudo pacman -U setaccio-0.4.0-1-x86_64.pkg.tar.zst
```

### Windows

Scarica l'installer (`.msi` o `-setup.exe`) dalla pagina
[Releases](https://github.com/russus87/setaccio/releases) ed eseguilo.

### Altre distribuzioni Linux

Le release includono anche un `.AppImage` e un `.deb`.

## Sviluppo

```bash
npm install
npm run tauri dev      # avvia l'app in sviluppo
npm run check          # svelte-check (type checking)
npm run tauri build    # compila i bundle nativi
```

Servono **Rust** (stable), **Node 20+** e le dipendenze Tauri per Linux
(`webkit2gtk-4.1`, `gtk3`). SQLite è compilato staticamente dentro il binario
(`rusqlite` con feature `bundled`), quindi in compilazione serve anche un
compilatore C (`base-devel` su Arch, `build-essential` su Debian/Ubuntu).
L'estrazione del testo dai PDF è Rust puro (`pdf-extract`), con eventuale
fallback sul binario di sistema `pdftotext` se disponibile.

## Cosa non fa (v1)

Per onestà, i limiti dichiarati di questa versione:

- **niente OCR automatico** — i PDF senza testo estraibile restano fuori
  dall'indice full-text;
- **niente sync cloud** — l'indice è locale e ci resta;
- **niente indicizzazione del codice sorgente** — gli alberi di progetto vengono
  riconosciuti per escluderne gli artefatti, non per cercarci dentro.

## Licenza

MIT © russus
