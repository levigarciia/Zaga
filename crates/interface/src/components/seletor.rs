use crate::components::base::icone;
use crate::recursos::icones;
use crate::tema;
use gpui::{
    AnyElement, App, ClickEvent, Corner, ElementId, SharedString, Window, anchored, deferred, div, point, prelude::*,
    px,
};
use std::rc::Rc;

type AcaoOpcao = Rc<dyn Fn(&mut Window, &mut App)>;

/// Uma escolha de um menu suspenso.
pub struct OpcaoMenu {
    pub rotulo: SharedString,
    pub ativa: bool,
    pub acao: AcaoOpcao,
}

impl OpcaoMenu {
    pub fn new(rotulo: impl Into<SharedString>, ativa: bool, acao: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        Self {
            rotulo: rotulo.into(),
            ativa,
            acao: Rc::new(acao),
        }
    }
}

/// Lista de opções flutuante, desenhada acima do restante da tela.
pub fn menu_flutuante(id: SharedString, opcoes: Vec<OpcaoMenu>) -> AnyElement {
    let itens = opcoes.into_iter().enumerate().map(|(indice, opcao)| {
        let acao = opcao.acao.clone();
        div()
            .id(ElementId::NamedInteger(id.clone(), indice as u64))
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .px_3()
            .py(px(7.0))
            .rounded_md()
            .text_sm()
            .cursor_pointer()
            .hover(|estilo| estilo.bg(tema::superficie_realce()))
            .child(opcao.rotulo)
            .children(
                opcao
                    .ativa
                    .then(|| icone(icones::confirmar).size_3p5().text_color(tema::roxo_claro())),
            )
            .on_click(move |_, janela, contexto| {
                contexto.stop_propagation();
                acao(janela, contexto);
            })
    });
    let lista = div()
        .occlude()
        .flex()
        .flex_col()
        .min_w(px(170.0))
        .p_1()
        .rounded_lg()
        .bg(tema::superficie())
        .border_1()
        .border_color(tema::borda())
        .shadow_lg()
        .children(itens);
    deferred(
        anchored()
            .anchor(Corner::TopRight)
            .offset(point(px(0.0), px(4.0)))
            .snap_to_window()
            .child(lista),
    )
    .with_priority(2)
    .into_any_element()
}

pub fn seletor(
    id: &'static str,
    atual: impl Into<SharedString>,
    aberto: bool,
    alternar: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    opcoes: impl FnOnce() -> Vec<OpcaoMenu>,
) -> impl IntoElement {
    let botao = div()
        .id(ElementId::Name(id.into()))
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .w(px(190.0))
        .h(px(36.0))
        .px_3()
        .rounded_lg()
        .text_sm()
        .bg(tema::superficie())
        .border_1()
        .border_color(if aberto { tema::roxo() } else { tema::borda() })
        .cursor_pointer()
        .hover(|estilo| estilo.bg(tema::superficie_realce()))
        .child(atual.into())
        .child(icone(icones::divisa))
        .on_click(alternar);
    let mut caixa = div().relative().flex_none().child(botao);
    if aberto {
        let ancora = div()
            .absolute()
            .right_0()
            .top_full()
            .child(menu_flutuante(id.into(), opcoes()));
        caixa = caixa.child(ancora);
    }
    caixa
}
