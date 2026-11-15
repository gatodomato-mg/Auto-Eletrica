// AutoEletrica — estoque de auto elétrica.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ui;

use autoeletrica::db::Database;
use eframe::egui;

fn main() -> eframe::Result<()> {
    // AUTOELETRICA_DB permite apontar para outro arquivo (demo, testes, backup).
    let caminho = std::env::var_os("AUTOELETRICA_DB")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(Database::default_path);
    let db = match Database::open(&caminho) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("AutoElétrica: não foi possível abrir {}: {e}", caminho.display());
            std::process::exit(1);
        }
    };

    let opcoes = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 860.0])
            .with_min_inner_size([980.0, 640.0])
            .with_title("AutoElétrica — estoque"),
        ..Default::default()
    };

    eframe::run_native(
        "AutoElétrica",
        opcoes,
        Box::new(move |cc| Ok(Box::new(ui::App::new(cc, db)))),
    )
}
