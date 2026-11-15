//! Semeia um banco de demonstração (usado para verificação visual da UI).
//! Uso: cargo run --example seed_demo --offline -- <caminho.db>

use autoeletrica::db::Database;
use autoeletrica::model::{MovTipo, Produto, Unidade};

fn main() {
    let caminho = std::env::args()
        .nth(1)
        .unwrap_or_else(|| std::env::temp_dir().join("ae-demo.db").to_string_lossy().into());
    let mut db = Database::open(std::path::Path::new(&caminho)).expect("abrir banco");
    if !db.buscar("", false).unwrap().is_empty() {
        println!("já tem dados em {caminho}");
        return;
    }

    let itens: [(&str, &str, &str, &str, &str, Unidade, i64, i64, f64, f64); 6] = [
        ("Lâmpada H4 12V 60/55W", "OSRAM", "Lâmpada", "Gol G5 2008 a 2012 / Fox 1.6", "OSRAM 64193", Unidade::Un, 890, 1890, 24.0, 10.0),
        ("Bateria 60Ah 12V", "Moura", "Bateria", "Universal", "M60GD", Unidade::Un, 28900, 45900, 6.0, 2.0),
        ("Fio flexível 2,5 mm", "Corflex", "Fio", "Universal", "", Unidade::Metro, 150, 390, 100.0, 20.0),
        ("Relé 5 pinos 12V 40A", "Bosch", "Relé", "Universal", "0332019150", Unidade::Un, 650, 1590, 18.0, 8.0),
        ("Fusível lâmina 15A", "Genérico", "Fusível", "Universal", "", Unidade::Un, 50, 150, 200.0, 50.0),
        ("Alternador 12V 90A", "Bosch", "Alternador", "Gol G5 2010 a 2014 / Saveiro", "0124525088", Unidade::Un, 42000, 68900, 1.0, 2.0),
    ];

    for (nome, marca, cat, app, oem, un, custo, venda, estoque, minimo) in itens {
        let p = Produto {
            nome: nome.into(),
            marca: marca.into(),
            categoria: cat.into(),
            aplicacao: app.into(),
            oem: oem.into(),
            unidade: un,
            custo_cents: custo,
            preco_cents: venda,
            estoque,
            estoque_min: minimo,
            corredor: "A".into(),
            prateleira: "1".into(),
            gaveta: cat.chars().take(1).collect(),
            ..Produto::novo()
        };
        let id = db.salvar(&p).expect("salvar");
        db.movimentar(id, MovTipo::Entrada, estoque, Some(custo), "carga inicial")
            .expect("entrada");
    }
    println!("{caminho}: {} peças", db.buscar("", false).unwrap().len());
}
