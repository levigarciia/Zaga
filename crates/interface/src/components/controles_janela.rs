use crate::components::base::icone;
use crate::recursos::icones;
use crate::tema;
use gpui::{Div, ElementId, Stateful, WindowControlArea, div, prelude::*, px};

fn controle(id: &'static str, simbolo: &'static str, area: WindowControlArea) -> Stateful<Div> {
    div()
        .id(ElementId::Name(id.into()))
        .flex()
        .items_center()
        .justify_center()
        .w(px(44.0))
        .h_full()
        .window_control_area(area)
        .child(icone(simbolo).size(px(14.0)).text_color(tema::texto()))
}

/// Botões de minimizar, maximizar e fechar. Usam a área não cliente do Windows, então encaixes de janela funcionam.
pub fn controles_janela() -> Div {
    div()
        .flex()
        .flex_none()
        .h_full()
        .child(
            controle("minimizar", icones::minimizar, WindowControlArea::Min)
                .hover(|estilo| estilo.bg(tema::superficie_realce())),
        )
        .child(
            controle("maximizar", icones::maximizar, WindowControlArea::Max)
                .hover(|estilo| estilo.bg(tema::superficie_realce())),
        )
        .child(controle("fechar", icones::fechar, WindowControlArea::Close).hover(|estilo| estilo.bg(tema::vermelho())))
}
