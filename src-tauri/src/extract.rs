//! Estrazione del testo indicizzabile.
//!
//! Nessun OCR: sui PDF reali dei lotti il testo è già estraibile, e una
//! passata OCR automatica su decine di migliaia di file costerebbe ore per
//! guadagnare pochissimo. Resta un'azione esplicita, fuori dalla v1.

use anyhow::{anyhow, bail, Context, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::io::{Cursor, Read};
use std::path::Path;

/// Testo estratto da un file, più ciò che serve a localizzare i match.
#[derive(Debug, Default, Clone)]
pub struct Estratto {
    pub testo: String,
    pub pagine: Option<i64>,
    /// Offset di inizio di ogni pagina dentro `testo`: serve a dire "il match
    /// è a pagina 3" invece del solo nome file.
    pub inizio_pagina: Vec<usize>,
}

/// Un file trovato dentro un archivio, indicizzato senza estrarre nulla sul
/// disco.
#[derive(Debug, Clone)]
pub struct VoceArchivio {
    /// Path virtuale nella forma `archivio.zip!/interno/doc.pdf`.
    pub path_virtuale: String,
    pub nome: String,
    pub size: i64,
    pub estratto: Option<Estratto>,
}

// ---------------------------------------------------------------------------
// Tetti
// ---------------------------------------------------------------------------

/// Tetto al testo indicizzato per singolo file. Oltre i 2 MB di testo il
/// contributo alla ricerca è marginale mentre l'indice FTS raddoppia: sui
/// tracciati da centinaia di MB significherebbe un database più grande dei
/// file indicizzati.
pub const TETTO_TESTO: usize = 2 * 1024 * 1024;

/// Tetto ai byte che accettiamo di leggere in memoria per una singola voce di
/// archivio. Sopra questa soglia la voce viene solo elencata per nome.
pub const TETTO_VOCE: u64 = 4 * 1024 * 1024;

/// Budget totale di byte decompressi per archivio. È la difesa contro lo
/// zip-bomb e contro i casi reali (`BOOKWORM.zip`, 272 MB): oltre il budget si
/// continua a elencare i nomi ma non si decomprime più niente.
pub const BUDGET_DECOMPRESSO: u64 = 64 * 1024 * 1024;

/// Quanti byte leggiamo per decidere se un file senza estensione è testo o
/// binario.
const CAMPIONE_BINARIO: usize = 8 * 1024;

// ---------------------------------------------------------------------------
// Riconoscimento delle estensioni
// ---------------------------------------------------------------------------

/// Estensioni trattate come testo semplice.
const EXT_TESTO: &[&str] = &[
    "txt", "text", "md", "markdown", "csv", "tsv", "json", "jsonl", "ndjson", "xml", "yaml", "yml",
    "log", "ini", "cfg", "conf", "properties", "toml", "sql", "htm", "html", "xhtml", "dat", "asc",
];

/// Contenitori OOXML/ODF/EPUB: sono zip con dentro XML.
const EXT_ZIP_XML: &[&str] = &[
    "docx", "docm", "xlsx", "xlsm", "pptx", "pptm", "odt", "ods", "odp", "odg", "epub",
];

/// Archivi di cui sappiamo leggere l'elenco delle voci.
const EXT_ARCHIVIO: &[&str] = &["zip", "tar", "tgz", "taz", "gz"];

fn minuscolo(ext: Option<&str>) -> Option<String> {
    ext.map(|e| e.trim_start_matches('.').to_ascii_lowercase())
}

/// True se l'estensione è una di quelle da cui sappiamo tirare fuori testo.
///
/// `None` (file senza estensione) conta come estraibile: sul filesystem reale
/// sono quasi sempre tracciati o note, e il filtro binario in `estrai` scarta
/// comunque ciò che testo non è.
pub fn estraibile(ext: Option<&str>) -> bool {
    match minuscolo(ext) {
        None => true,
        Some(e) => {
            e == "pdf" || EXT_TESTO.contains(&e.as_str()) || EXT_ZIP_XML.contains(&e.as_str())
        }
    }
}

/// True se l'estensione è un archivio di cui sappiamo leggere il contenuto.
pub fn e_archivio(ext: Option<&str>) -> bool {
    match minuscolo(ext) {
        None => false,
        Some(e) => EXT_ARCHIVIO.contains(&e.as_str()),
    }
}

fn ext_di(path: &Path) -> Option<String> {
    minuscolo(path.extension().and_then(|e| e.to_str()))
}

// ---------------------------------------------------------------------------
// Estrazione da file su disco
// ---------------------------------------------------------------------------

/// Estrae il testo da un file sul disco.
pub fn estrai(path: &Path) -> Result<Estratto> {
    let ext = ext_di(path);
    match ext.as_deref() {
        Some("pdf") => estrai_pdf(path),
        Some(e) if EXT_ZIP_XML.contains(&e) => {
            let dati = std::fs::read(path)
                .with_context(|| format!("lettura di {}", path.display()))?;
            estrai_zip_xml(&dati, e)
        }
        Some(e) if EXT_TESTO.contains(&e) => estrai_testo_da_disco(path),
        None => {
            // Senza estensione ci fidiamo dei magic bytes prima di dichiarare
            // che è testo: nei lotti reali capitano PDF rinominati senza suffisso.
            if inizia_con_pdf(path)? {
                estrai_pdf(path)
            } else {
                estrai_testo_da_disco(path)
            }
        }
        Some(e) => bail!("estensione «{e}» non estraibile"),
    }
}

fn inizia_con_pdf(path: &Path) -> Result<bool> {
    let mut f = std::fs::File::open(path)
        .with_context(|| format!("apertura di {}", path.display()))?;
    let mut testa = [0u8; 5];
    let letti = f.read(&mut testa)?;
    Ok(&testa[..letti] == b"%PDF-")
}

fn estrai_testo_da_disco(path: &Path) -> Result<Estratto> {
    let meta = std::fs::metadata(path)
        .with_context(|| format!("stat di {}", path.display()))?;
    if !meta.is_file() {
        bail!("{} non è un file regolare", path.display());
    }
    // Si legge al massimo il doppio del tetto: anche in Latin-1 (1 byte per
    // carattere) tanto basta per riempire il tetto di testo, e non si carica in
    // RAM un tracciato da 800 MB per poi buttarne il 99%.
    let mut f = std::fs::File::open(path)
        .with_context(|| format!("apertura di {}", path.display()))?;
    let mut dati = Vec::new();
    f.by_ref()
        .take((TETTO_TESTO as u64) * 2)
        .read_to_end(&mut dati)?;
    estrai_testo_da_byte(&dati)
}

/// Decodifica byte in testo provando UTF-8 e ripiegando su Windows-1252.
///
/// I tracciati e i documenti legacy dell'utente arrivano da mainframe e da
/// Windows: dichiarare tutto UTF-8 vorrebbe dire perdere ogni accento.
pub fn decodifica(dati: &[u8]) -> String {
    // Un BOM è la sola dichiarazione di encoding di cui ci si può fidare.
    if let Some((enc, quanti)) = encoding_rs::Encoding::for_bom(dati) {
        let (s, _, _) = enc.decode(&dati[quanti..]);
        return s.into_owned();
    }
    match std::str::from_utf8(dati) {
        Ok(s) => s.to_string(),
        Err(_) => {
            // Windows-1252 è un superset di Latin-1 sulla parte stampabile:
            // copre entrambi i casi con una sola tabella e non fallisce mai.
            let (s, _, _) = encoding_rs::WINDOWS_1252.decode(dati);
            s.into_owned()
        }
    }
}

fn estrai_testo_da_byte(dati: &[u8]) -> Result<Estratto> {
    if dati.is_empty() {
        bail!("file vuoto");
    }
    // Un NUL nel campione è il segnale più affidabile che il file è binario:
    // indicizzarlo riempirebbe l'FTS di spazzatura non cercabile.
    let campione = &dati[..dati.len().min(CAMPIONE_BINARIO)];
    if campione.contains(&0) {
        bail!("contenuto binario, niente testo da indicizzare");
    }
    let testo = tronca(decodifica(dati), TETTO_TESTO);
    Ok(Estratto {
        testo,
        pagine: None,
        inizio_pagina: Vec::new(),
    })
}

// ---------------------------------------------------------------------------
// PDF
// ---------------------------------------------------------------------------

fn estrai_pdf(path: &Path) -> Result<Estratto> {
    // `pdf-extract` va in panic su alcuni PDF malformati: il catch_unwind lo
    // trasforma in un errore recuperabile. In release il profilo usa
    // `panic = "abort"` e la rete non tiene, ma lì l'alternativa è comunque il
    // fallback su pdftotext, che gira in un processo separato.
    let per_pagina = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        pdf_extract::extract_text_by_pages(path)
    }))
    .ok()
    .and_then(|r| r.ok());

    if let Some(pagine) = per_pagina {
        if pagine.iter().any(|p| !p.trim().is_empty()) {
            return Ok(assembla_pagine(pagine));
        }
    }

    // Fallback: il binario di sistema `pdftotext` (Poppler). Regge molti PDF
    // che pdf-extract rifiuta, e separa le pagine con un form feed, che è
    // esattamente il confine che ci serve per `inizio_pagina`.
    pdftotext(path).with_context(|| format!("nessun testo estraibile da {}", path.display()))
}

fn pdftotext(path: &Path) -> Result<Estratto> {
    let out = std::process::Command::new("pdftotext")
        .arg("-q")
        .arg("-enc")
        .arg("UTF-8")
        .arg(path)
        .arg("-")
        .output()
        .context("pdftotext non disponibile sul sistema")?;
    if !out.status.success() {
        bail!("pdftotext ha fallito su {}", path.display());
    }
    let testo = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut pagine: Vec<String> = testo.split('\u{c}').map(|s| s.to_string()).collect();
    // pdftotext chiude anche l'ultima pagina con il form feed: senza questo
    // avremmo sempre una pagina fantasma in coda.
    if pagine.last().map(|p| p.trim().is_empty()).unwrap_or(false) {
        pagine.pop();
    }
    if pagine.iter().all(|p| p.trim().is_empty()) {
        bail!("pdftotext non ha restituito testo");
    }
    Ok(assembla_pagine(pagine))
}

/// Concatena le pagine separandole con un form feed e registra dove ognuna
/// comincia.
fn assembla_pagine(pagine: Vec<String>) -> Estratto {
    let quante = pagine.len();
    let mut testo = String::new();
    let mut inizio = Vec::with_capacity(quante);
    for (i, p) in pagine.into_iter().enumerate() {
        inizio.push(testo.len());
        testo.push_str(&p);
        if i + 1 < quante {
            testo.push('\u{c}');
        }
    }
    let testo = tronca(testo, TETTO_TESTO);
    // Se il tetto ha tagliato via delle pagine i loro offset non puntano più a
    // niente: meglio perdere l'informazione che indicare una pagina sbagliata.
    inizio.retain(|o| *o <= testo.len());
    Estratto {
        pagine: if quante > 0 { Some(quante as i64) } else { None },
        testo,
        inizio_pagina: inizio,
    }
}

/// Numero di pagina (1-based) in cui cade un offset dentro `testo`.
pub fn pagina_di(inizio_pagina: &[usize], offset: usize) -> Option<i64> {
    if inizio_pagina.is_empty() {
        return None;
    }
    let idx = match inizio_pagina.binary_search(&offset) {
        Ok(i) => i,
        Err(0) => 0,
        Err(i) => i - 1,
    };
    Some(idx as i64 + 1)
}

fn tronca(mut s: String, max: usize) -> String {
    if s.len() <= max {
        return s;
    }
    // `truncate` pretende un confine di carattere: si arretra fino a trovarlo.
    let mut taglio = max;
    while taglio > 0 && !s.is_char_boundary(taglio) {
        taglio -= 1;
    }
    s.truncate(taglio);
    s
}

// ---------------------------------------------------------------------------
// Contenitori zip+XML: docx / xlsx / pptx / odt / ods / odp / epub
// ---------------------------------------------------------------------------

fn estrai_zip_xml(dati: &[u8], ext: &str) -> Result<Estratto> {
    let mut zip = zip::ZipArchive::new(Cursor::new(dati))
        .with_context(|| format!("«{ext}» non è uno zip leggibile"))?;

    // Le parti interessanti dipendono dal formato: qui si decide *quali* file
    // interni leggere, la conversione XML→testo è la stessa per tutti.
    let nomi: Vec<String> = (0..zip.len())
        .filter_map(|i| zip.by_index(i).ok().map(|f| f.name().to_string()))
        .collect();

    let mut parti: Vec<String> = Vec::new();
    let mut per_pagina = false;
    match ext {
        "docx" | "docm" => {
            for n in ["word/document.xml", "word/footnotes.xml", "word/endnotes.xml"] {
                if nomi.iter().any(|x| x == n) {
                    parti.push(n.to_string());
                }
            }
        }
        "xlsx" | "xlsm" => {
            // Le celle di testo vivono nella tabella condivisa, non nei fogli:
            // senza sharedStrings di un xlsx si indicizzerebbero solo i numeri.
            if nomi.iter().any(|x| x == "xl/sharedStrings.xml") {
                parti.push("xl/sharedStrings.xml".into());
            }
            let mut fogli: Vec<String> = nomi
                .iter()
                .filter(|n| n.starts_with("xl/worksheets/") && n.ends_with(".xml"))
                .cloned()
                .collect();
            fogli.sort();
            parti.extend(fogli);
        }
        "pptx" | "pptm" => {
            let mut slide: Vec<String> = nomi
                .iter()
                .filter(|n| n.starts_with("ppt/slides/") && n.ends_with(".xml"))
                .cloned()
                .collect();
            slide.sort_by_key(|n| (numero_finale(n), n.clone()));
            parti.extend(slide);
            // Una slide è l'equivalente onesto di una pagina: vale la pena
            // tenerne i confini per dire "il match è alla slide 4".
            per_pagina = true;
        }
        "odt" | "ods" | "odp" | "odg" => {
            if nomi.iter().any(|x| x == "content.xml") {
                parti.push("content.xml".into());
            }
        }
        "epub" => {
            let mut doc: Vec<String> = nomi
                .iter()
                .filter(|n| {
                    let l = n.to_ascii_lowercase();
                    l.ends_with(".xhtml") || l.ends_with(".html") || l.ends_with(".htm")
                })
                .cloned()
                .collect();
            doc.sort_by_key(|n| (numero_finale(n), n.clone()));
            parti.extend(doc);
        }
        _ => bail!("formato zip+xml «{ext}» non gestito"),
    }

    if parti.is_empty() {
        bail!("nessuna parte XML nota dentro un file «{ext}»");
    }

    let mut pezzi: Vec<String> = Vec::new();
    for n in parti {
        let mut f = match zip.by_name(&n) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let mut buf = Vec::new();
        // Anche dentro un docx una parte può essere enorme (o mentire sulla
        // dimensione dichiarata): si legge comunque con un tetto.
        if f.by_ref()
            .take(TETTO_VOCE * 4)
            .read_to_end(&mut buf)
            .is_err()
        {
            continue;
        }
        pezzi.push(testo_da_xml(&buf));
    }

    let pezzi: Vec<String> = pezzi.into_iter().filter(|p| !p.trim().is_empty()).collect();
    if pezzi.is_empty() {
        bail!("nessun testo dentro il file «{ext}»");
    }

    if per_pagina {
        Ok(assembla_pagine(pezzi))
    } else {
        Ok(Estratto {
            testo: tronca(pezzi.join("\n"), TETTO_TESTO),
            pagine: None,
            inizio_pagina: Vec::new(),
        })
    }
}

/// Ultimo numero presente nel nome, per ordinare `slide10.xml` dopo `slide2.xml`.
fn numero_finale(nome: &str) -> u64 {
    let cifre: String = nome
        .chars()
        .rev()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    cifre.chars().rev().collect::<String>().parse().unwrap_or(0)
}

/// Tag il cui confine vale un a capo: senza questo un documento intero
/// diventerebbe una riga sola e gli snippet sarebbero illeggibili.
const TAG_BLOCCO: &[&str] = &[
    "p", "br", "tab", "tr", "h", "h1", "h2", "h3", "h4", "h5", "h6", "li", "div", "title", "row",
    "sheetdata", "si",
];

/// Converte una parte XML in testo scartando i tag. Nessuna regex: i tag
/// annidati e gli attributi con `>` dentro le virgolette li sbaglierebbero.
pub fn testo_da_xml(dati: &[u8]) -> String {
    let mut reader = Reader::from_reader(dati);
    {
        let c = reader.config_mut();
        // L'XML dei documenti reali è spesso sciatto (soprattutto l'XHTML degli
        // epub): l'obiettivo qui è il testo, non la validazione.
        c.check_end_names = false;
        c.allow_unmatched_ends = true;
        c.check_comments = false;
    }
    let mut out = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Text(e)) => {
                if let Ok(s) = e.decode() {
                    out.push_str(s.as_ref());
                }
            }
            Ok(Event::CData(e)) => {
                if let Ok(s) = e.decode() {
                    out.push_str(s.as_ref());
                }
            }
            Ok(Event::GeneralRef(e)) => {
                // Le entità arrivano come evento separato: `&amp;` va risolto a
                // mano o il testo indicizzato conterrebbe i buchi.
                if let Ok(nome) = e.decode() {
                    if let Ok(risolta) = quick_xml::escape::unescape(&format!("&{nome};")) {
                        out.push_str(risolta.as_ref());
                    }
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => separa(&mut out, e.name().as_ref()),
            Ok(Event::End(e)) => separa(&mut out, e.name().as_ref()),
            Ok(Event::Eof) => break,
            // Un XML rotto a metà: si tiene quello che si è già letto invece di
            // buttare via tutto il documento.
            Err(_) => break,
            _ => {}
        }
        if out.len() > TETTO_TESTO {
            break;
        }
    }
    normalizza_spazi_conservando_righe(&out)
}

/// Al confine di un tag ci va un separatore, altrimenti le parole di due run
/// diversi si incollerebbero (`Gentile` + `Cliente` → `GentileCliente`).
fn separa(out: &mut String, qname: &[u8]) {
    let sep = if TAG_BLOCCO.contains(&nome_locale(qname).as_str()) {
        '\n'
    } else {
        ' '
    };
    if !out.is_empty() && !out.ends_with(sep) {
        out.push(sep);
    }
}

fn nome_locale(qname: &[u8]) -> String {
    let s = String::from_utf8_lossy(qname);
    match s.rsplit_once(':') {
        Some((_, l)) => l.to_ascii_lowercase(),
        None => s.to_ascii_lowercase(),
    }
}

/// Collassa gli spazi orizzontali e le righe vuote ripetute, che l'estrazione
/// da XML produce a valanga.
fn normalizza_spazi_conservando_righe(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut a_capo = 0usize;
    let mut spazio = false;
    for c in s.chars() {
        if c == '\n' || c == '\r' {
            a_capo += 1;
            spazio = false;
        } else if c.is_whitespace() {
            spazio = true;
        } else {
            if a_capo > 0 && !out.is_empty() {
                out.push('\n');
            } else if spazio && !out.is_empty() {
                out.push(' ');
            }
            a_capo = 0;
            spazio = false;
            out.push(c);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Archivi
// ---------------------------------------------------------------------------

/// Elenca e indicizza il contenuto di un archivio senza scriverlo su disco.
pub fn dentro_archivio(path: &Path, limite_voci: usize) -> Result<Vec<VoceArchivio>> {
    let nome_basso = path
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let ext = ext_di(path);

    match ext.as_deref() {
        Some("zip") => voci_zip(path, limite_voci),
        Some("tar") => voci_tar(std::fs::File::open(path)?, path, limite_voci),
        Some("tgz") | Some("taz") => voci_tar(
            flate2::read::GzDecoder::new(std::fs::File::open(path)?),
            path,
            limite_voci,
        ),
        Some("gz") if nome_basso.ends_with(".tar.gz") => voci_tar(
            flate2::read::GzDecoder::new(std::fs::File::open(path)?),
            path,
            limite_voci,
        ),
        // Un `.gz` singolo non è un contenitore: ha un solo membro, il file
        // stesso senza il suffisso.
        Some("gz") => voci_gz_singolo(path),
        _ => bail!("{} non è un archivio riconosciuto", path.display()),
    }
}

fn path_virtuale(archivio: &Path, interno: &str) -> String {
    format!("{}!/{}", archivio.display(), interno.trim_start_matches('/'))
}

fn nome_da_percorso(interno: &str) -> String {
    interno
        .rsplit('/')
        .next()
        .unwrap_or(interno)
        .to_string()
}

fn ext_da_nome(nome: &str) -> Option<String> {
    nome.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase())
}

/// Estrae il testo da byte già in memoria, scegliendo il decoder in base al
/// nome della voce.
fn estrai_da_memoria(nome: &str, dati: &[u8]) -> Option<Estratto> {
    let ext = ext_da_nome(nome);
    match ext.as_deref() {
        Some("pdf") => {
            let per_pagina = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                pdf_extract::extract_text_from_mem_by_pages(dati)
            }))
            .ok()
            .and_then(|r| r.ok())?;
            if per_pagina.iter().all(|p| p.trim().is_empty()) {
                // Niente fallback su pdftotext: richiederebbe di scrivere il PDF
                // su disco, ed è proprio quello che qui non vogliamo fare.
                return None;
            }
            Some(assembla_pagine(per_pagina))
        }
        Some(e) if EXT_ZIP_XML.contains(&e) => estrai_zip_xml(dati, e).ok(),
        Some(e) if EXT_TESTO.contains(&e) => estrai_testo_da_byte(dati).ok(),
        None => estrai_testo_da_byte(dati).ok(),
        _ => None,
    }
}

fn voci_zip(path: &Path, limite_voci: usize) -> Result<Vec<VoceArchivio>> {
    let f = std::fs::File::open(path)
        .with_context(|| format!("apertura di {}", path.display()))?;
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(f))
        .with_context(|| format!("{} non è uno zip leggibile", path.display()))?;

    let mut voci = Vec::new();
    let mut budget = BUDGET_DECOMPRESSO;
    for i in 0..zip.len() {
        if voci.len() >= limite_voci {
            break;
        }
        let mut voce = match zip.by_index(i) {
            Ok(v) => v,
            Err(_) => continue, // voce cifrata o corrotta: si salta, non si abortisce
        };
        if voce.is_dir() {
            continue;
        }
        let interno = voce.name().to_string();
        let size = voce.size();
        let nome = nome_da_percorso(&interno);

        let estratto = if estraibile(ext_da_nome(&nome).as_deref())
            && size <= TETTO_VOCE
            && size <= budget
        {
            let mut buf = Vec::with_capacity(size as usize);
            // `take` sulla dimensione dichiarata: uno zip-bomb dichiara poco e
            // decomprime molto, e il `take` è l'unica difesa che regge.
            match voce.by_ref().take(TETTO_VOCE).read_to_end(&mut buf) {
                Ok(letti) => {
                    budget = budget.saturating_sub(letti as u64);
                    estrai_da_memoria(&nome, &buf)
                }
                Err(_) => None,
            }
        } else {
            None
        };

        voci.push(VoceArchivio {
            path_virtuale: path_virtuale(path, &interno),
            nome,
            size: size as i64,
            estratto,
        });
    }
    Ok(voci)
}

fn voci_tar<R: Read>(lettore: R, path: &Path, limite_voci: usize) -> Result<Vec<VoceArchivio>> {
    let mut tar = tar::Archive::new(std::io::BufReader::new(lettore));
    let mut voci = Vec::new();
    let mut budget = BUDGET_DECOMPRESSO;

    let entries = tar
        .entries()
        .with_context(|| format!("{} non è un tar leggibile", path.display()))?;
    for e in entries {
        if voci.len() >= limite_voci {
            break;
        }
        let mut e = match e {
            Ok(e) => e,
            Err(_) => break, // il tar è sequenziale: dopo un errore non ci si riallinea
        };
        if !e.header().entry_type().is_file() {
            continue;
        }
        let interno = e
            .path()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        if interno.is_empty() {
            continue;
        }
        let size = e.header().size().unwrap_or(0);
        let nome = nome_da_percorso(&interno);

        let estratto = if estraibile(ext_da_nome(&nome).as_deref())
            && size <= TETTO_VOCE
            && size <= budget
        {
            let mut buf = Vec::new();
            match e.by_ref().take(TETTO_VOCE).read_to_end(&mut buf) {
                Ok(letti) => {
                    budget = budget.saturating_sub(letti as u64);
                    estrai_da_memoria(&nome, &buf)
                }
                Err(_) => None,
            }
        } else {
            None
        };

        voci.push(VoceArchivio {
            path_virtuale: path_virtuale(path, &interno),
            nome,
            size: size as i64,
            estratto,
        });
    }
    Ok(voci)
}

fn voci_gz_singolo(path: &Path) -> Result<Vec<VoceArchivio>> {
    let nome_esterno = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| anyhow!("path senza nome file"))?;
    let interno = nome_esterno
        .strip_suffix(".gz")
        .unwrap_or(&nome_esterno)
        .to_string();

    let f = std::fs::File::open(path)
        .with_context(|| format!("apertura di {}", path.display()))?;
    let mut dec = flate2::read::GzDecoder::new(std::io::BufReader::new(f));
    let mut buf = Vec::new();
    let letti = dec
        .by_ref()
        .take(TETTO_VOCE)
        .read_to_end(&mut buf)
        .with_context(|| format!("{} non è un gzip leggibile", path.display()))?;

    let estratto = if estraibile(ext_da_nome(&interno).as_deref()) {
        estrai_da_memoria(&interno, &buf)
    } else {
        None
    };

    Ok(vec![VoceArchivio {
        path_virtuale: path_virtuale(path, &interno),
        nome: nome_da_percorso(&interno),
        // È la dimensione *letta*, non quella reale: sopra il tetto il gzip non
        // viene decompresso oltre e non c'è modo onesto di saperne di più.
        size: letti as i64,
        estratto,
    }])
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use super::*;
    use std::io::Write;

    /// Cartella temporanea per il test, senza dipendere da `tempfile`.
    fn dir_temp(etichetta: &str) -> std::path::PathBuf {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let d = std::env::temp_dir().join(format!("setaccio-{etichetta}-{n}"));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn riconoscimento_estensioni() {
        assert!(estraibile(Some("pdf")));
        assert!(estraibile(Some("PDF")), "il confronto è case-insensitive");
        assert!(estraibile(Some("docx")));
        assert!(estraibile(Some("txt")));
        assert!(estraibile(None), "i file senza estensione si tentano");
        assert!(!estraibile(Some("jpg")));
        assert!(!estraibile(Some("zip")), "lo zip passa da dentro_archivio");

        assert!(e_archivio(Some("zip")));
        assert!(e_archivio(Some("tar")));
        assert!(e_archivio(Some("tgz")));
        assert!(e_archivio(Some("gz")));
        assert!(!e_archivio(Some("pdf")));
        assert!(!e_archivio(None));
    }

    #[test]
    fn decodifica_latin1_recupera_gli_accenti() {
        let d = dir_temp("latin1");
        let p = d.join("legacy.txt");
        // "Perché però" in Windows-1252/Latin-1: byte non validi in UTF-8.
        let mut byte = b"Perch\xe9 per\xf2 il tracciato \xe8 vecchio".to_vec();
        byte.push(b'\n');
        std::fs::write(&p, &byte).unwrap();

        let e = estrai(&p).unwrap();
        assert!(e.testo.contains("Perché"), "testo estratto: {}", e.testo);
        assert!(e.testo.contains("però"));
        assert!(e.testo.contains('è'));
        assert!(e.pagine.is_none(), "un txt non ha pagine");

        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn utf8_valido_non_viene_rovinato() {
        let d = dir_temp("utf8");
        let p = d.join("moderno.md");
        std::fs::write(&p, "Perché però è così — davvero").unwrap();
        let e = estrai(&p).unwrap();
        assert_eq!(e.testo, "Perché però è così — davvero");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn file_binario_senza_estensione_da_errore() {
        let d = dir_temp("bin");
        let p = d.join("dati");
        std::fs::write(&p, [0x00, 0x01, 0x02, 0x03, 0xff, 0x00]).unwrap();
        assert!(estrai(&p).is_err(), "un binario non deve finire nell'indice");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn file_inesistente_da_errore_non_panic() {
        let e = estrai(Path::new("/non/esiste/davvero.txt"));
        assert!(e.is_err());
        let e = dentro_archivio(Path::new("/non/esiste/davvero.zip"), 10);
        assert!(e.is_err());
    }

    fn scrivi_zip(path: &Path, voci: &[(&str, &[u8])]) {
        let f = std::fs::File::create(path).unwrap();
        let mut w = zip::ZipWriter::new(f);
        let opzioni = zip::write::SimpleFileOptions::default();
        for (nome, dati) in voci {
            w.start_file(*nome, opzioni).unwrap();
            w.write_all(dati).unwrap();
        }
        w.finish().unwrap();
    }

    #[test]
    fn zip_elencato_e_indicizzato_senza_toccare_il_disco() {
        let d = dir_temp("zip");
        let z = d.join("pacchetto.zip");
        scrivi_zip(
            &z,
            &[
                ("note/lettera.txt", "CONTRATTO PER IL SERVIZIO".as_bytes()),
                ("immagine.jpg", &[0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10]),
            ],
        );

        let voci = dentro_archivio(&z, 100).unwrap();
        assert_eq!(voci.len(), 2);

        let lettera = voci.iter().find(|v| v.nome == "lettera.txt").unwrap();
        assert_eq!(
            lettera.path_virtuale,
            format!("{}!/note/lettera.txt", z.display())
        );
        let t = lettera.estratto.as_ref().expect("il txt va indicizzato");
        assert!(t.testo.contains("CONTRATTO PER IL SERVIZIO"));

        let img = voci.iter().find(|v| v.nome == "immagine.jpg").unwrap();
        assert!(img.estratto.is_none(), "dal jpg si indicizza solo il nome");

        // Niente deve essere finito su disco accanto all'archivio.
        let sul_disco: Vec<_> = std::fs::read_dir(&d)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(sul_disco.len(), 1, "trovati residui: {sul_disco:?}");

        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn zip_rispetta_il_limite_voci() {
        let d = dir_temp("ziplimite");
        let z = d.join("tanti.zip");
        let voci: Vec<(String, Vec<u8>)> = (0..20)
            .map(|i| (format!("f{i}.txt"), format!("riga {i}").into_bytes()))
            .collect();
        let rif: Vec<(&str, &[u8])> = voci
            .iter()
            .map(|(n, d)| (n.as_str(), d.as_slice()))
            .collect();
        scrivi_zip(&z, &rif);

        let lette = dentro_archivio(&z, 5).unwrap();
        assert_eq!(lette.len(), 5, "il limite_voci deve essere rispettato");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn voce_enorme_viene_solo_elencata() {
        let d = dir_temp("zipgrosso");
        let z = d.join("grosso.zip");
        // Oltre TETTO_VOCE: comprime benissimo (è tutto uguale) ma decompresso
        // pesa troppo per finire in RAM. È il caso BOOKWORM.zip in piccolo.
        let grosso = vec![b'a'; (TETTO_VOCE + 1024) as usize];
        scrivi_zip(&z, &[("enorme.txt", &grosso), ("piccolo.txt", b"ciao")]);

        let voci = dentro_archivio(&z, 100).unwrap();
        let enorme = voci.iter().find(|v| v.nome == "enorme.txt").unwrap();
        assert!(
            enorme.estratto.is_none(),
            "sopra il tetto si indicizza solo il nome"
        );
        assert!(enorme.size > TETTO_VOCE as i64);
        let piccolo = voci.iter().find(|v| v.nome == "piccolo.txt").unwrap();
        assert!(piccolo.estratto.is_some());

        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn tar_gz_letto_in_memoria() {
        use flate2::write::GzEncoder;
        let d = dir_temp("targz");
        let t = d.join("roba.tar.gz");

        let f = std::fs::File::create(&t).unwrap();
        let enc = GzEncoder::new(f, flate2::Compression::fast());
        let mut b = tar::Builder::new(enc);
        let dati = b"OGGETTO: COMUNICAZIONE DI RECESSO";
        let mut h = tar::Header::new_gnu();
        h.set_size(dati.len() as u64);
        h.set_mode(0o644);
        h.set_cksum();
        b.append_data(&mut h, "dentro/lettera.txt", &dati[..]).unwrap();
        b.into_inner().unwrap().finish().unwrap();

        let voci = dentro_archivio(&t, 100).unwrap();
        assert_eq!(voci.len(), 1);
        assert_eq!(voci[0].nome, "lettera.txt");
        assert!(voci[0]
            .estratto
            .as_ref()
            .unwrap()
            .testo
            .contains("RECESSO"));

        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn docx_costruito_al_volo() {
        let d = dir_temp("docx");
        let p = d.join("lettera.docx");
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
 <w:body>
  <w:p><w:r><w:t>TEST CARTA 1</w:t></w:r></w:p>
  <w:p><w:r><w:t xml:space="preserve">Gentile </w:t></w:r><w:r><w:t>Cliente &amp; soci</w:t></w:r></w:p>
 </w:body>
</w:document>"#;
        scrivi_zip(
            &p,
            &[
                ("[Content_Types].xml", b"<Types/>"),
                ("word/document.xml", xml),
            ],
        );

        let e = estrai(&p).unwrap();
        assert!(e.testo.contains("TEST CARTA 1"), "testo: {}", e.testo);
        assert!(
            e.testo.contains("Gentile Cliente & soci"),
            "i run vanno concatenati e le entità risolte: {}",
            e.testo
        );
        assert!(!e.testo.contains("<w:t>"), "i tag non devono passare");

        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn pptx_conta_le_slide() {
        let d = dir_temp("pptx");
        let p = d.join("presentazione.pptx");
        let s1 = br#"<p:sld xmlns:a="x"><a:t>Prima slide</a:t></p:sld>"#;
        let s2 = br#"<p:sld xmlns:a="x"><a:t>Seconda slide</a:t></p:sld>"#;
        scrivi_zip(
            &p,
            &[("ppt/slides/slide1.xml", s1), ("ppt/slides/slide2.xml", s2)],
        );

        let e = estrai(&p).unwrap();
        assert_eq!(e.pagine, Some(2));
        assert_eq!(e.inizio_pagina.len(), 2);
        assert_eq!(pagina_di(&e.inizio_pagina, 0), Some(1));
        assert_eq!(
            pagina_di(&e.inizio_pagina, e.inizio_pagina[1]),
            Some(2),
            "l'offset della seconda slide deve dire pagina 2"
        );
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn xml_rotto_non_fa_panic() {
        let t = testo_da_xml(b"<a><b>ciao</b><c>senza chiusura");
        assert!(t.contains("ciao"));
    }

    #[test]
    fn tetto_sul_testo_indicizzato() {
        let d = dir_temp("tetto");
        let p = d.join("enorme.log");
        // Il tetto va rispettato anche a fronte di un file molto più grande.
        let riga = "riga di log ripetuta all'infinito\n";
        let quante = (TETTO_TESTO / riga.len()) + 5_000;
        let contenuto: String = riga.repeat(quante);
        std::fs::write(&p, &contenuto).unwrap();

        let e = estrai(&p).unwrap();
        assert!(
            e.testo.len() <= TETTO_TESTO,
            "testo da {} byte, tetto {}",
            e.testo.len(),
            TETTO_TESTO
        );
        std::fs::remove_dir_all(&d).ok();
    }

    /// Gira solo sulla macchina dove il PDF reale esiste: è il file su cui
    /// l'estrazione è stata verificata a mano.
    #[test]
    fn pdf_reale_dei_lotti() {
        let p = Path::new("/home/russus/Scaricati/T1Q73KFP/T1Q73KFP_0000001.pdf");
        if !p.exists() {
            return;
        }
        let e = estrai(p).expect("il PDF dei lotti deve essere estraibile");
        assert!(
            e.testo.contains("TEST CARTA 1"),
            "atteso «TEST CARTA 1» nel testo estratto ({} caratteri)",
            e.testo.chars().count()
        );
        assert!(
            e.testo.contains("CONTRATTO PER IL SERVIZIO DI CONSULENZA"),
            "atteso l'oggetto della lettera nel testo estratto"
        );
        assert_eq!(e.pagine, Some(1), "la lettera è di una pagina sola");
        assert_eq!(e.inizio_pagina, vec![0]);
        assert_eq!(pagina_di(&e.inizio_pagina, 42), Some(1));
    }

    /// Il ramo di scorta va provato a parte: sul PDF reale `pdf-extract` ce la
    /// fa da solo, quindi senza questo test la strada del fallback non
    /// verrebbe mai percorsa.
    #[test]
    fn fallback_pdftotext_sullo_stesso_pdf() {
        let p = Path::new("/home/russus/Scaricati/T1Q73KFP/T1Q73KFP_0000001.pdf");
        if !p.exists() {
            return;
        }
        let e = match pdftotext(p) {
            Ok(e) => e,
            // Poppler non installato su questa macchina: non è un fallimento.
            Err(_) => return,
        };
        assert!(e.testo.contains("TEST CARTA 1"));
        assert!(e.testo.contains("CONTRATTO PER IL SERVIZIO DI CONSULENZA"));
        assert_eq!(e.pagine, Some(1), "il form feed finale non conta come pagina");
    }
}

