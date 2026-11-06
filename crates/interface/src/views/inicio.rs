use crate::aplicativo::Aplicativo;
use crate::components::base::{alternador, botao_primario, cartao, divisor, icone, indicador};
use crate::recursos::{SIMBOLO, icones};
use crate::tema;
use gpui::{AnyElement, Context, FontWeight, div, img, prelude::*, px};
use zaga_nucleo::protocolo::{EstadoMotor, Pedido};

impl Aplicativo {
    pub(crate) fn pagina_inicio(&mut self, contexto: &mut Context<Self>) -> AnyElement {
        div()
            .id("pagina_inicio")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_4()
            .px_6()
            .pb_6()
            .child(self.heroi(contexto))
            .child(
                div().text_sm().text_color(tema::texto_discreto()).child(
                    self.estado
                        .as_ref()
                        .and_then(|estado| estado.jogo_ativo.as_ref())
                        .map_or_else(
                            || "Nenhum jogo reconhecido em primeiro plano".to_owned(),
                            |jogo| format!("Jogo detectado: {jogo}"),
                        ),
                ),
            )
            .children(self.alerta_de_erro())
            .child(self.cartao_de_captura(contexto))
            .into_any_element()
    }

    fn heroi(&self, contexto: &mut Context<Self>) -> impl IntoElement {
        let capturando = self.estado.as_ref().is_some_and(|estado| estado.sessao.is_some());
        let duracao = self.estado.as_ref().map_or(30, |estado| estado.config.duracao_clipe);
        let descricao = if capturando {
            "O ZAGA está rodando e pronto para capturar suas melhores jogadas."
        } else {
            "Ligue o buffer de gravação para guardar suas melhores jogadas."
        };
        let salvar = contexto.listener(move |aplicativo, _, _janela, contexto| {
            aplicativo.enviar_pedido(Pedido::SalvarClipe { segundos: duracao }, contexto);
            aplicativo.avisar(format!("Salvando os últimos {duracao} segundos"), contexto);
        });
        div()
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .justify_center()
            .gap_3()
            .min_h(px(300.0))
            .px_10()
            .py_8()
            .rounded_xl()
            .overflow_hidden()
            .bg(tema::gradiente_heroi())
            .border_1()
            .border_color(tema::borda())
            .child(
                img(SIMBOLO)
                    .absolute()
                    .right(px(36.0))
                    .top(px(18.0))
                    .size(px(300.0))
                    .opacity(0.9),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .text_size(px(40.0))
                    .font_weight(FontWeight::BOLD)
                    .child("Pronto pra")
                    .child(div().text_color(tema::roxo_claro()).child("zagar?")),
            )
            .child(
                div()
                    .max_w(px(380.0))
                    .text_color(tema::texto_discreto())
                    .child(descricao),
            )
            .child(div().flex().items_center().gap_3().mt_3().child(botao_primario(
                "salvar_agora",
                Some(icones::tesoura),
                format!("Salvar últimos {duracao} s"),
                capturando,
                salvar,
            )))
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

    fn cartao_de_captura(&self, contexto: &mut Context<Self>) -> impl IntoElement {
        let ligada = self.estado.as_ref().is_some_and(|estado| estado.config.captura_ativa);
        let gravando = self.estado.as_ref().is_some_and(|estado| estado.sessao.is_some());
        let duracao = self.estado.as_ref().map_or(30, |estado| estado.config.duracao_clipe);
        let alternar = contexto.listener(|aplicativo, _, _janela, contexto| {
            aplicativo.alterar_config(|config| config.captura_ativa = !config.captura_ativa, contexto);
        });
        cartao()
            .flex()
            .flex_col()
            .flex_none()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .px_5()
                    .py_4()
                    .child(indicador(gravando).size(px(12.0)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .gap(px(2.0))
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Buffer de gravação"))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(tema::texto_discreto())
                                    .child(format!("Guarda os últimos {duracao} segundos da sua gameplay.")),
                            ),
                    )
                    .child(alternador("alternar_captura", ligada, alternar)),
            )
            .child(divisor())
            .child(self.linha_de_indicadores())
    }

    fn linha_de_indicadores(&self) -> impl IntoElement {
        let estado = self.estado.as_ref();
        let duracao = estado.map_or_else(
            || "...".to_owned(),
            |estado| format!("{} s", estado.config.duracao_clipe),
        );
        let qualidade = estado.map_or_else(|| "...".to_owned(), descrever_qualidade);
        let uso = estado.map_or_else(|| "...".to_owned(), descrever_uso);
        div()
            .flex()
            .px_5()
            .py_4()
            .gap_4()
            .child(indicador_de_resumo(icones::relogio, "Duração do clipe", duracao))
            .child(indicador_de_resumo(icones::monitor, "Qualidade", qualidade))
            .child(indicador_de_resumo(icones::folha, "Uso de recursos", uso).text_color(tema::verde()))
    }
}

fn indicador_de_resumo(simbolo: &'static str, rotulo: &'static str, valor: String) -> gpui::Div {
    div()
        .flex()
        .flex_1()
        .items_center()
        .gap_3()
        .child(icone(simbolo).size(px(22.0)).text_color(tema::roxo_claro()))
        .child(
            div()
                .flex()
                .flex_col()
                .child(div().text_xs().text_color(tema::texto_discreto()).child(rotulo))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(tema::texto())
                        .child(valor),
                ),
        )
}

fn descrever_qualidade(estado: &EstadoMotor) -> String {
    match &estado.sessao {
        Some(sessao) => format!("{}p · {} FPS", sessao.altura, estado.config.fps),
        None => format!("{} FPS", estado.config.fps),
    }
}

fn descrever_uso(estado: &EstadoMotor) -> String {
    let inteiro = estado.uso_cpu_decimos / 10;
    let decimo = estado.uso_cpu_decimos % 10;
    format!("{inteiro},{decimo}% CPU")
}
