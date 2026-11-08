use crate::aplicativo::{AbaConfig, Aplicativo, Pagina};
use crate::components::base::indicador;
use crate::components::controles_janela::controles_janela;
use crate::components::seletor::{OpcaoMenu, menu_flutuante};
use crate::tema;
use gpui::{Context, FontWeight, WindowControlArea, div, prelude::*, px};
use zaga_nucleo::config::{DURACOES_CLIPE, Qualidade};

impl Aplicativo {
    fn resumo_do_motor(&self) -> (bool, &'static str) {
        match &self.estado {
            None => (false, "Conectando"),
            Some(estado) if estado.sessao.is_some() => (true, "Captura ativa"),
            Some(estado) if estado.config.captura_ativa => (false, "Falha na captura"),
            Some(_) => (false, "Captura pausada"),
        }
    }

    pub(crate) fn barra_superior(&self, contexto: &mut Context<Self>) -> impl IntoElement {
        let (ativo, titulo) = self.resumo_do_motor();
        let jogo = self.estado.as_ref().and_then(|estado| estado.jogo_ativo.as_deref());
        div()
            .flex()
            .flex_none()
            .items_center()
            .h(px(52.0))
            .bg(tema::painel())
            .border_b_1()
            .border_color(tema::borda())
            .child(
                div()
                    .id("estado_captura")
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap_2()
                    .px_4()
                    .h_full()
                    .text_xs()
                    .text_color(tema::texto_discreto())
                    .cursor_pointer()
                    .hover(|estilo| estilo.bg(tema::superficie()))
                    .children(jogo.map(|nome| crate::recursos::simbolo_do_jogo(nome, 28.0)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(div().flex().items_center().gap_2().child(indicador(ativo)).child(
                                if jogo.is_some() && ativo {
                                    "Clipando agora"
                                } else {
                                    titulo
                                },
                            ))
                            .children(jogo.map(|nome| {
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(tema::texto())
                                    .child(nome.to_owned())
                            })),
                    )
                    .on_click(contexto.listener(|aplicativo, _, _, contexto| {
                        aplicativo.alterar_config(|config| config.captura_ativa = !config.captura_ativa, contexto);
                    })),
            )
            .children(
                DURACOES_CLIPE
                    .into_iter()
                    .enumerate()
                    .map(|(indice, segundos)| self.atalho_de_captura(indice, segundos, ativo, contexto)),
            )
            .child(div().flex_1().h_full().window_control_area(WindowControlArea::Drag))
            .child(controles_janela())
    }

    fn atalho_de_captura(
        &self,
        indice: usize,
        segundos: u32,
        _ativo: bool,
        contexto: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let atalho = self
            .estado
            .as_ref()
            .map_or_else(|| "...".to_owned(), |estado| estado.config.atalhos[indice].descricao());
        let id = format!("ajustes_clipe_{indice}");
        let aberto = self.menu_esta_aberto(&id);
        let menu = self.menu_de_ajustes_do_atalho(indice, contexto);
        div()
            .relative()
            .id(("capturar_pela_barra", indice))
            .flex()
            .flex_none()
            .items_center()
            .gap_2()
            .h(px(32.0))
            .px_3()
            .rounded_md()
            .text_xs()
            .text_color(tema::texto_discreto())
            .child(
                div()
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(tema::superficie_realce())
                    .child(atalho),
            )
            .child(format!("Clipe {segundos} s"))
            .cursor_pointer()
            .hover(|estilo| estilo.bg(tema::superficie()).text_color(tema::texto()))
            .on_click(contexto.listener(move |aplicativo, _, _, contexto| {
                aplicativo.alternar_menu(id.clone(), contexto);
            }))
            .when(aberto, |botao| {
                botao.child(div().absolute().right_0().top_full().child(menu))
            })
    }

    fn menu_de_ajustes_do_atalho(&self, indice: usize, contexto: &mut Context<Self>) -> gpui::AnyElement {
        let entidade = contexto.entity();
        let editar = entidade.clone();
        let mut opcoes = vec![OpcaoMenu::new("Alterar atalho", false, move |janela, cx| {
            editar.update(cx, |app, cx| {
                app.ir_para(Pagina::Configuracoes, cx);
                app.aba = AbaConfig::Atalhos;
                app.iniciar_gravacao_de_atalho(indice, janela, cx);
            });
        })];
        let atual = self.config_atual().qualidade;
        for (qualidade, rotulo) in [
            (Qualidade::Economica, "Qualidade econômica (todos os clipes)"),
            (Qualidade::Alta, "Qualidade alta (todos os clipes)"),
            (Qualidade::Maxima, "Qualidade máxima (todos os clipes)"),
        ] {
            let entidade = entidade.clone();
            opcoes.push(OpcaoMenu::new(rotulo, atual == qualidade, move |_, cx| {
                entidade.update(cx, |app, cx| {
                    app.alterar_config(|config| config.qualidade = qualidade, cx);
                });
            }));
        }
        opcoes.push(OpcaoMenu::new("Resolução e FPS", false, move |_, cx| {
            entidade.update(cx, |app, cx| {
                app.ir_para(Pagina::Configuracoes, cx);
                app.aba = AbaConfig::Captura;
            });
        }));
        menu_flutuante(format!("menu_clipe_{indice}").into(), opcoes)
    }
}
