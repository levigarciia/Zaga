use crate::components::base::icone;
use crate::tema;
use gpui::{
    App, Bounds, ClipboardItem, Context, Div, ElementInputHandler, Entity, EntityInputHandler, EventEmitter,
    FocusHandle, Focusable, KeyDownEvent, Pixels, Point, Render, SharedString, UTF16Selection, Window, canvas, div,
    prelude::*, px,
};
use std::ops::Range;

pub enum EventoCampo {
    Confirmado,
    Cancelado,
}

/// Campo de uma linha com o cursor sempre no fim, suficiente para busca e renomeação.
///
/// Recebe texto pelo sistema de entrada do Windows, então acentos e dead keys funcionam.
pub struct CampoTexto {
    texto: String,
    marcado: Option<Range<usize>>,
    dica: SharedString,
    simbolo: Option<&'static str>,
    foco: FocusHandle,
}

impl EventEmitter<EventoCampo> for CampoTexto {}

impl Focusable for CampoTexto {
    fn focus_handle(&self, _contexto: &App) -> FocusHandle {
        self.foco.clone()
    }
}

impl CampoTexto {
    pub fn new(dica: &'static str, simbolo: Option<&'static str>, contexto: &mut Context<Self>) -> Self {
        Self {
            texto: String::new(),
            marcado: None,
            dica: dica.into(),
            simbolo,
            foco: contexto.focus_handle(),
        }
    }

    pub fn texto(&self) -> &str {
        &self.texto
    }

    pub fn definir_texto(&mut self, texto: &str) {
        self.texto = texto.to_owned();
        self.marcado = None;
    }

    pub fn focar(&self, janela: &mut Window) {
        self.foco.focus(janela);
    }

    fn tratar_tecla(&mut self, evento: &KeyDownEvent, _janela: &mut Window, contexto: &mut Context<Self>) {
        let tecla = &evento.keystroke;
        match tecla.key.as_str() {
            "backspace" => {
                self.texto.pop();
                self.marcado = None;
                contexto.notify();
            }
            "enter" => contexto.emit(EventoCampo::Confirmado),
            "escape" => contexto.emit(EventoCampo::Cancelado),
            "v" if tecla.modifiers.control => self.colar(contexto),
            _ => {}
        }
    }

    fn colar(&mut self, contexto: &mut Context<Self>) {
        let colado = contexto
            .read_from_clipboard()
            .and_then(|item: ClipboardItem| item.text());
        if let Some(colado) = colado {
            self.texto.push_str(&colado.replace(['\r', '\n'], " "));
            contexto.notify();
        }
    }

    fn fim_utf16(&self) -> usize {
        self.texto.encode_utf16().count()
    }
}

impl EntityInputHandler for CampoTexto {
    fn text_for_range(
        &mut self,
        intervalo: Range<usize>,
        ajustado: &mut Option<Range<usize>>,
        _janela: &mut Window,
        _contexto: &mut Context<Self>,
    ) -> Option<String> {
        let unidades: Vec<u16> = self.texto.encode_utf16().collect();
        let fim = intervalo.end.min(unidades.len());
        let inicio = intervalo.start.min(fim);
        *ajustado = Some(inicio..fim);
        Some(String::from_utf16_lossy(&unidades[inicio..fim]))
    }

    fn selected_text_range(
        &mut self,
        _ignorar: bool,
        _janela: &mut Window,
        _contexto: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let fim = self.fim_utf16();
        Some(UTF16Selection {
            range: fim..fim,
            reversed: false,
        })
    }

    fn marked_text_range(&self, _janela: &mut Window, _contexto: &mut Context<Self>) -> Option<Range<usize>> {
        self.marcado.clone()
    }

    fn unmark_text(&mut self, _janela: &mut Window, _contexto: &mut Context<Self>) {
        self.marcado = None;
    }

    fn replace_text_in_range(
        &mut self,
        intervalo: Option<Range<usize>>,
        texto: &str,
        _janela: &mut Window,
        contexto: &mut Context<Self>,
    ) {
        let alvo = intervalo.or_else(|| self.marcado.take());
        if let Some(alvo) = alvo {
            substituir_utf16(&mut self.texto, alvo, texto);
        } else {
            self.texto.push_str(texto);
        }
        self.marcado = None;
        contexto.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        intervalo: Option<Range<usize>>,
        texto: &str,
        _selecao: Option<Range<usize>>,
        _janela: &mut Window,
        contexto: &mut Context<Self>,
    ) {
        let fim = self.fim_utf16();
        let alvo = intervalo.or_else(|| self.marcado.clone()).unwrap_or(fim..fim);
        let inicio = alvo.start;
        substituir_utf16(&mut self.texto, alvo, texto);
        let tamanho = texto.encode_utf16().count();
        self.marcado = (tamanho > 0).then_some(inicio..inicio + tamanho);
        contexto.notify();
    }

    fn bounds_for_range(
        &mut self,
        _intervalo: Range<usize>,
        limites: Bounds<Pixels>,
        _janela: &mut Window,
        _contexto: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(limites)
    }

    fn character_index_for_point(
        &mut self,
        _ponto: Point<Pixels>,
        _janela: &mut Window,
        _contexto: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.fim_utf16())
    }
}

impl Render for CampoTexto {
    fn render(&mut self, janela: &mut Window, contexto: &mut Context<Self>) -> impl IntoElement {
        let focado = self.foco.is_focused(janela);
        let foco = self.foco.clone();
        let entidade: Entity<Self> = contexto.entity();
        let ponte_de_entrada = canvas(
            |_, _, _| {},
            move |limites, _, janela, contexto| {
                janela.handle_input(&foco, ElementInputHandler::new(limites, entidade), contexto);
            },
        )
        .absolute()
        .size_full();

        let vazio = self.texto.is_empty();
        let conteudo = if vazio {
            self.dica.clone()
        } else {
            self.texto.clone().into()
        };

        div()
            .id("campo_texto")
            .relative()
            .track_focus(&self.foco)
            .key_context("CampoTexto")
            .on_key_down(contexto.listener(Self::tratar_tecla))
            .flex()
            .items_center()
            .gap_2()
            .w_full()
            .h(px(36.0))
            .px_3()
            .rounded_lg()
            .text_sm()
            .bg(tema::superficie())
            .border_1()
            .border_color(if focado { tema::roxo() } else { tema::borda() })
            .cursor_text()
            .children(self.simbolo.map(icone))
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .when(vazio && focado, |linha| linha.child(cursor()))
                    .child(
                        div()
                            .whitespace_nowrap()
                            .text_color(if vazio { tema::texto_discreto() } else { tema::texto() })
                            .child(conteudo),
                    )
                    .when(!vazio && focado, |linha| linha.child(cursor())),
            )
            .child(ponte_de_entrada)
    }
}

fn cursor() -> Div {
    div().w(px(1.5)).h(px(16.0)).flex_none().bg(tema::roxo_claro())
}

fn substituir_utf16(texto: &mut String, intervalo: Range<usize>, novo: &str) {
    let unidades: Vec<u16> = texto.encode_utf16().collect();
    let fim = intervalo.end.min(unidades.len());
    let inicio = intervalo.start.min(fim);
    let mut resultado = String::from_utf16_lossy(&unidades[..inicio]);
    resultado.push_str(novo);
    resultado.push_str(&String::from_utf16_lossy(&unidades[fim..]));
    *texto = resultado;
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn substitui_respeitando_acentos() {
        let mut texto = String::from("ação");
        substituir_utf16(&mut texto, 1..3, "gi");
        assert_eq!(texto, "agio");
    }

    #[test]
    fn intervalo_alem_do_fim_e_limitado() {
        let mut texto = String::from("abc");
        substituir_utf16(&mut texto, 2..99, "z");
        assert_eq!(texto, "abz");
    }
}
