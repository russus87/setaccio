//! Censimento da riga di comando: scandisce una o più cartelle e stampa cosa
//! ha trovato. Serve a validare il motore su volumi veri senza passare dalla
//! UI, e a produrre i numeri con cui si verifica l'ipotesi da cui è nato il
//! progetto.
//!
//! Uso: `cargo run --release --example censimento -- <cartella> [altra…]`

use std::sync::Arc;
use std::time::Instant;

use setaccio_lib::db::Db;
use setaccio_lib::types::Fascia;
use setaccio_lib::{cerca, scan};

fn mb(byte: i64) -> String {
    format!("{:.1} GB", byte as f64 / 1024.0 / 1024.0 / 1024.0)
}

fn main() -> anyhow::Result<()> {
    let cartelle: Vec<String> = std::env::args().skip(1).collect();
    if cartelle.is_empty() {
        eprintln!("uso: censimento <cartella> [altra…]");
        std::process::exit(2);
    }

    let db = Arc::new(Db::in_memoria()?);
    for c in &cartelle {
        db.sorgente_aggiungi(c, Fascia::Documenti)?;
        println!("sorgente: {c}");
    }

    let controllo = Arc::new(scan::Controllo::nuovo());
    let inizio = Instant::now();

    let c2 = controllo.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(3));
        let p = c2.progresso();
        if p.finito {
            break;
        }
        eprint!(
            "\r  visti {:>6} · indicizzati {:>6} · artefatti {:>6} · testo {:>6} · errori {:>4}",
            p.visti, p.indicizzati, p.saltati_repo, p.estratti, p.errori
        );
    });

    scan::esegui(db.clone(), controllo.clone(), |_| {})?;
    let durata = inizio.elapsed();
    let p = controllo.progresso();
    eprintln!();

    println!("\n─── scansione ───");
    println!("  durata            {:.1}s", durata.as_secs_f64());
    println!("  file visti        {}", p.visti);
    println!("  indicizzati       {}", p.indicizzati);
    println!("  artefatti (repo)  {}", p.saltati_repo);
    println!("  testo estratto    {}", p.estratti);
    println!("  errori            {}", p.errori);

    let s = cerca::statistiche(&db)?;
    println!("\n─── indice ───");
    println!("  file (non artefatti)  {}  ({})", s.file_totali, mb(s.byte_totali));
    println!("  documenti             {}", s.documenti);
    println!("  tracciati             {}", s.tracciati);
    println!("  artefatti esclusi     {}", s.artefatti_esclusi);
    println!("  non classificati      {}", s.non_classificati);

    println!("\n─── per tipo ───");
    for c in &s.per_tipo {
        println!("  {:<12} {:>6}  {}", c.etichetta, c.quanti, mb(c.byte));
    }

    println!("\n─── per contesto ───");
    for c in s.per_contesto.iter().take(12) {
        println!("  {:<22} {:>6}  {}", c.etichetta, c.quanti, mb(c.byte));
    }

    // La prova del nove: una ricerca full-text su ciò che si è appena indicizzato.
    for termine in ["contratto", "recesso"] {
        let r = cerca::full_text(&db, termine, &Default::default())?;
        println!("\n─── ricerca «{termine}» → {} risultati ───", r.len());
        for x in r.iter().take(3) {
            let dove = match (x.pagina, x.riga) {
                (Some(p), _) => format!(" · pag. {p}"),
                (_, Some(r)) => format!(" · riga {r}"),
                _ => String::new(),
            };
            println!("  {}{}", x.file.nome, dove);
            println!("    {}", x.snippet.replace('\n', " ").trim());
        }
    }

    Ok(())
}
