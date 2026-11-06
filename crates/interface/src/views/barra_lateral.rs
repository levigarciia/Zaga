use crate::aplicativo::{Aplicativo, Pagina};
use crate::components::base::{icone, indicador};
use crate::recursos::{LOGO, icones};
use crate::tema;
use gpui::{Context, ElementId, FontWeight, WindowControlArea, div, img, prelude::*, px};

impl Aplicativo {
    pub(crate) fn barra_lateral(&self, contexto: &mut Context<Self>) -> impl IntoElement {
        let (ativo, titulo, detalhe) = self.resumo_do_motor();
        div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(210.0))
            .h_full()
            .bg(tema::painel())
            .border_r_1()
            .border_color(tema::borda())
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(px(84.0))
                    .pl(px(14.0))
                    .window_control_area(WindowControlArea::Drag)
                    .child(img(LOGO).w(px(132.0)).h(px(38.0))),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .px_3()
                    .child(self.item_de_navegacao("Início", icones::casa, Pagina::Inicio, contexto))
                    .child(self.item_de_navegacao("Clipes", icones::clipes, Pagina::Clipes, contexto))
                    .child(self.item_de_navegacao(
                        "Configurações",
                        icones::engrenagem,
                        Pagina::Configuracoes,
                        contexto,
                    )),
            )
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_4()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(indicador(ativo))
                            .child(div().text_xs().font_weight(FontWeight::MEDIUM).child(titulo)),
                    )
                    .child(div().text_xs().text_color(tema::texto_discreto()).child(detalhe)),
            )
    }

    fn item_de_navegacao(
        &self,
        rotulo: &'static str,
        simbolo: &'static str,
        pagina: Pagina,
        contexto: &mut Context<Self>,
    ) -> impl IntoElement {
        let ativo = self.pagina == pagina || (pagina == Pagina::Clipes && self.pagina == Pagina::Editor);
        let base = div()
            .id(ElementId::Name(rotulo.into()))
            .flex()
            .items_center()
            .gap_3()
            .h(px(40.0))
            .px_3()
            .rounded_lg()
            .text_sm()
            .cursor_pointer()
            .on_click(contexto.listener(move |aplicativo, _, _janela, contexto| {
                aplicativo.ir_para(pagina, contexto);
            }));
        if ativo {
            base.bg(tema::gradiente_ativo())
                .text_color(tema::texto())
                .font_weight(FontWeight::MEDIUM)
                .child(icone(simbolo).text_color(tema::texto()))
                .child(rotulo)
        } else {
            base.text_color(tema::texto_discreto())
                .hover(|estilo| estilo.bg(tema::superficie()).text_color(tema::texto()))
                .child(icone(simbolo))
                .child(rotulo)
        }
    }

    fn resumo_do_motor(&self) -> (bool, &'static str, &'static str) {
        match &self.estado {
            None => (false, "Conectando", "Iniciando o motor"),
            Some(estado) if estado.sessao.is_some() => (true, "ZAGA ativo", "Capturando em segundo plano"),
            Some(estado) if estado.config.captura_ativa => (false, "Falha na captura", "Veja os detalhes em Início"),
            Some(_) => (false, "Captura pausada", "Nada está sendo gravado"),
        }
    }
}
