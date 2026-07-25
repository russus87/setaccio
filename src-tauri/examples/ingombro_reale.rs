//! Stampa quello che la vista Ingombro mostrerebbe, sull'indice vero.
//!
//! Serve a guardare il risultato del comando `ingombro` su dati reali senza
//! passare dalla finestra: l'indice di sviluppo ha decine di migliaia di
//! file, ed è l'unico posto in cui si vede se l'aggregazione per cartella
//! regge e se i tempi sono accettabili.
//!
//! Sola lettura: apre il database e interroga, non tocca né indice né disco.
//!
//! ```sh
//! cargo run --example ingombro_reale
//! ```

use setaccio_lib::{db, db::Db, ingombro, types::Filtri};

fn byte(n: i64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    format!("{v:.1} {}", U[i])
}

fn main() -> anyhow::Result<()> {
    let percorso = db::percorso_db()?;
    println!("indice: {}\n", percorso.display());
    let db = Db::apri(&percorso)?;

    for (nome, soglia) in [("tutto", 0i64), ("oltre 10 MB", 10 * 1024 * 1024)] {
        let filtri = Filtri {
            size_min: (soglia > 0).then_some(soglia),
            ..Default::default()
        };
        let inizio = std::time::Instant::now();
        let out = ingombro::ingombro(&db, &filtri, 100)?;
        let durata = inizio.elapsed();

        println!("=== {nome} — {durata:?}");
        println!(
            "  {} file, {} in tutto; i primi {} pesano {} ({}%)",
            out.quanti_totali,
            byte(out.byte_totali),
            out.file.len(),
            byte(out.byte_mostrati),
            if out.byte_totali > 0 {
                out.byte_mostrati * 100 / out.byte_totali
            } else {
                0
            }
        );

        println!("  -- i 5 file più grandi");
        for f in out.file.iter().take(5) {
            println!("     {:>10}  {}", byte(f.size), f.path);
        }

        println!("  -- le 8 cartelle più pesanti");
        for c in out.cartelle.iter().take(8) {
            println!(
                "     {:>10}  {:>6} file  {}",
                byte(c.byte),
                c.quanti,
                c.path
            );
        }

        println!("  -- per estensione");
        let riga: Vec<String> = out
            .per_estensione
            .iter()
            .take(8)
            .map(|e| format!("{} {}", e.etichetta, byte(e.byte)))
            .collect();
        println!("     {}\n", riga.join(" · "));
    }

    Ok(())
}
