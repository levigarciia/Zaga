use crate::aplicativo::Aplicativo;
use crate::components::base::{botao_icone, botao_primario, botao_secundario, icone};
use crate::components::seletor::{OpcaoMenu, menu_flutuante};
use crate::recursos::icones;
use crate::services::biblioteca::{
    Clipe, OrdemClipes, filtrar_clipes, formatar_duracao, formatar_tamanho, rotulo_data,
};
use crate::services::sistema::{copiar_arquivo_para_area_de_transferencia, enviar_para_lixeira};
use crate::tema;
use gpui::{AnyElement, Context, ElementId, FontWeight, ObjectFit, SharedString, Window, div, img, prelude::*, px};
use std::path::{Path, PathBuf};

const LARGURA_BARRA_LATERAL: f32 = 72.0;
const MARGEM_HORIZONTAL: f32 = 48.0;
const ESPACO_ENTRE_CARTOES: f32 = 16.0;
const LARGURA_MINIMA_CARTAO: f32 = 230.0;
const MENU_ORDEM: &str = "ordem_clipes";
const LIMITE_DE_CARTOES: usize = 120;

impl Aplicativo {
    pub(crate) fn pagina_clipes(&mut self, janela: &mut Window, contexto: &mut Context<Self>) -> AnyElement {
        let busca = self.busca.read(contexto).texto().to_owned();
        let visiveis = filtrar_clipes(&self.clipes, &busca, self.ordem);
        let largura_util = f32::from(janela.viewport_size().width) - LARGURA_BARRA_LATERAL - MARGEM_HORIZONTAL;
        let colunas = ((largura_util + ESPACO_ENTRE_CARTOES) / (LARGURA_MINIMA_CARTAO + ESPACO_ENTRE_CARTOES))
            .floor()
            .max(1.0);
        let largura_cartao = (largura_util - ESPACO_ENTRE_CARTOES * (colunas - 1.0)) / colunas;

        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_4()
            .px_6()
            .child(self.cabecalho_de_clipes(contexto))
            .child(self.corpo_de_clipes(&visiveis, largura_cartao, contexto))
            .into_any_element()
    }

    fn cabecalho_de_clipes(&self, contexto: &mut Context<Self>) -> impl IntoElement {
        let total_bytes: u64 = self.clipes.iter().map(|clipe| clipe.bytes).sum();
        let resumo = format!(
            "{} · {}",
            contar_clipes(self.clipes.len()),
            formatar_tamanho(total_bytes)
        );
        div()
            .flex()
            .flex_none()
            .items_start()
            .justify_between()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(24.0))
                            .font_family(tema::FONTE_TITULOS)
                            .font_weight(FontWeight::BOLD)
                            .child("Clipes"),
                    )
                    .child(div().text_sm().text_color(tema::texto_discreto()).child(resumo)),
            )
            .child(self.acoes_de_clipes(contexto))
    }

    fn acoes_de_clipes(&self, contexto: &mut Context<Self>) -> impl IntoElement {
        if self.selecionando {
            return self.acoes_de_selecao(contexto).into_any_element();
        }
        let selecionar = contexto.listener(|aplicativo, _, _janela, contexto| {
            aplicativo.selecionando = true;
            contexto.notify();
        });
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(div().w(px(240.0)).child(self.busca.clone()))
            .child(self.botao_de_ordem(contexto))
            .child(botao_secundario("selecionar", None, "Selecionar", selecionar))
            .into_any_element()
    }

    fn acoes_de_selecao(&self, contexto: &mut Context<Self>) -> impl IntoElement {
        let quantidade = self.selecionados.len();
        let excluir = contexto.listener(|aplicativo, _, _janela, contexto| aplicativo.excluir_selecionados(contexto));
        let cancelar = contexto.listener(|aplicativo, _, _janela, contexto| {
            aplicativo.selecionando = false;
            aplicativo.selecionados.clear();
            contexto.notify();
        });
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .text_color(tema::texto_discreto())
                    .child(format!("{quantidade} selecionado(s)")),
            )
            .child(botao_secundario("cancelar_selecao", None, "Cancelar", cancelar))
            .child(botao_primario(
                "excluir_selecao",
                Some(icones::lixeira),
                "Excluir",
                quantidade > 0,
                excluir,
            ))
    }

    fn botao_de_ordem(&self, contexto: &mut Context<Self>) -> impl IntoElement {
        let abrir = contexto.listener(|aplicativo, _, _janela, contexto| {
            aplicativo.alternar_menu(MENU_ORDEM, contexto);
        });
        let mut caixa = div().relative().flex_none().child(
            botao_icone("abrir_ordem", icones::filtro, abrir)
                .border_1()
                .border_color(tema::borda()),
        );
        if self.menu_esta_aberto(MENU_ORDEM) {
            let opcoes = [
                ("Mais recentes", OrdemClipes::Recentes),
                ("Mais antigos", OrdemClipes::Antigos),
                ("Maiores", OrdemClipes::Maiores),
            ]
            .into_iter()
            .map(|(rotulo, ordem)| {
                let entidade = contexto.entity();
                OpcaoMenu::new(rotulo, self.ordem == ordem, move |_janela, contexto| {
                    entidade.update(contexto, |aplicativo, contexto| {
                        aplicativo.ordem = ordem;
                        aplicativo.menu_aberto = None;
                        contexto.notify();
                    });
                })
            })
            .collect();
            caixa = caixa.child(
                div()
                    .absolute()
                    .right_0()
                    .top_full()
                    .child(menu_flutuante(MENU_ORDEM.into(), opcoes)),
            );
        }
        caixa
    }

    fn corpo_de_clipes(&self, visiveis: &[Clipe], largura: f32, contexto: &mut Context<Self>) -> AnyElement {
        if visiveis.is_empty() {
            return estado_vazio(self.clipes.is_empty()).into_any_element();
        }
        let cartoes: Vec<AnyElement> = visiveis
            .iter()
            .take(LIMITE_DE_CARTOES)
            .enumerate()
            .map(|(indice, clipe)| self.cartao_de_clipe(indice, clipe, largura, contexto))
            .collect();
        div()
            .id("grade_clipes")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .pb_6()
            .child(div().flex().flex_wrap().gap(px(ESPACO_ENTRE_CARTOES)).children(cartoes))
            .into_any_element()
    }

    pub(crate) fn cartao_de_clipe(
        &self,
        indice: usize,
        clipe: &Clipe,
        largura: f32,
        contexto: &mut Context<Self>,
    ) -> AnyElement {
        let caminho = clipe.caminho.clone();
        let selecionado = self.selecionados.contains(&clipe.caminho);
        let clicar = contexto.listener(move |aplicativo, _, janela, contexto| {
            if aplicativo.selecionando {
                aplicativo.alternar_selecao(&caminho, contexto);
            } else {
                aplicativo.abrir_editor(&caminho, janela, contexto);
            }
        });
        div()
            .id(("cartao_clipe", indice))
            .flex()
            .flex_col()
            .flex_none()
            .w(px(largura))
            .rounded_md()
            .bg(tema::painel())
            .border_1()
            .border_color(if selecionado { tema::roxo_claro() } else { tema::borda() })
            .cursor_pointer()
            .hover(|estilo| estilo.border_color(tema::texto_discreto()))
            .on_click(clicar)
            .child(miniatura_do_clipe(clipe, largura, self.selecionando, selecionado))
            .child(self.rodape_do_cartao(indice, clipe, contexto))
            .into_any_element()
    }

    fn rodape_do_cartao(&self, indice: usize, clipe: &Clipe, contexto: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_1()
            .pl_3()
            .pr_1()
            .py_2()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(clipe.titulo.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_xs()
                            .text_color(tema::texto_discreto())
                            .children(
                                clipe
                                    .jogo
                                    .as_deref()
                                    .map(|jogo| crate::recursos::simbolo_do_jogo(jogo, 16.0)),
                            )
                            .children(
                                clipe
                                    .automatico
                                    .then(|| icone(icones::tesoura).size_3p5().text_color(tema::roxo_claro())),
                            )
                            .child(rotulo_data(clipe.modificado)),
                    ),
            )
            .children((!self.selecionando).then(|| self.menu_do_clipe(indice, clipe, contexto)))
    }

    fn menu_do_clipe(&self, indice: usize, clipe: &Clipe, contexto: &mut Context<Self>) -> impl IntoElement {
        let id_menu: SharedString = format!("menu_clipe_{indice}").into();
        let id_abrir = id_menu.clone();
        let abrir = contexto.listener(move |aplicativo, _, janela, contexto| {
            let _ = janela;
            contexto.stop_propagation();
            aplicativo.alternar_menu(id_abrir.clone(), contexto);
        });
        let mut caixa = div().relative().flex_none().child(botao_icone(
            ElementId::NamedInteger("mais".into(), indice as u64),
            icones::mais,
            abrir,
        ));
        if self.menu_esta_aberto(&id_menu) {
            let opcoes = self.opcoes_do_clipe(&clipe.caminho, contexto);
            caixa = caixa.child(
                div()
                    .absolute()
                    .right_0()
                    .top_full()
                    .child(menu_flutuante(id_menu, opcoes)),
            );
        }
        caixa
    }

    fn opcoes_do_clipe(&self, caminho: &Path, contexto: &mut Context<Self>) -> Vec<OpcaoMenu> {
        let entidade = contexto.entity();
        let copiar = caminho.to_path_buf();
        let revelar = caminho.to_path_buf();
        let excluir = caminho.to_path_buf();
        let (para_copiar, para_revelar, para_excluir) = (entidade.clone(), entidade.clone(), entidade);
        vec![
            OpcaoMenu::new("Copiar para compartilhar", false, move |_janela, contexto| {
                para_copiar.update(contexto, |aplicativo, contexto| {
                    aplicativo.menu_aberto = None;
                    aplicativo.copiar_clipe(&copiar, contexto);
                });
            }),
            OpcaoMenu::new("Mostrar na pasta", false, move |_janela, contexto| {
                contexto.reveal_path(&revelar);
                para_revelar.update(contexto, |aplicativo, contexto| {
                    aplicativo.menu_aberto = None;
                    contexto.notify();
                });
            }),
            OpcaoMenu::new("Excluir", false, move |_janela, contexto| {
                para_excluir.update(contexto, |aplicativo, contexto| {
                    aplicativo.menu_aberto = None;
                    aplicativo.excluir_clipes(&[excluir.clone()], contexto);
                });
            }),
        ]
    }

    fn alternar_selecao(&mut self, caminho: &Path, contexto: &mut Context<Self>) {
        if !self.selecionados.remove(caminho) {
            self.selecionados.insert(caminho.to_path_buf());
        }
        contexto.notify();
    }

    fn excluir_selecionados(&mut self, contexto: &mut Context<Self>) {
        let caminhos: Vec<PathBuf> = std::mem::take(&mut self.selecionados).into_iter().collect();
        self.selecionando = false;
        self.excluir_clipes(&caminhos, contexto);
    }

    pub(crate) fn excluir_clipes(&mut self, caminhos: &[PathBuf], contexto: &mut Context<Self>) {
        let referencias: Vec<&Path> = caminhos.iter().map(PathBuf::as_path).collect();
        match enviar_para_lixeira(&referencias) {
            Ok(()) => {
                self.clipes.retain(|clipe| !caminhos.contains(&clipe.caminho));
                self.avisar("Enviado para a Lixeira", contexto);
            }
            Err(erro) => self.avisar(format!("{erro:#}"), contexto),
        }
    }

    pub(crate) fn copiar_clipe(&mut self, caminho: &Path, contexto: &mut Context<Self>) {
        match copiar_arquivo_para_area_de_transferencia(caminho) {
            Ok(()) => self.avisar("Clipe copiado. Agora é só colar no Discord ou onde quiser.", contexto),
            Err(erro) => self.avisar(format!("{erro:#}"), contexto),
        }
    }
}

fn contar_clipes(quantidade: usize) -> String {
    if quantidade == 1 {
        "1 clipe".to_owned()
    } else {
        format!("{quantidade} clipes")
    }
}

fn miniatura_do_clipe(clipe: &Clipe, largura: f32, selecionando: bool, selecionado: bool) -> impl IntoElement + use<> {
    let altura = (largura - 2.0) * 9.0 / 16.0;
    let imagem = clipe.miniatura.clone().map(|caminho| {
        img(caminho)
            .absolute()
            .inset_0()
            .size_full()
            .rounded_t_md()
            .object_fit(ObjectFit::Cover)
            .into_any_element()
    });
    let vazio = imagem.is_none().then(|| {
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .child(icone(icones::clipes).size_6())
    });
    let duracao = clipe.duracao.map(|duracao| {
        div()
            .absolute()
            .bottom_2()
            .right_2()
            .px(px(6.0))
            .py(px(1.0))
            .rounded_md()
            .text_xs()
            .bg(gpui::rgba(0x000000b8))
            .child(formatar_duracao(duracao))
    });
    let marcador = selecionando.then(|| {
        let caixa = div()
            .absolute()
            .top_2()
            .left_2()
            .flex()
            .items_center()
            .justify_center()
            .size(px(22.0))
            .rounded_md()
            .border_1()
            .border_color(tema::texto());
        if selecionado {
            caixa
                .bg(tema::roxo())
                .child(icone(icones::confirmar).size_3p5().text_color(tema::texto()))
        } else {
            caixa.bg(gpui::rgba(0x00000080))
        }
    });
    div()
        .relative()
        .flex_none()
        .w_full()
        .h(px(altura))
        .overflow_hidden()
        .rounded_t_md()
        .bg(tema::superficie())
        .children(imagem)
        .children(vazio)
        .children(duracao)
        .children(marcador)
}

fn estado_vazio(sem_clipes: bool) -> impl IntoElement {
    let (titulo, detalhe) = if sem_clipes {
        (
            "Nenhum clipe ainda",
            "Aperte Alt+F9 durante o jogo para salvar os últimos 30 segundos.",
        )
    } else {
        ("Nada encontrado", "Tente buscar por outro nome.")
    };
    div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
        .child(icone(icones::clipes).size(px(40.0)).text_color(tema::roxo_claro()))
        .child(
            div()
                .mt_2()
                .font_family(tema::FONTE_TITULOS)
                .font_weight(FontWeight::SEMIBOLD)
                .child(titulo),
        )
        .child(div().text_sm().text_color(tema::texto_discreto()).child(detalhe))
}
