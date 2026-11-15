//! Tipos do domínio: produto, movimento e conversões de dinheiro/quantidade.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unidade {
    Un,
    Metro,
    Quilo,
    Litro,
}

impl Default for Unidade {
    fn default() -> Self {
        Unidade::Un
    }
}

impl Unidade {
    pub const TODAS: [Unidade; 4] = [Unidade::Un, Unidade::Metro, Unidade::Quilo, Unidade::Litro];

    pub fn sigla(self) -> &'static str {
        match self {
            Unidade::Un => "UN",
            Unidade::Metro => "M",
            Unidade::Quilo => "KG",
            Unidade::Litro => "L",
        }
    }

    pub fn rotulo(self) -> &'static str {
        match self {
            Unidade::Un => "Unidade (UN)",
            Unidade::Metro => "Metro (M)",
            Unidade::Quilo => "Quilo (KG)",
            Unidade::Litro => "Litro (L)",
        }
    }

    pub fn from_sigla(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "M" => Unidade::Metro,
            "KG" => Unidade::Quilo,
            "L" => Unidade::Litro,
            _ => Unidade::Un,
        }
    }

    /// Fio, estanho, tubo: aceita 1,5 m em vez de exigir 1 unidade fechada.
    pub fn fracionada(self) -> bool {
        self != Unidade::Un
    }
}

#[derive(Clone, Debug, Default)]
pub struct Produto {
    pub id: Option<i64>,
    pub sku: String,
    pub ean: String,
    pub cod_fabricante: String,
    pub oem: String,
    pub nome: String,
    pub marca: String,
    pub categoria: String,
    pub aplicacao: String,
    pub equivalentes: String,
    pub corredor: String,
    pub prateleira: String,
    pub gaveta: String,
    pub unidade: Unidade,
    pub custo_cents: i64,
    pub preco_cents: i64,
    pub estoque: f64,
    pub estoque_min: f64,
    pub foto: String,
    pub ativo: bool,
}

impl Produto {
    pub fn novo() -> Self {
        Produto { ativo: true, ..Default::default() }
    }

    pub fn localizacao(&self) -> String {
        let partes: Vec<&str> = [&self.corredor, &self.prateleira, &self.gaveta]
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if partes.is_empty() { "—".into() } else { partes.join(" > ") }
    }

    /// Margem sobre o custo em %. `None` quando o custo é zero.
    pub fn margem(&self) -> Option<f64> {
        (self.custo_cents > 0).then(|| {
            (self.preco_cents - self.custo_cents) as f64 * 100.0 / self.custo_cents as f64
        })
    }

    pub fn abaixo_minimo(&self) -> bool {
        self.estoque_min > 0.0 && self.estoque <= self.estoque_min
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MovTipo {
    Entrada,
    Saida,
    Ajuste,
}

impl MovTipo {
    pub fn as_str(self) -> &'static str {
        match self {
            MovTipo::Entrada => "entrada",
            MovTipo::Saida => "saida",
            MovTipo::Ajuste => "ajuste",
        }
    }

    pub fn rotulo(self) -> &'static str {
        match self {
            MovTipo::Entrada => "Entrada",
            MovTipo::Saida => "Saída",
            MovTipo::Ajuste => "Ajuste de inventário",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "entrada" => MovTipo::Entrada,
            "ajuste" => MovTipo::Ajuste,
            _ => MovTipo::Saida,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Movimento {
    pub id: i64,
    pub produto_id: i64,
    /// Positivo em entrada, negativo em saída, diferença em ajuste.
    pub qtd: f64,
    pub tipo: MovTipo,
    pub custo_unit_cents: i64,
    pub motivo: String,
    pub criado_em: String,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Resumo {
    pub itens: i64,
    pub unidades: f64,
    pub custo_cents: i64,
    pub venda_cents: i64,
}

pub fn brl(cents: i64) -> String {
    let neg = cents < 0;
    let v = cents.unsigned_abs();
    let reais = (v / 100).to_string();
    let mut out = String::with_capacity(reais.len() + 4);
    for (i, ch) in reais.chars().enumerate() {
        if i > 0 && (reais.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(ch);
    }
    format!("{}{},{:02}", if neg { "-R$ " } else { "R$ " }, out, v % 100)
}

/// Aceita "12,50", "12.50", "R$ 1.234,56".
pub fn parse_money(txt: &str) -> Option<i64> {
    let mut s = txt.trim().replace("R$", "").replace(' ', "");
    if s.is_empty() {
        return None;
    }
    if s.contains(',') {
        s = s.replace('.', "").replace(',', ".");
    }
    s.parse::<f64>().ok().filter(|v| v.is_finite()).map(|v| (v * 100.0).round() as i64)
}

/// Aceita "1,5" e "1.5"; rejeita negativo.
pub fn parse_qty(txt: &str) -> Option<f64> {
    let s = txt.trim().replace(',', ".");
    s.parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0)
}

/// 2.0 -> "2", 1.5 -> "1,5", 0.125 -> "0,125".
pub fn qty_str(v: f64) -> String {
    let s = format!("{:.3}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.replace('.', ",")
}
