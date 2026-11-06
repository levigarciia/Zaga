use crate::tema;
use gpui::{
    App, ClickEvent, Div, ElementId, FontWeight, SharedString, Stateful, Svg, Window, div, prelude::*, px, svg,
};

pub fn icone(caminho: &'static str) -> Svg {
    svg()
        .path(caminho)
        .size_4()
        .flex_none()
        .text_color(tema::texto_discreto())
}

pub fn cartao() -> Div {
    div()
        .rounded_xl()
        .bg(tema::painel())
        .border_1()
        .border_color(tema::borda())
}

pub fn divisor() -> Div {
    div().h(px(1.0)).w_full().flex_none().bg(tema::borda())
}

pub fn botao_primario(
    id: impl Into<ElementId>,
    simbolo: Option<&'static str>,
    rotulo: impl Into<SharedString>,
    habilitado: bool,
    ao_clicar: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let base = div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap_2()
        .h(px(36.0))
        .px_4()
        .rounded_lg()
        .text_sm()
        .font_weight(FontWeight::MEDIUM)
        .children(simbolo.map(|caminho| icone(caminho).text_color(tema::texto())))
        .child(rotulo.into());
    if !habilitado {
        return base.bg(tema::superficie_realce()).text_color(tema::texto_discreto());
    }
    base.bg(tema::gradiente_primario())
        .text_color(tema::texto())
        .cursor_pointer()
        .hover(|estilo| estilo.bg(tema::roxo_claro()))
        .on_click(ao_clicar)
}

pub fn botao_secundario(
    id: impl Into<ElementId>,
    simbolo: Option<&'static str>,
    rotulo: impl Into<SharedString>,
    ao_clicar: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap_2()
        .h(px(36.0))
        .px_4()
        .rounded_lg()
        .text_sm()
        .bg(tema::superficie())
        .border_1()
        .border_color(tema::borda())
        .text_color(tema::texto())
        .cursor_pointer()
        .hover(|estilo| estilo.bg(tema::superficie_realce()))
        .children(simbolo.map(icone))
        .child(rotulo.into())
        .on_click(ao_clicar)
}

pub fn botao_icone(
    id: impl Into<ElementId>,
    simbolo: &'static str,
    ao_clicar: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(36.0))
        .rounded_lg()
        .cursor_pointer()
        .hover(|estilo| estilo.bg(tema::superficie_realce()))
        .child(icone(simbolo).text_color(tema::texto()))
        .on_click(ao_clicar)
}

pub fn alternador(
    id: impl Into<ElementId>,
    ligado: bool,
    ao_clicar: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let trilho = div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .w(px(44.0))
        .h(px(24.0))
        .p(px(3.0))
        .rounded_full()
        .cursor_pointer()
        .on_click(ao_clicar);
    let botao = div().size(px(18.0)).rounded_full().bg(tema::texto());
    if ligado {
        trilho.justify_end().bg(tema::gradiente_primario()).child(botao)
    } else {
        trilho.justify_start().bg(tema::superficie_realce()).child(botao)
    }
}

pub fn linha_de_ajuste(
    simbolo: &'static str,
    rotulo: &'static str,
    descricao: Option<&'static str>,
    controle: impl IntoElement,
) -> Div {
    let textos = div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .child(div().text_sm().child(rotulo))
        .children(descricao.map(|texto| div().text_xs().text_color(tema::texto_discreto()).child(texto)));
    div()
        .flex()
        .items_center()
        .gap_3()
        .min_h(px(52.0))
        .px_4()
        .py_2()
        .child(icone(simbolo))
        .child(textos.flex_1())
        .child(controle)
}

pub fn indicador(ativo: bool) -> Div {
    div()
        .flex_none()
        .size(px(9.0))
        .rounded_full()
        .bg(if ativo { tema::verde() } else { tema::texto_discreto() })
}
