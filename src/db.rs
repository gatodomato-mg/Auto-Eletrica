//! Persistência SQLite do AutoEletrica. Tudo em uma tabela de produtos + um livro
//! de movimentos; o estoque é a soma dos movimentos, aplicada na coluna `estoque`.

use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use std::path::{Path, PathBuf};

use crate::model::{self as m, MovTipo, Produto, Resumo, Unidade};

pub type Res<T> = Result<T, String>;

const CAMPOS: &str = "id, sku, ean, cod_fabricante, oem, nome, marca, categoria, aplicacao, \
equivalentes, corredor, prateleira, gaveta, unidade, custo_cents, preco_cents, estoque, \
estoque_min, foto, ativo";

fn erro(e: rusqlite::Error) -> String {
    format!("banco: {e}")
}

fn map_produto(row: &Row) -> rusqlite::Result<Produto> {
    let unidade: String = row.get(13)?;
    Ok(Produto {
        id: Some(row.get(0)?),
        sku: row.get(1)?,
        ean: row.get(2)?,
        cod_fabricante: row.get(3)?,
        oem: row.get(4)?,
        nome: row.get(5)?,
        marca: row.get(6)?,
        categoria: row.get(7)?,
        aplicacao: row.get(8)?,
        equivalentes: row.get(9)?,
        corredor: row.get(10)?,
        prateleira: row.get(11)?,
        gaveta: row.get(12)?,
        unidade: Unidade::from_sigla(&unidade),
        custo_cents: row.get(14)?,
        preco_cents: row.get(15)?,
        estoque: row.get(16)?,
        estoque_min: row.get(17)?,
        foto: row.get(18)?,
        ativo: row.get::<_, i64>(19)? == 1,
    })
}

/// "Elétrica H4" -> "eletrica h4". Busca sem acento e sem caixa.
fn normalizar(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            _ => c,
        })
        .collect()
}

/// "2008 a 2012" vira "2008 2009 2010 2011 2012": o mecânico digita o ano do carro,
/// não a faixa. Cobre intervalos de até 40 anos; fora disso o texto fica como está.
fn expandir_anos(s: &str) -> String {
    fn ano(t: &str) -> Option<i32> {
        t.trim_matches(|c: char| !c.is_ascii_digit())
            .parse()
            .ok()
            .filter(|y: &i32| (1950..=2100).contains(y))
    }
    let t: Vec<&str> = s.split_whitespace().collect();
    let mut out: Vec<String> = Vec::with_capacity(t.len());
    let mut i = 0;
    while i < t.len() {
        if let (Some(a), true, Some(b)) = (
            ano(t[i]),
            i + 1 < t.len() && t[i + 1].eq_ignore_ascii_case("a"),
            t.get(i + 2).and_then(|x| ano(x)),
        ) {
            if b >= a && b - a <= 40 {
                out.extend((a..=b).map(|y| y.to_string()));
                i += 3;
                continue;
            }
        }
        out.push(t[i].to_string());
        i += 1;
    }
    out.join(" ")
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Res<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        let conn = Connection::open(path).map_err(erro)?;
        conn.pragma_update(None, "foreign_keys", true).map_err(erro)?;
        let db = Database { conn };
        db.criar_tabelas()?;
        Ok(db)
    }

    pub fn open_memory() -> Res<Self> {
        let conn = Connection::open_in_memory().map_err(erro)?;
        let db = Database { conn };
        db.criar_tabelas()?;
        Ok(db)
    }

    fn criar_tabelas(&self) -> Res<()> {
        self.conn
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS produtos (
                    id              INTEGER PRIMARY KEY AUTOINCREMENT,
                    sku             TEXT NOT NULL UNIQUE,
                    ean             TEXT NOT NULL DEFAULT '',
                    cod_fabricante  TEXT NOT NULL DEFAULT '',
                    oem             TEXT NOT NULL DEFAULT '',
                    nome            TEXT NOT NULL,
                    marca           TEXT NOT NULL DEFAULT '',
                    categoria       TEXT NOT NULL DEFAULT '',
                    aplicacao       TEXT NOT NULL DEFAULT '',
                    equivalentes    TEXT NOT NULL DEFAULT '',
                    corredor        TEXT NOT NULL DEFAULT '',
                    prateleira      TEXT NOT NULL DEFAULT '',
                    gaveta          TEXT NOT NULL DEFAULT '',
                    unidade         TEXT NOT NULL DEFAULT 'UN',
                    custo_cents     INTEGER NOT NULL DEFAULT 0,
                    preco_cents     INTEGER NOT NULL DEFAULT 0,
                    estoque         REAL NOT NULL DEFAULT 0,
                    estoque_min     REAL NOT NULL DEFAULT 0,
                    foto            TEXT NOT NULL DEFAULT '',
                    busca           TEXT NOT NULL DEFAULT '',
                    ativo           INTEGER NOT NULL DEFAULT 1,
                    criado_em       TEXT NOT NULL DEFAULT (datetime('now','localtime')),
                    atualizado_em   TEXT NOT NULL DEFAULT (datetime('now','localtime'))
                );
                CREATE TABLE IF NOT EXISTS movimentos (
                    id               INTEGER PRIMARY KEY AUTOINCREMENT,
                    produto_id       INTEGER NOT NULL REFERENCES produtos(id),
                    tipo             TEXT NOT NULL,
                    qtd              REAL NOT NULL,
                    custo_unit_cents INTEGER NOT NULL DEFAULT 0,
                    motivo           TEXT NOT NULL DEFAULT '',
                    criado_em        TEXT NOT NULL DEFAULT (datetime('now','localtime'))
                );
                CREATE INDEX IF NOT EXISTS idx_mov_produto ON movimentos(produto_id, id DESC);
                "#,
            )
            .map_err(erro)
    }

    pub fn default_path() -> PathBuf {
        match std::env::var_os("LOCALAPPDATA") {
            Some(dir) => PathBuf::from(dir).join("AutoEletrica").join("autoeletrica.db"),
            None => PathBuf::from("autoeletrica.db"),
        }
    }

    /// Cria ou atualiza. Devolve o id. `sku` vazio recebe o próximo AE0001.
    pub fn salvar(&mut self, p: &Produto) -> Res<i64> {
        let nome = p.nome.trim();
        if nome.is_empty() {
            return Err("nome da peça é obrigatório".into());
        }
        if p.custo_cents < 0 || p.preco_cents < 0 {
            return Err("custo e preço não podem ser negativos".into());
        }
        if p.estoque < 0.0 || p.estoque_min < 0.0 {
            return Err("estoque não pode ser negativo".into());
        }

        let base = format!(
            "{} {} {} {} {} {} {} {} {}",
            p.sku, p.ean, p.cod_fabricante, p.oem, nome, p.marca, p.categoria, p.aplicacao,
            p.equivalentes
        );
        let busca = normalizar(&format!("{} {}", base, expandir_anos(&p.aplicacao)));
        let unidade = p.unidade.sigla();

        let valores = (
            p.ean.trim(),
            p.cod_fabricante.trim(),
            p.oem.trim(),
            nome,
            p.marca.trim(),
            p.categoria.trim(),
            p.aplicacao.trim(),
            p.equivalentes.trim(),
            p.corredor.trim(),
            p.prateleira.trim(),
            p.gaveta.trim(),
            unidade,
            p.custo_cents,
            p.preco_cents,
            p.estoque,
            p.estoque_min,
            p.foto.trim(),
            busca,
            p.ativo as i64,
        );

        if let Some(id) = p.id.filter(|id| *id > 0) {
            let sku = if p.sku.trim().is_empty() {
                self.sku_atual(id)?
            } else {
                p.sku.trim().to_string()
            };
            self.conn
                .execute(
                    "UPDATE produtos SET sku = ?1, ean = ?2, cod_fabricante = ?3, oem = ?4,
                     nome = ?5, marca = ?6, categoria = ?7, aplicacao = ?8, equivalentes = ?9,
                     corredor = ?10, prateleira = ?11, gaveta = ?12, unidade = ?13,
                     custo_cents = ?14, preco_cents = ?15, estoque = ?16, estoque_min = ?17,
                     foto = ?18, busca = ?19, ativo = ?20,
                     atualizado_em = datetime('now','localtime') WHERE id = ?21",
                    params![
                        sku,
                        valores.0,
                        valores.1,
                        valores.2,
                        valores.3,
                        valores.4,
                        valores.5,
                        valores.6,
                        valores.7,
                        valores.8,
                        valores.9,
                        valores.10,
                        valores.11,
                        valores.12,
                        valores.13,
                        valores.14,
                        valores.15,
                        valores.16,
                        valores.17,
                        valores.18,
                        id
                    ],
                )
                .map_err(|e| conflito(e, &sku))?;
            Ok(id)
        } else {
            let sku = if p.sku.trim().is_empty() {
                self.proximo_sku()?
            } else {
                p.sku.trim().to_string()
            };
            self.conn
                .execute(
                    "INSERT INTO produtos (sku, ean, cod_fabricante, oem, nome, marca, categoria,
                     aplicacao, equivalentes, corredor, prateleira, gaveta, unidade,
                     custo_cents, preco_cents, estoque, estoque_min, foto, busca, ativo)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                     ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
                    params![
                        sku,
                        valores.0,
                        valores.1,
                        valores.2,
                        valores.3,
                        valores.4,
                        valores.5,
                        valores.6,
                        valores.7,
                        valores.8,
                        valores.9,
                        valores.10,
                        valores.11,
                        valores.12,
                        valores.13,
                        valores.14,
                        valores.15,
                        valores.16,
                        valores.17,
                        valores.18
                    ],
                )
                .map_err(|e| conflito(e, &sku))?;
            Ok(self.conn.last_insert_rowid())
        }
    }

    fn sku_atual(&self, id: i64) -> Res<String> {
        self.conn
            .query_row("SELECT sku FROM produtos WHERE id = ?1", [id], |r| r.get(0))
            .map_err(erro)
    }

    fn proximo_sku(&self) -> Res<String> {
        let max: i64 = self
            .conn
            .query_row("SELECT COALESCE(MAX(id), 0) FROM produtos", [], |r| r.get(0))
            .map_err(erro)?;
        Ok(format!("AE{:04}", max + 1))
    }

    /// Tokens separados por espaço precisam casar todos (em qualquer campo, sem acento).
    pub fn buscar(&self, termo: &str, so_abaixo_min: bool) -> Res<Vec<Produto>> {
        let mut sql = format!("SELECT {CAMPOS} FROM produtos WHERE 1=1");
        let mut args: Vec<String> = Vec::new();

        for token in normalizar(termo).split_whitespace() {
            args.push(format!("%{}%", token.replace('%', "")));
            sql.push_str(&format!(" AND busca LIKE ?{}", args.len()));
        }
        if so_abaixo_min {
            sql.push_str(" AND estoque_min > 0 AND estoque <= estoque_min");
        }
        sql.push_str(" ORDER BY nome COLLATE NOCASE");

        let mut stmt = self.conn.prepare(&sql).map_err(erro)?;
        let linhas = stmt
            .query_map(params_from_iter(args.iter()), map_produto)
            .map_err(erro)?;
        linhas.collect::<rusqlite::Result<Vec<_>>>().map_err(erro)
    }

    pub fn get(&self, id: i64) -> Res<Option<Produto>> {
        let mut stmt = self
            .conn
            .prepare(&format!("SELECT {CAMPOS} FROM produtos WHERE id = ?1"))
            .map_err(erro)?;
        stmt.query_row([id], map_produto)
            .optional()
            .map_err(erro)
    }

    /// Entrada: soma quantidade e refaz o custo médio ponderado.
    /// Saída: baixa quantidade, recusa se faltar. Ajuste: `qtd` é a contagem nova.
    pub fn movimentar(
        &mut self,
        produto_id: i64,
        tipo: MovTipo,
        qtd: f64,
        custo_unit_cents: Option<i64>,
        motivo: &str,
    ) -> Res<()> {
        let p = self
            .get(produto_id)?
            .ok_or_else(|| format!("produto id={produto_id} não encontrado"))?;
        if !qtd.is_finite() || qtd < 0.0 {
            return Err("quantidade inválida".into());
        }
        let custo = custo_unit_cents.unwrap_or(p.custo_cents).max(0);

        let (novo_estoque, novo_custo, delta) = match tipo {
            MovTipo::Entrada => {
                if qtd == 0.0 {
                    return Err("quantidade da entrada deve ser maior que zero".into());
                }
                let novo = p.estoque + qtd;
                let custo_medio =
                    ((p.custo_cents as f64 * p.estoque) + custo as f64 * qtd) / novo;
                (novo, (custo_medio.round() as i64).max(0), qtd)
            }
            MovTipo::Saida => {
                if qtd == 0.0 {
                    return Err("quantidade da saída deve ser maior que zero".into());
                }
                if p.estoque + 1e-9 < qtd {
                    return Err(format!(
                        "estoque insuficiente: tem {} {}",
                        m::qty_str(p.estoque),
                        p.unidade.sigla()
                    ));
                }
                (p.estoque - qtd, p.custo_cents, -qtd)
            }
            MovTipo::Ajuste => (qtd, p.custo_cents, qtd - p.estoque),
        };

        let tx = self.conn.transaction().map_err(erro)?;
        tx.execute(
            "UPDATE produtos SET estoque = ?1, custo_cents = ?2,
             atualizado_em = datetime('now','localtime') WHERE id = ?3",
            params![novo_estoque, novo_custo, produto_id],
        )
        .map_err(erro)?;
        tx.execute(
            "INSERT INTO movimentos (produto_id, tipo, qtd, custo_unit_cents, motivo)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![produto_id, tipo.as_str(), delta, custo, motivo.trim()],
        )
        .map_err(erro)?;
        tx.commit().map_err(erro)
    }

    pub fn movimentos(&self, produto_id: i64, limite: i64) -> Res<Vec<m::Movimento>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, produto_id, tipo, qtd, custo_unit_cents, motivo, criado_em
                 FROM movimentos WHERE produto_id = ?1 ORDER BY id DESC LIMIT ?2",
            )
            .map_err(erro)?;
        let linhas = stmt
            .query_map(params![produto_id, limite], |row| {
                let tipo: String = row.get(2)?;
                Ok(m::Movimento {
                    id: row.get(0)?,
                    produto_id: row.get(1)?,
                    tipo: MovTipo::from_str(&tipo),
                    qtd: row.get(3)?,
                    custo_unit_cents: row.get(4)?,
                    motivo: row.get(5)?,
                    criado_em: row.get(6)?,
                })
            })
            .map_err(erro)?;
        linhas.collect::<rusqlite::Result<Vec<_>>>().map_err(erro)
    }

    pub fn abaixo_minimo(&self) -> Res<Vec<Produto>> {
        self.buscar("", true)
    }

    pub fn resumo(&self) -> Res<Resumo> {
        self.conn
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(estoque), 0),
                        COALESCE(SUM(custo_cents * estoque), 0),
                        COALESCE(SUM(preco_cents * estoque), 0)
                 FROM produtos WHERE ativo = 1",
                [],
                |row| {
                    Ok(Resumo {
                        itens: row.get(0)?,
                        unidades: row.get(1)?,
                        custo_cents: row.get::<_, f64>(2)?.round() as i64,
                        venda_cents: row.get::<_, f64>(3)?.round() as i64,
                    })
                },
            )
            .map_err(erro)
    }
}

fn conflito(e: rusqlite::Error, sku: &str) -> String {
    if e.to_string().contains("UNIQUE") {
        format!("já existe uma peça com o código interno {sku}")
    } else {
        erro(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peca(nome: &str, custo: i64, preco: i64) -> Produto {
        Produto { nome: nome.into(), custo_cents: custo, preco_cents: preco, ..Produto::novo() }
    }

    #[test]
    fn custo_medio_ponderado() {
        let mut db = Database::open_memory().unwrap();
        let id = db.salvar(&peca("Lâmpada H7", 1000, 2000)).unwrap();
        db.movimentar(id, MovTipo::Entrada, 2.0, Some(1000), "").unwrap();
        db.movimentar(id, MovTipo::Entrada, 2.0, Some(2000), "").unwrap();
        let p = db.get(id).unwrap().unwrap();
        assert_eq!(p.estoque, 4.0);
        assert_eq!(p.custo_cents, 1500);
    }

    #[test]
    fn saida_recusa_sem_estoque() {
        let mut db = Database::open_memory().unwrap();
        let id = db.salvar(&peca("Fio 2,5mm", 100, 300)).unwrap();
        db.movimentar(id, MovTipo::Entrada, 100.0, None, "rolo").unwrap();
        assert!(db.movimentar(id, MovTipo::Saida, 101.0, None, "").is_err());
        assert_eq!(db.get(id).unwrap().unwrap().estoque, 100.0);
        db.movimentar(id, MovTipo::Saida, 1.5, None, "O.S.").unwrap();
        assert_eq!(db.get(id).unwrap().unwrap().estoque, 98.5);
    }

    #[test]
    fn busca_multi_token_sem_acento() {
        let mut db = Database::open_memory().unwrap();
        let mut p = peca("Alternador", 0, 0);
        p.aplicacao = "Gol G5 2008 a 2012 / Fox 1.6".into();
        p.oem = "VW 06A903023".into();
        db.salvar(&p).unwrap();
        db.salvar(&peca("Bateria 60Ah", 0, 0)).unwrap();
        assert_eq!(db.buscar("gol 2010", false).unwrap().len(), 1);
        assert_eq!(db.buscar("ALTERNADOR", false).unwrap().len(), 1);
        assert_eq!(db.buscar("06a903023", false).unwrap().len(), 1);
        assert_eq!(db.buscar("bateria 60", false).unwrap().len(), 1);
        assert_eq!(db.buscar("", false).unwrap().len(), 2);
    }

    #[test]
    fn ajuste_fracionado_e_minimo() {
        let mut db = Database::open_memory().unwrap();
        let mut p = peca("Fio 4mm", 100, 300);
        p.unidade = Unidade::Metro;
        p.estoque_min = 10.0;
        let id = db.salvar(&p).unwrap();
        db.movimentar(id, MovTipo::Ajuste, 95.5, None, "contagem").unwrap();
        let p = db.get(id).unwrap().unwrap();
        assert_eq!(p.estoque, 95.5);
        assert!(db.abaixo_minimo().unwrap().is_empty());
        db.movimentar(id, MovTipo::Saida, 90.0, None, "").unwrap();
        assert_eq!(db.abaixo_minimo().unwrap().len(), 1);
    }

    #[test]
    fn dinheiro_e_quantidade() {
        assert_eq!(m::parse_money("R$ 1.234,56"), Some(123456));
        assert_eq!(m::parse_money("12.50"), Some(1250));
        assert_eq!(m::brl(123456), "R$ 1.234,56");
        assert_eq!(m::brl(-5), "-R$ 0,05");
        assert_eq!(m::parse_qty("1,5"), Some(1.5));
        assert_eq!(m::qty_str(1.5), "1,5");
        assert_eq!(m::qty_str(2.0), "2");
    }
}
