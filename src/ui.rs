//! Janela única: busca + lista à esquerda, ficha da peça à direita.

use eframe::egui::{self, Color32, RichText};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use autoeletrica::db::Database;
use autoeletrica::model::{
    brl, parse_money, parse_qty, qty_str, MovTipo, Movimento, Produto, Resumo, Unidade,
};

fn money_txt(cents: i64) -> String {
    format!("{:.2}", cents as f64 / 100.0).replace('.', ",")
}

/// Uma linha do grid de formulário: rótulo à esquerda, campo à direita.
/// O Grid do egui não quebra linha sozinho, por isso o end_row aqui.
fn campo_linha(ui: &mut egui::Ui, rotulo: &str, valor: &mut String) {
    ui.label(rotulo);
    ui.add(egui::TextEdit::singleline(valor).desired_width(250.0));
    ui.end_row();
}

/// Rótulo clicável: qualquer célula da linha da lista abre a ficha da peça.
fn celula(ui: &mut egui::Ui, texto: impl Into<egui::WidgetText>) -> egui::Response {
    ui.add(egui::Label::new(texto).sense(egui::Sense::click()))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

// ----------------------------------------------------------------

struct FormState {
    p: Produto,
    custo: String,
    preco: String,
    minimo: String,
}

impl FormState {
    fn de(p: Produto) -> Self {
        FormState {
            custo: money_txt(p.custo_cents),
            preco: money_txt(p.preco_cents),
            minimo: qty_str(p.estoque_min),
            p,
        }
    }

    fn novo() -> Self {
        Self::de(Produto::novo())
    }

    /// Converte os campos de volta ao Produto. Retorna erro se dados inválidos.
    fn aplicar(&self) -> Result<Produto, String> {
        let mut p = self.p.clone();
        p.custo_cents = parse_money(&self.custo).ok_or("custo inválido")?;
        p.preco_cents = parse_money(&self.preco).ok_or("preço inválido")?;
        p.estoque_min = parse_qty(&self.minimo).ok_or("estoque mínimo inválido")?;
        if p.nome.trim().is_empty() {
            return Err("nome da peça é obrigatório".into());
        }
        Ok(p)
    }
}

// ----------------------------------------------------------------

struct MovState {
    produto_id: i64,
    nome: String,
    tipo: MovTipo,
    qtd: String,
    custo: String,
    motivo: String,
}

// ----------------------------------------------------------------

pub struct App {
    db: Database,
    db_path: PathBuf,
    busca: String,
    filtro_min: bool,
    lista: Vec<Produto>,
    resumo: Resumo,
    abaixo: usize,
    selecionado: Option<i64>,
    form: Option<FormState>,
    movs: Vec<Movimento>,
    mov: Option<MovState>,
    novo: Option<FormState>,
    msg: Option<(String, bool)>,
    fotos: HashMap<String, egui::TextureHandle>, // só guarda handles válidos
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, db: Database, db_path: PathBuf) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let mut app = App {
            db,
            db_path,
            busca: String::new(),
            filtro_min: false,
            lista: Vec::new(),
            resumo: Resumo::default(),
            abaixo: 0,
            selecionado: None,
            form: None,
            movs: Vec::new(),
            mov: None,
            novo: None,
            msg: None,
            fotos: HashMap::new(),
        };
        app.recarregar();
        app
    }

    fn recarregar(&mut self) {
        match self.db.buscar(&self.busca, self.filtro_min) {
            Ok(l) => self.lista = l,
            Err(e) => self.msg = Some((e, true)),
        }
        self.resumo = self.db.resumo().unwrap_or_default();
        self.abaixo = self.db.abaixo_minimo().map(|v| v.len()).unwrap_or(0);
        // Preserva a seleção se a peça ainda estiver na lista
        if let Some(id) = self.selecionado {
            if !self.lista.iter().any(|p| p.id == Some(id)) {
                self.selecionado = None;
                self.form = None;
            }
        }
    }

    fn selecionar(&mut self, id: i64) {
        match self.db.get(id) {
            Ok(Some(p)) => {
                self.selecionado = Some(id);
                self.form = Some(FormState::de(p));
                self.movs = self.db.movimentos(id, 20).unwrap_or_default();
                self.msg = None;
            }
            Ok(None) => self.msg = Some(("peça não encontrada".into(), true)),
            Err(e) => self.msg = Some((e, true)),
        }
    }

    /// Textura: carrega/decodeia imagem e devolve o TextureHandle. Cache apenas handles válidos.
    /// Pasta de backups ao lado do banco.
    fn pasta_backups(&self) -> PathBuf {
        self.db_path
            .parent()
            .unwrap_or(Path::new("."))
            .join("backups")
    }

    fn backup(&mut self) {
        let destino = self
            .pasta_backups()
            .join(format!("autoeletrica-{}.db", chrono::Local::now().format("%Y%m%d-%H%M%S")));
        match self.db.backup(&destino) {
            Ok(()) => self.msg = Some((format!("backup: {}", destino.display()), false)),
            Err(e) => self.msg = Some((e, true)),
        }
    }

    fn exportar_csv(&mut self) {
        let destino = self
            .pasta_backups()
            .join(format!("catalogo-{}.csv", chrono::Local::now().format("%Y%m%d-%H%M%S")));
        let csv = match self.db.exportar_csv() {
            Ok(c) => c,
            Err(e) => {
                self.msg = Some((e, true));
                return;
            }
        };
        if let Some(pai) = destino.parent() {
            let _ = std::fs::create_dir_all(pai);
        }
        match std::fs::write(&destino, csv) {
            Ok(()) => self.msg = Some((format!("CSV: {}", destino.display()), false)),
            Err(e) => self.msg = Some((format!("erro ao gravar CSV: {e}"), true)),
        }
    }

    fn textura(&mut self, ctx: &egui::Context, caminho: &str) -> Option<egui::TextureHandle> {
        if caminho.trim().is_empty() {
            return None;
        }
        // Retorna do cache se já houver um handle
        if let Some(h) = self.fotos.get(caminho) {
            return Some(h.clone());
        }
        let tex = std::fs::read(caminho)
            .ok()
            .and_then(|bytes| image::load_from_memory(&bytes).ok())
            .map(|img| {
                let rgba = img.to_rgba8();
                let tamanho = [rgba.width() as usize, rgba.height() as usize];
                let cor = egui::ColorImage::from_rgba_unmultiplied(tamanho, rgba.as_raw());
                ctx.load_texture(caminho, cor, egui::TextureOptions::LINEAR)
            });
        // Só insere no cache se a textura foi criada com sucesso
        if let Some(t) = &tex {
            self.fotos.insert(caminho.to_string(), t.clone());
        }
        tex
    }

    fn topo(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("topo").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("AutoElétrica").strong().size(18.0));
                ui.separator();
                let resposta = ui.add(
                    egui::TextEdit::singleline(&mut self.busca)
                        .hint_text("Peça, código, OEM, carro… (Enter busca)")
                        .desired_width(340.0),
                );
                if resposta.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.recarregar();
                }
                if ui.button("Buscar").clicked() {
                    self.recarregar();
                }
                if ui.checkbox(&mut self.filtro_min, "só abaixo do mínimo").changed() {
                    self.recarregar();
                }
                ui.separator();
                if ui.button("+ Novo produto").clicked() {
                    self.novo = Some(FormState::novo());
                }
                if ui.button("Backup").clicked() {
                    self.backup();
                }
                if ui.button("Exportar CSV").clicked() {
                    self.exportar_csv();
                }
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label(format!("{} peças", self.resumo.itens));
                ui.separator();
                ui.label(format!("{} un", qty_str(self.resumo.unidades)));
                ui.separator();
                ui.label(format!("custo {}", brl(self.resumo.custo_cents)));
                ui.separator();
                ui.label(format!("venda {}", brl(self.resumo.venda_cents)));
                if let Some((texto, erro)) = &self.msg {
                    ui.separator();
                    let cor = if *erro { Color32::from_rgb(255, 120, 120) } else { Color32::LIGHT_GREEN };
                    ui.label(RichText::new(texto).color(cor));
                }
            });
            if self.abaixo > 0 {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("⚠ {} peça(s) abaixo do estoque mínimo", self.abaixo))
                            .color(Color32::from_rgb(255, 190, 90))
                            .strong(),
                    );
                    // Botão "ver" agora apenas atualiza a lista filtrada, sem duplicidade
                    if ui.button("ver filtrado").clicked() {
                        self.filtro_min = true;
                        self.recarregar();
                    }
                });
            }
            ui.add_space(2.0);
        });
    }

    fn lista(&mut self, ctx: &egui::Context) {
        let mut clicado = None;
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                egui::Grid::new("lista").num_columns(8).striped(true).spacing([12.0, 4.0]).show(ui, |ui| {
                    for titulo in ["Código", "Peça", "Aplicação", "Local", "Estoque", "Custo", "Venda", "Margem"] {
                        ui.label(RichText::new(titulo).strong().size(12.0));
                    }
                    ui.end_row();

                    for p in &self.lista {
                        let id = p.id.unwrap_or(0);
                        let sel = self.selecionado == Some(id);
                        let mut hit = false;
                        // Qualquer célula abre a ficha; o nome mostra a seleção.
                        hit |= celula(ui, p.sku.as_str()).clicked();
                        let nome_texto = format!(
                            "{}{}",
                            p.nome,
                            if p.marca.is_empty() { String::new() } else { format!(" · {}", p.marca) }
                        );
                        let nome = RichText::new(&nome_texto);
                        let nome = if sel { nome.strong() } else { nome };
                        hit |= ui.selectable_label(sel, nome).clicked();
                        let curta: String = p.aplicacao.chars().take(34).collect();
                        let app = if curta.is_empty() { "—" } else { curta.as_str() };
                        hit |= celula(ui, app)
                            .on_hover_text(if p.aplicacao.is_empty() { "—" } else { p.aplicacao.as_str() })
                            .clicked();
                        hit |= celula(ui, p.localizacao()).clicked();
                        let estoque_texto = format!("{} {}", qty_str(p.estoque), p.unidade.sigla());
                        if p.abaixo_minimo() {
                            hit |= celula(
                                ui,
                                RichText::new(format!("⚠ {estoque_texto}"))
                                    .color(Color32::from_rgb(255, 140, 90)),
                            )
                            .clicked();
                        } else {
                            hit |= celula(ui, estoque_texto).clicked();
                        }
                        hit |= celula(ui, brl(p.custo_cents)).clicked();
                        hit |= celula(ui, brl(p.preco_cents)).clicked();
                        hit |= celula(ui, p.margem().map(|m| format!("{m:.0}%")).unwrap_or_default())
                            .clicked();
                        if hit {
                            clicado = Some(id);
                        }
                        ui.end_row();
                    }
                });
                if self.lista.is_empty() {
                    ui.add_space(12.0);
                    ui.label("Nada encontrado. Ajuste a busca ou cadastre uma peça.");
                }
            });
        });
        if let Some(id) = clicado {
            self.selecionar(id);
        }
    }

    fn detalhe(&mut self, ctx: &egui::Context) {
        if self.selecionado.is_none() {
            return;
        }
        egui::SidePanel::right("detalhe")
            .default_width(460.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // Pega o form em mutável; se nada houver, sai cedo
                    let mut form = match self.form.take() {
                        Some(f) => f,
                        None => return,
                    };
                    let id = form.p.id.unwrap_or(0);

                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&form.p.nome).strong().size(16.0));
                        if !form.p.ativo {
                            ui.label(RichText::new("inativa").color(Color32::GRAY));
                        }
                    });
                    ui.label(format!("{} · estoque {} {}", form.p.sku, qty_str(form.p.estoque), form.p.unidade.sigla()));
                    ui.label(format!("Local: {}", form.p.localizacao()));

                    // Foto
                    let foto = self.textura(ctx, &form.p.foto);
                    match foto {
                        Some(tex) => {
                            let proporcao = tex.size_vec2().x / tex.size_vec2().y.max(1.0);
                            let largura = 180.0_f32.min(tex.size_vec2().x);
                            let tamanho = egui::vec2(largura, largura / proporcao);
                            ui.add(egui::Image::new((tex.id(), tamanho)));
                        }
                        None => {
                            ui.label(RichText::new("sem foto").color(Color32::GRAY).small());
                        }
                    }

                    ui.separator();
                    ui.label(RichText::new("Dados da peça").strong());
                    egui::Grid::new("campos").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                        campo_linha(ui, "Código interno", &mut form.p.sku);
                        campo_linha(ui, "Código de barras", &mut form.p.ean);
                        campo_linha(ui, "Cód. fabricante", &mut form.p.cod_fabricante);
                        campo_linha(ui, "Cód. original (OEM)", &mut form.p.oem);
                        campo_linha(ui, "Nome", &mut form.p.nome);
                        campo_linha(ui, "Marca", &mut form.p.marca);
                        campo_linha(ui, "Categoria", &mut form.p.categoria);
                        ui.label("Aplicação");
                        ui.add(egui::TextEdit::multiline(&mut form.p.aplicacao).desired_rows(3).desired_width(250.0));
                        ui.end_row();
                        ui.label("Equivalentes");
                        ui.add(egui::TextEdit::multiline(&mut form.p.equivalentes).desired_rows(2).desired_width(250.0));
                        ui.end_row();
                        campo_linha(ui, "Corredor", &mut form.p.corredor);
                        campo_linha(ui, "Prateleira", &mut form.p.prateleira);
                        campo_linha(ui, "Gaveta", &mut form.p.gaveta);
                        ui.label("Unidade");
                        egui::ComboBox::from_id_salt("unidade_sel")
                            .selected_text(form.p.unidade.rotulo())
                            .show_ui(ui, |ui| {
                                for u in Unidade::TODAS { ui.selectable_value(&mut form.p.unidade, u, u.rotulo()); }
                            });
                        ui.end_row();
                        campo_linha(ui, "Foto (caminho)", &mut form.p.foto);
                        campo_linha(ui, "Custo (R$)", &mut form.custo);
                        campo_linha(ui, "Venda (R$)", &mut form.preco);
                        campo_linha(ui, "Estoque mínimo", &mut form.minimo);
                    });

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("Salvar alterações").clicked() {
                            match form.aplicar() {
                                Ok(p) => match self.db.salvar(&p) {
                                    Ok(_) => {
                                        self.msg = Some(("alterações salvas".into(), false));
                                        self.selecionar(id);
                                        self.recarregar();
                                    }
                                    Err(e) => self.msg = Some((e, true)),
                                },
                                Err(e) => self.msg = Some((e, true)),
                            }
                        }
                        let rotulo = if form.p.ativo { "Desativar" } else { "Reativar" };
                        if ui.button(rotulo).clicked() {
                            let mut p = form.p.clone();
                            p.ativo = !p.ativo;
                            match self.db.salvar(&p) {
                                Ok(_) => {
                                    self.selecionar(id);
                                    self.recarregar();
                                }
                                Err(e) => self.msg = Some((e, true)),
                            }
                        }
                    });

                    ui.separator();
                    ui.label(RichText::new("Movimentar estoque").strong());
                    ui.horizontal(|ui| {
                        for tipo in [MovTipo::Entrada, MovTipo::Saida, MovTipo::Ajuste] {
                            if ui.button(tipo.rotulo()).clicked() {
                                self.mov = Some(MovState {
                                    produto_id: id,
                                    nome: form.p.nome.clone(),
                                    tipo,
                                    qtd: String::new(),
                                    custo: money_txt(form.p.custo_cents),
                                    motivo: String::new(),
                                });
                            }
                        }
                    });

                    if !self.movs.is_empty() {
                        ui.separator();
                        ui.label(RichText::new("Últimos movimentos").strong());
                        for mv in &self.movs {
                            let sinal = if mv.qtd > 0.0 { "+" } else { "" };
                            ui.label(
                                RichText::new(format!(
                                    "{} · {} · {}{} · {}",
                                    mv.criado_em,
                                    mv.tipo.rotulo(),
                                    sinal,
                                    qty_str(mv.qtd.abs()),
                                    if mv.motivo.is_empty() { "—".to_string() } else { mv.motivo.clone() }
                                ))
                                .size(11.0)
                                .color(Color32::GRAY),
                            );
                        }
                    }

                    ui.add_space(8.0);
                    // Depois de salvar, a ficha recém-carregada do banco já está aqui: não sobrescrever.
                    self.form.get_or_insert(form);
                    self.mostrar_aviso(ui);
                });
            });
    }

    fn mostrar_aviso(&self, ui: &mut egui::Ui) {
        if let Some((texto, erro)) = &self.msg {
            let cor = if *erro { Color32::from_rgb(255, 120, 120) } else { Color32::LIGHT_GREEN };
            ui.label(RichText::new(texto).color(cor));
        }
    }

    fn janela_movimento(&mut self, ctx: &egui::Context) {
        let Some(mut mov) = self.mov.take() else { return };
        let mut aberto = true;
        let mut executar = false;
        egui::Window::new(mov.tipo.rotulo())
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .open(&mut aberto)
            .show(ctx, |ui| {
                ui.label(&mov.nome);
                ui.horizontal(|ui| {
                    ui.label(if mov.tipo == MovTipo::Ajuste { "Contagem nova" } else { "Quantidade" });
                    ui.add(egui::TextEdit::singleline(&mut mov.qtd).desired_width(100.0).hint_text("1,5"));
                });
                if mov.tipo == MovTipo::Entrada {
                    ui.horizontal(|ui| {
                        ui.label("Custo unitário (R$)");
                        ui.text_edit_singleline(&mut mov.custo);
                    });
                }
                if mov.tipo == MovTipo::Ajuste {
                    ui.label(RichText::new("A contagem substitui o estoque atual.").small());
                }
                ui.horizontal(|ui| {
                    ui.label("Motivo");
                    ui.add(egui::TextEdit::singleline(&mut mov.motivo).desired_width(220.0));
                });
                if ui.button("Confirmar").clicked() {
                    executar = true;
                }
            });

        if executar {
            let qtd = match parse_qty(&mov.qtd) {
                Some(q) => q,
                None => {
                    self.msg = Some(("quantidade inválida".into(), true));
                    self.mov = Some(mov);
                    return;
                }
            };
            let custo = if mov.tipo == MovTipo::Entrada {
                match parse_money(&mov.custo) {
                    Some(c) => Some(c),
                    None => {
                        self.msg = Some(("custo inválido".into(), true));
                        self.mov = Some(mov);
                        return;
                    }
                }
            } else {
                None
            };
            match self.db.movimentar(mov.produto_id, mov.tipo, qtd, custo, &mov.motivo) {
                Ok(()) => {
                    self.msg = Some(("estoque atualizado".into(), false));
                    let id = mov.produto_id;
                    self.selecionar(id);
                    self.recarregar();
                }
                Err(e) => {
                    self.msg = Some((e, true));
                    self.mov = Some(mov);
                }
            }
        } else if aberto {
            self.mov = Some(mov);
        }
    }

    fn janela_novo(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.novo.take() else { return };
        let mut aberto = true;
        let mut salvar = false;
        egui::Window::new("Nova peça")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .open(&mut aberto)
            .show(ctx, |ui| {
                egui::Grid::new("novo_campos").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                    campo_linha(ui, "Nome *", &mut form.p.nome);
                    campo_linha(ui, "Marca", &mut form.p.marca);
                    campo_linha(ui, "Categoria", &mut form.p.categoria);
                    campo_linha(ui, "Código interno", &mut form.p.sku);
                    campo_linha(ui, "Código de barras", &mut form.p.ean);
                    campo_linha(ui, "Cód. fabricante", &mut form.p.cod_fabricante);
                    campo_linha(ui, "Cód. original (OEM)", &mut form.p.oem);
                    ui.label("Aplicação");
                    ui.add(egui::TextEdit::multiline(&mut form.p.aplicacao).desired_rows(3).desired_width(250.0));
                    ui.end_row();
                    ui.label("Equivalentes");
                    ui.add(egui::TextEdit::multiline(&mut form.p.equivalentes).desired_rows(2).desired_width(250.0));
                    ui.end_row();
                    campo_linha(ui, "Corredor", &mut form.p.corredor);
                    campo_linha(ui, "Prateleira", &mut form.p.prateleira);
                    campo_linha(ui, "Gaveta", &mut form.p.gaveta);
                    ui.label("Unidade");
                    egui::ComboBox::from_id_salt("novo_unidade")
                        .selected_text(form.p.unidade.rotulo())
                        .show_ui(ui, |ui| {
                            for u in Unidade::TODAS { ui.selectable_value(&mut form.p.unidade, u, u.rotulo()); }
                        });
                    ui.end_row();
                    campo_linha(ui, "Custo (R$)", &mut form.custo);
                    campo_linha(ui, "Venda (R$)", &mut form.preco);
                    campo_linha(ui, "Estoque mínimo", &mut form.minimo);
                    campo_linha(ui, "Foto (caminho)", &mut form.p.foto);
                });
                ui.add_space(4.0);
                if ui.button("Cadastrar").clicked() {
                    salvar = true;
                }
            });

        if salvar {
            match form.aplicar() {
                Ok(p) => match self.db.salvar(&p) {
                    Ok(id) => {
                        self.msg = Some(("peça cadastrada".into(), false));
                        self.novo = None;
                        self.recarregar();
                        self.selecionar(id);
                        return;
                    }
                    Err(e) => self.msg = Some((e, true)),
                },
                Err(e) => self.msg = Some((e, true)),
            }
            self.novo = Some(form);
        } else if aberto {
            self.novo = Some(form);
        }
    }
}

// ----------------------------------------------------------------

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.topo(ctx);
        self.detalhe(ctx);
        self.lista(ctx);
        self.janela_movimento(ctx);
        self.janela_novo(ctx);
    }
}