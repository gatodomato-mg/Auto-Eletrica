// AutoEletrica — estoque de auto elétrica.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ui;

use std::path::Path;

use autoeletrica::db::Database;
use eframe::egui;

/// Cópia do banco a cada arranque, mantendo as 10 mais recentes.
/// Copia antes de abrir: não há escrita concorrente.
fn backup_rotativo(caminho: &Path) {
    if !caminho.exists() {
        return;
    }
    let dir = caminho
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("backups");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let destino = dir.join(format!("auto-{}.db", chrono::Local::now().format("%Y%m%d-%H%M%S")));
    if std::fs::copy(caminho, &destino).is_err() {
        return;
    }
    let mut antigos: Vec<std::path::PathBuf> = match std::fs::read_dir(&dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "db"))
            .collect(),
        Err(_) => return,
    };
    antigos.sort();
    while antigos.len() > 10 {
        let _ = std::fs::remove_file(antigos.remove(0));
    }
}

fn main() -> eframe::Result<()> {
    // AUTOELETRICA_DB permite apontar para outro arquivo (demo, testes, backup).
    let caminho = std::env::var_os("AUTOELETRICA_DB")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(Database::default_path);
    backup_rotativo(&caminho);

    let db = match Database::open(&caminho) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("AutoElétrica: não foi possível abrir {}: {e}", caminho.display());
            std::process::exit(1);
        }
    };

    let opcoes = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            // 1366×768 é o notebook típico do balcão; 680 de altura cabe na área útil (728 px).
            .with_inner_size([1200.0, 680.0])
            .with_min_inner_size([960.0, 600.0])
            .with_title("AutoElétrica — estoque"),
        ..Default::default()
    };

    eframe::run_native(
        "AutoElétrica",
        opcoes,
        Box::new(move |cc| Ok(Box::new(ui::App::new(cc, db, caminho)))),
    )
}
