use crate::aplicativo::{Aplicativo, Pagina};
use crate::components::base::{botao_primario, botao_secundario, cartao, icone};
use crate::recursos::{BANNER_INICIO, icone_do_jogo, icones};
use crate::services::biblioteca::{formatar_duracao, jogos_com_clipes, rotulo_data, ultimos_clipes};
use crate::tema;
use gpui::{AnyElement, Context, FontWeight, ObjectFit, Window, div, img, prelude::*, px};
use zaga_nucleo::protocolo::Pedido;

impl Aplicativo {
    pub(crate) fn pagina_inicio(&mut self, janela: &mut Window, contexto: &mut Context<Self>) -> AnyElement {
        let jogos = jogos_com_clipes(&self.clipes);
        if self
            .filtro_jogo_inicio
            .as_ref()
            .is_some_and(|jogo| !jogos.contains(jogo))
        {
            self.filtro_jogo_inicio = None;
        }
        let largura = (f32::from(janela.viewport_size().width) - 120.0).max(1.0);
        let mostrar_atividade = largura >= 1000.0;
        let largura_grade = if mostrar_atividade { largura - 320.0 } else { largura };
        let colunas = ((largura_grade + 16.0) / 236.0).floor().clamp(1.0, 3.0);
        let largura_cartao = (largura_grade - 16.0 * (colunas - 1.0)) / colunas;
        div()
            .id("pagina_inicio")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_6()
            .px_6()
            .pb_6()
            .child(self.heroi(contexto))
            .children(self.alerta_de_erro())
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap_5()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(self.clipes_recentes_inicio(largura_cartao, contexto)),
                    )
                    .when(mostrar_atividade, |linha| linha.child(self.atividade_recente(contexto))),
            )
            .into_any_element()
    }

    fn heroi(&self, contexto: &mut Context<Self>) -> impl IntoElement {
        let capturando = self.estado.as_ref().is_some_and(|estado| estado.sessao.is_some());
        let duracao = self.estado.as_ref().map_or(30, |estado| estado.config.duracao_clipe);
        let salvar = contexto.listener(move |aplicativo, _, _, contexto| {
            aplicativo.enviar_pedido(Pedido::SalvarClipe { segundos: duracao }, contexto);
            aplicativo.avisar(format!("Salvando os últimos {duracao} segundos"), contexto);
        });
        div()
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .min_h(px(280.0))
            .overflow_hidden()
            .border_b_1()
            .border_color(tema::borda())
            .child(
                img(BANNER_INICIO)
                    .opacity(0.45)
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(ObjectFit::Cover),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .px_5()
                    .py_8()
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .text_size(px(42.0))
                            .font_family(tema::FONTE_TITULOS)
                            .font_weight(FontWeight::BOLD)
                            .child("Pronto pra")
                            .child(div().text_color(tema::roxo_claro()).child("Zagar?")),
                    )
                    .child(
                        div()
                            .max_w(px(360.0))
                            .text_size(px(19.0))
                            .text_color(tema::texto_discreto())
                            .child("Capture, edite e organize seus melhores momentos."),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_4()
                            .mt_3()
                            .child(
                                botao_primario(
                                    "salvar_agora",
                                    Some(icones::tesoura),
                                    format!("Salvar últimos {duracao} s"),
                                    capturando,
                                    salvar,
                                )
                                .h(px(46.0))
                                .px_5()
                                .text_size(px(16.0)),
                            )
                            .child(
                                botao_secundario(
                                    "abrir_biblioteca_inicio",
                                    Some(icones::clipes),
                                    "Ver clipes",
                                    contexto.listener(|aplicativo, _, _, contexto| {
                                        aplicativo.ir_para(Pagina::Clipes, contexto);
                                    }),
                                )
                                .h(px(46.0))
                                .border_color(tema::borda()),
                            ),
                    ),
            )
    }

    fn filtros_inicio(&self, jogos: &[String], contexto: &mut Context<Self>) -> impl IntoElement {
        div().flex().flex_wrap().flex_none().gap_4().children(
            jogos
                .iter()
                .enumerate()
                .map(|(indice, jogo)| self.filtro_de_jogo_inicio(indice, jogo, contexto)),
        )
    }

    fn filtro_de_jogo_inicio(&self, indice: usize, jogo: &str, contexto: &mut Context<Self>) -> AnyElement {
        let selecionado = self.filtro_jogo_inicio.as_deref() == Some(jogo);
        let filtro = jogo.to_owned();
        let titulo = jogo.to_owned();
        let simbolo = icone_do_jogo(jogo).map(|(caminho, cor)| match cor {
            Some(cor) => icone(caminho)
                .size(px(18.0))
                .text_color(gpui::rgb(cor))
                .into_any_element(),
            None => img(caminho)
                .size(px(18.0))
                .rounded_sm()
                .object_fit(ObjectFit::Contain)
                .into_any_element(),
        });
        div()
            .id(("filtro_jogo_inicio", indice))
            .flex()
            .flex_none()
            .items_center()
            .gap_2()
            .h(px(34.0))
            .px_3()
            .rounded_full()
            .text_xs()
            .bg(if selecionado {
                tema::superficie_realce()
            } else {
                tema::painel()
            })
            .border_1()
            .border_color(if selecionado { tema::roxo_claro() } else { tema::borda() })
            .text_color(if selecionado {
                tema::texto()
            } else {
                tema::texto_discreto()
            })
            .cursor_pointer()
            .hover(|estilo| estilo.bg(tema::superficie_realce()).text_color(tema::texto()))
            .on_click(contexto.listener(move |aplicativo, _, _, contexto| {
                aplicativo.filtro_jogo_inicio = if aplicativo.filtro_jogo_inicio.as_deref() == Some(filtro.as_str()) {
                    None
                } else {
                    Some(filtro.clone())
                };
                aplicativo.menu_aberto = None;
                contexto.notify();
            }))
            .children(simbolo)
            .child(titulo)
            .into_any_element()
    }
    fn clipes_recentes_inicio(&self, largura: f32, contexto: &mut Context<Self>) -> AnyElement {
        let clipes = ultimos_clipes(&self.clipes, self.filtro_jogo_inicio.as_deref(), 6);
        let jogos = jogos_com_clipes(&self.clipes);
        div()
            .flex()
            .flex_col()
            .flex_none()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(20.0))
                                    .font_family(tema::FONTE_TITULOS)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Últimos clipes"),
                            )
                            .child(
                                botao_secundario(
                                    "ver_todos_inicio",
                                    None,
                                    "Ver todos",
                                    contexto.listener(|aplicativo, _, _, contexto| {
                                        aplicativo.ir_para(Pagina::Clipes, contexto)
                                    }),
                                )
                                .bg(tema::fundo())
                                .border_color(tema::fundo())
                                .text_color(tema::roxo_claro()),
                            ),
                    )
                    .when(!jogos.is_empty(), |cabecalho| {
                        cabecalho.child(self.filtros_inicio(&jogos, contexto))
                    }),
            )
            .when(clipes.is_empty(), |caixa| {
                caixa.child(
                    div()
                        .p_6()
                        .rounded_xl()
                        .bg(tema::painel())
                        .text_sm()
                        .text_color(tema::texto_discreto())
                        .child("Seus clipes salvos aparecerão aqui."),
                )
            })
            .child(
                div().flex().flex_wrap().gap_4().children(
                    clipes
                        .iter()
                        .enumerate()
                        .map(|(indice, clipe)| self.cartao_de_clipe(indice, clipe, largura, contexto)),
                ),
            )
            .into_any_element()
    }

    fn atividade_recente(&self, contexto: &mut Context<Self>) -> impl IntoElement {
        let recentes = ultimos_clipes(&self.clipes, self.filtro_jogo_inicio.as_deref(), 3);
        cartao()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(300.0))
            .p_4()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(icone(icones::relogio).size_5().text_color(tema::roxo_claro()))
                    .child(
                        div()
                            .font_family(tema::FONTE_TITULOS)
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Atividade recente"),
                    ),
            )
            .when(recentes.is_empty(), |caixa| {
                caixa.child(
                    div()
                        .text_sm()
                        .text_color(tema::texto_discreto())
                        .child("Nenhum clipe salvo ainda."),
                )
            })
            .children(recentes.iter().enumerate().map(|(indice, clipe)| {
                let caminho = clipe.caminho.clone();
                let detalhe = clipe.duracao.map_or_else(
                    || rotulo_data(clipe.modificado),
                    |duracao| format!("{} · {}", rotulo_data(clipe.modificado), formatar_duracao(duracao)),
                );
                div()
                    .id(("atividade_inicio", indice))
                    .flex()
                    .items_center()
                    .gap_3()
                    .py_2()
                    .border_b_1()
                    .border_color(tema::borda())
                    .cursor_pointer()
                    .hover(|estilo| estilo.bg(tema::superficie()))
                    .on_click(contexto.listener(move |aplicativo, _, janela, contexto| {
                        aplicativo.abrir_editor(&caminho, janela, contexto);
                    }))
                    .child(
                        div()
                            .relative()
                            .flex_none()
                            .w(px(72.0))
                            .h(px(44.0))
                            .rounded_md()
                            .overflow_hidden()
                            .bg(tema::superficie())
                            .children(
                                clipe
                                    .miniatura
                                    .clone()
                                    .map(|caminho| img(caminho).size_full().object_fit(ObjectFit::Cover)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .truncate()
                                    .child(clipe.jogo.clone().unwrap_or_else(|| clipe.titulo.clone())),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .truncate()
                                    .text_color(tema::texto_discreto())
                                    .child(detalhe),
                            ),
                    )
            }))
    }

    fn alerta_de_erro(&self) -> Option<impl IntoElement + use<>> {
        let mensagem = self.estado.as_ref()?.ultimo_erro.clone()?;
        Some(
            div()
                .flex_none()
                .px_4()
                .py_3()
                .rounded_lg()
                .text_sm()
                .bg(tema::erro_fundo())
                .border_1()
                .border_color(tema::vermelho())
                .child(mensagem),
        )
    }
}
