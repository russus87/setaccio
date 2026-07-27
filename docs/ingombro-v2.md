# Ingombro v2 — ordinare per recuperabilità, non per dimensione

Nota di design nata da una sessione di pulizia manuale del **27/07/2026**, fatta
a mano perché Setaccio non sapeva ancora farla. Il disco era all'**83%**; una
singola passata l'ha portato al **65%**, liberando **175 GB** reali. Qui sta cosa
ha reso possibile quella passata, e cosa manca a `Ingombro` v0.4.0 per arrivarci
da solo.

> **Attenzione ai numeri.** `du` valutava quelle stesse cartelle **411 GB**;
> `df` si è mosso di **175 GB**. Non è un errore di misura, è la differenza fra
> dimensione logica e occupazione reale su btrfs compresso — vedi
> *[Il trabocchetto della misura](#il-trabocchetto-della-misura-du-non-è-lo-spazio-che-recuperi)*,
> che è forse il requisito più importante di tutta questa nota.

## L'osservazione che la origina

La sezione `Ingombro` oggi mostra tre cose: i file più grandi, le cartelle che
pesano di più, la ripartizione per estensione. **Nessuna delle tre avrebbe fatto
emergere i 411 GB.**

Una classifica per dimensione, su questa macchina, mette in cima `~/vms` con
230 GB — che sono i dischi di due VM Windows, cioè esattamente la cosa da **non**
toccare. Il secondo posto sono 41 GB di modelli LLM: idem. Per arrivare alla
risposta giusta bisognava sapere un'altra cosa, che la dimensione non dice:

> quanto costa riavere indietro quello che cancello?

I 411 GB erano artefatti di build Rust. Costo di recupero: **zero** — un
`cargo build`, offline, senza toccare la rete, perché i sorgenti delle
dipendenze stanno già in `~/.cargo/registry`. Solo tempo di CPU.

Il caso limite misurato: `corso-c/pdf_accessibility_studio` aveva **11,2 MB di
sorgente** (2,5 MB di solo codice scritto a mano) e **40 GB di `target/`**.
Rapporto **3.688×**.

## Il concetto centrale: la classe di recuperabilità

È un nuovo valore sull'asse **stato**, che Setaccio già modella come ortogonale
a tipo e contesto. Quattro classi, ordinate per costo di recupero crescente:

| Classe | Costo di recupero | Esempi | Misurato qui (logico) |
|---|---|---|---|
| **1 — Rigenerabile offline** | solo CPU | `target/` Rust (deps già in `~/.cargo/registry`) | **411 GB** → 175 GB reali |
| **2 — Rigenerabile con rete** | CPU + banda + registry raggiungibile | `node_modules`, `.gradle/caches`, `.nuget`, `.npm`, browser di puppeteer/playwright | ~46 GB |
| **3 — Spazzatura** | niente da recuperare, mai | cache pacman/yay, coredump, cestino, `plocate.db` | ~28 GB |
| **4 — Dato vero** | irrecuperabile | VM, modelli LLM, ISO, foto | 280+ GB |

La regola operativa che ne discende: **la classe 4 non si propone mai per la
cancellazione, si riporta e basta.** Ordinare per dimensione risponde a "cosa
è grosso". Ordinare per dimensione *dentro la classe 1* risponde a "cosa
cancello stasera senza pensarci".

La distinzione 1 vs 2 non è pedanteria: cancellare `target/` su un portatile in
treno è gratis, cancellare `node_modules` no.

## Il trabocchetto della misura: `du` non è lo spazio che recuperi

Il requisito più importante emerso dalla sessione, perché è quello che decide se
lo strumento è credibile.

Le 34 build dir cancellate pesavano **411 GB secondo `du`**. A cancellazione
finita `df` era passato da 781 GB a 606 GB occupati: **175 GB reali**. Un fattore
**2,35×** di scarto.

La ragione: il filesystem è montato
`btrfs … compress=zstd:1`. I binari di debug — pagine di zeri, simboli DWARF
ripetitivi — si comprimono enormemente, quindi occupavano su disco molto meno
della loro dimensione logica. `du` riporta la dimensione logica; il gigabyte che
l'utente riguadagna è quello fisico.

Un secondo effetto, più insidioso perché sembra un bug: **btrfs non libera tutto
all'istante**. Le letture di `df` subito dopo la cancellazione davano 622 GB, poi
615, poi 606, mentre `btrfs-cleaner` smaltiva i riferimenti in background. Ci
sono voluti diversi minuti perché il numero si stabilizzasse.

Cosa ne discende per Setaccio:

- **Misurare l'occupazione reale degli extent**, non `st_blocks` ingenuo:
  `FIEMAP`/`ioctl` per gli extent compressi, come fa `compsize`. Su filesystem
  non compressi le due cifre coincidono e non cambia nulla.
- **Se la misura reale non è disponibile, dirlo.** Meglio "circa 411 GB logici,
  su questo filesystem probabilmente molto meno" che una promessa secca di 411 GB
  disattesa.
- **Non dichiarare il risultato leggendo `df` subito dopo**: attendere che si
  stabilizzi, o dichiarare esplicitamente che il conto è ancora in corso.
- La **barra di proiezione** del grafico va costruita sul numero reale, non su
  quello logico. Uno strumento che promette 411 e ne consegna 175 non viene
  usato una seconda volta.

## Il rilevatore esiste già

Il README descrive il criterio che Setaccio usa per distinguere i documenti
dagli artefatti: **risalire gli antenati del file finché non si trova un marker
di repository** (`.git`, `Cargo.toml`, `package.json`, `pom.xml`, `.csproj`,
`pubspec.yaml`).

Lo stesso identico meccanismo identifica le directory di build — e, cosa più
importante, **è il controllo di sicurezza**: una cartella è una build dir solo
se accanto ha il manifesto corrispondente.

| Directory | Manifesto fratello | Ecosistema | Dove stanno le dipendenze |
|---|---|---|---|
| `target/` | `Cargo.toml` | Rust | `~/.cargo/registry` → **offline** |
| `target/` | `pom.xml` | Maven | `~/.m2` |
| `build/` | `build.gradle` | Gradle | `~/.gradle/caches` |
| `node_modules/` | `package.json` | npm | `~/.npm` |
| `bin/`, `obj/` | `*.csproj` | .NET | `~/.nuget` |
| `build/`, `.dart_tool/` | `pubspec.yaml` | Flutter | `~/.pub-cache` |

Il 27/07 questo controllo si è ripagato da solo: ha correttamente **rifiutato
6 cartelle di nome `target`** che non avevano `Cargo.toml` ma `pom.xml` — erano
build Maven legittime, trattate a parte. Avrebbe rifiutato allo stesso modo una
cartella dell'utente che si chiama "target" per caso.

## Il rapporto sorgente/artefatto come ordinamento

**È il rapporto, non la dimensione assoluta, a segnalare il caso patologico.**
Un progetto con 2 GB di `target` su 500 MB di sorgente è fisiologico; uno con
17 GB su 1,3 MB ha accumulato immondizia per mesi.

E i due ordinamenti **non danno la stessa classifica** — è questo che rende la
vista utile invece che ridondante. Misurato (sorgente = `du -sb` escludendo
`node_modules` e `.git`):

| Progetto | Artefatto | Sorgente | Rapporto |
|---|---|---|---|
| `dotforge` | 16 GB | 1,3 MB | **12.783×** |
| `workpulse` | 18 GB | 1,4 MB | **12.673×** |
| `cortocircuito` | 12 GB | 1,0 MB | **12.141×** |
| `pinterest_crawler` | 12 GB | 1,1 MB | 10.662× |
| `lotus` | 15 GB | 1,5 MB | 10.485× |
| … | | | |
| `pdf_accessibility_studio` | **40 GB** | 11,2 MB | 3.688× |
| `charon` | 36 GB | 17,8 MB | 2.068× |

I due progetti **più grossi in assoluto** sono in fondo alla classifica per
rapporto: hanno tanto sorgente, quindi tanto `target` è fisiologico. I peggiori
in rapporto sono progetti piccoli che hanno ricompilato all'infinito. Sono due
domande diverse e servono entrambe.

Da affiancare: **data dell'ultima build** e **data dell'ultimo commit**. Il
progetto più grosso era fermo dal 29 giugno — un mese di 40 GB fermi lì.

## Le copie stantie — riusare il motore di dedup

Cargo non fa mai garbage collection: a ogni build scrive un binario nuovo con un
hash diverso in `deps/` e **non cancella il precedente**.

Misurato in `pdf_accessibility_studio`: **8.840 file** in `debug/deps`, di cui
**98 binari sopra i 50 MB**, e fra questi:

- `pdf_accessibility_studio` → **7 copie** da ~530 MB = 3,7 GB
- `pipeline`, `pdfa_core`, `pagine`, `genera_struttura` → 5 copie a testa
- `taggatura` → 4; `tabelle`, `form`, `artifact` → 3
- `libpdfa_lib.a` → 1,5 GB, presente in **due** copie identiche

Pesano mezzo giga l'uno perché sono binari di test linkati in debug con tutti i
simboli dentro.

Setaccio **ha già** il confronto per hash del contenuto, usato per i duplicati.
Stesso motore: *"N copie dello stesso artefatto, tieni la più recente"* è un
sotto-piano che recupera moltissimo **senza** un clean completo — utile quando
il progetto è attivo e non vuoi ricompilare da zero.

## I deliverable dentro l'albero di build

La sottigliezza che separa uno strumento usabile da uno di cui ti fidi:
**non tutto ciò che sta in una build dir è buttabile.**

Il 27/07, prima di cancellare, sono stati salvati a mano 5 artefatti veri da
`target/release/bundle/`:

- `Rustman_0.8.0_amd64.AppImage` (97 MB) e il `.deb`
- `dotforge_0.1.4_amd64.deb`
- `Mermaid Forge` `.deb` e `.rpm`

Regola: i percorsi tipo `*/target/release/bundle/**/*.{deb,AppImage,rpm,msi,exe}`
— e gli equivalenti `build/app/outputs/**/*.apk`, `bin/Release/**/*.nupkg` —
sono **output che potresti volere**. Il piano deve proporre di **spostarli da
parte**, non cancellarli in silenzio insieme al resto.

## Permessi: verificare prima, non fallire a metà

L'unico fallimento della sessione: `Progetti/norio/norioc-api/target` conteneva
**230 file di proprietà `root:root`**, scritti da un container Docker che girava
da root. `rm` è morto a metà con **187 errori di permesso**.

L'anteprima deve verificare la scrivibilità **dell'intero sottoalbero** e
dichiarare *"questa parte richiede privilegi elevati"* **prima** di mostrare il
piano. Altrimenti un piano che promette N GB ne consegna N−x, e l'utente lo
scopre da un muro di errori — che è esattamente il contrario della promessa
"niente parte senza che tu abbia visto il piano".

## Il grafico

Deve rispondere a colpo d'occhio a due domande insieme: **dove sono i gigabyte**
e **quali tornano indietro**.

**Treemap: area = dimensione, colore = classe di recuperabilità.**

Non la treemap classica alla WinDirStat, che colora per estensione e non dice
nulla di azionabile. Qui il colore porta l'asse che serve a decidere:

- **classe 1** (costo zero) — colore pieno, si legge subito come "prendi"
- **classe 2** (serve rete) — intermedio
- **classe 3** (spazzatura) — intermedio
- **classe 4** (dato vero) — neutro/grigio, visivamente "off limits"

Viste complementari:

- **Barra di proiezione** — occupazione attuale contro occupazione prevista
  applicando il piano selezionato. È ciò che rende leggibile un recupero di
  411 GB *prima* di eseguirlo: il 27/07 la barra sarebbe andata da 781 GB a
  622 GB in un colpo solo.
- **Per progetto** — barre orizzontali ordinate per rapporto sorgente/artefatto,
  non per dimensione assoluta.

E soprattutto: **selezionare un'area della treemap significa aggiungerla al
piano**, con la barra di proiezione che si aggiorna dal vivo. Così il grafico
non è decorazione, è **l'editor del piano**.

Tema chiaro/scuro: c'è già `tema.svelte.ts`.

## Dove tocca il codice

| File | Cosa |
|---|---|
| `src-tauri/src/ingombro.rs` | classi di recuperabilità, rapporto sorgente/artefatto, proiezione |
| `src-tauri/src/classify.rs` | i marker di repo stanno già qui — estenderli alle build dir |
| `src-tauri/src/dedupe.rs` | riuso per le copie stantie in `deps/` |
| `src-tauri/src/organize.rs`, `cestino.rs` | piano, anteprima, esecuzione, verifica permessi |
| `src/lib/views/` | vista treemap + barra di proiezione |

## Numeri di riferimento — misura reale del 27/07/2026

Utili come dataset di demo e come fixture di regressione.

| | |
|---|---|
| Disco | 950 GB, 781 GB usati (83%) → **606 GB (65%)** |
| Build dir rimosse | 34 su 35 (1 fallita per permessi) |
| **Recuperato, reale (`df`)** | **175 GB** |
| Recuperato, logico (`du`) | 411 GB — scarto **2,35×** per `compress=zstd` |
| Assestamento | 622 → 615 → 606 GB in ~10 min, `btrfs-cleaner` in background |
| `Documenti/corso-c` | 172 GB → 870 MB (logico) |
| `Documenti/Progetti` | 263 GB → 22 GB (logico) |
| Più grossi | `pdf_accessibility_studio` 40 G, `charon` 36 G, `space-station` 35 G, `devtoys` 31 G, `rustman` 26 G, `archmind` 23 G |
| Peggiori per rapporto | `dotforge` 12.783×, `workpulse` 12.673×, `cortocircuito` 12.141× |
| Copie stantie | 8.840 file in una sola `deps/`, 98 binari > 50 MB, fino a 7 copie dello stesso |
| Deliverable salvati | 5 bundle, 112 MB, da `target/release/bundle` |
| Spazzatura pura trovata | 28 GB (cache pacman 13 G, cestino 7,3 G, cache yay 5,1 G, plocate 1,7 G, 191 coredump 874 M) |

## Mockup

`mockups/ingombro-v2.html` — treemap navigabile con questi dati veri: area =
dimensione, colore = classe, click = composizione del piano, barra di proiezione
sul numero reale. Usa i token di `src/app.css`, quindi si può leggere anche come
riferimento di stile per la vista da costruire.
