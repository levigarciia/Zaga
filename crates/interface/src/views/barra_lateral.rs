use crate::aplicativo::{Aplicativo, Pagina};
use crate::components::base::icone;
use crate::recursos::{SIMBOLO, icones};
use crate::tema;
use gpui::{Context, ElementId, WindowControlArea, div, img, prelude::*, px};

impl Aplicativo {
    pub(crate) fn barra_lateral(&self, contexto: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .flex_none()
            .items_center()
            .w(px(72.0))
            .h_full()
            .bg(tema::painel())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .w_full()
                    .h(px(52.0))
                    .window_control_area(WindowControlArea::Drag)
                    .child(img(SIMBOLO).size(px(36.0))),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .pt_4()
                    .child(self.item_de_navegacao("Início", icones::casa, Pagina::Inicio, contexto))
                    .child(self.item_de_navegacao("Clipes", icones::clipes, Pagina::Clipes, contexto)),
            )
            .child(div().flex_1())
            .child(div().pb_4().child(self.item_de_navegacao(
                "Ajustes",
                icones::engrenagem,
                Pagina::Configuracoes,
                contexto,
            )))
    }

    fn item_de_navegacao(
        &self,
        rotulo: &'static str,
        simbolo: &'static str,
        pagina: Pagina,
        contexto: &mut Context<Self>,
    ) -> impl IntoElement {
        let ativo = self.pagina == pagina || (pagina == Pagina::Clipes && self.pagina == Pagina::Editor);
        let cor = if ativo {
            tema::roxo_claro()
        } else {
            tema::texto_discreto()
        };
        div()
            .id(ElementId::Name(rotulo.into()))
            .flex()
            .items_center()
            .justify_center()
            .size(px(44.0))
            .rounded_lg()
            .text_size(px(10.0))
            .text_color(cor)
            .cursor_pointer()
            .when(ativo, |item| item.bg(tema::superficie_realce()))
            .hover(|estilo| estilo.bg(tema::superficie_realce()).text_color(tema::texto()))
            .child(icone(simbolo).size(px(22.0)).text_color(cor))
            .on_click(contexto.listener(move |aplicativo, _, _janela, contexto| {
                aplicativo.ir_para(pagina, contexto);
            }))
    }
}
