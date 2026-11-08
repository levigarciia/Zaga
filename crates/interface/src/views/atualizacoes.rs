use crate::aplicativo::Aplicativo;
use crate::atualizador::{self, Situacao};
use crate::components::base::botao_primario;
use crate::tema;
use gpui::{Context, div, prelude::*};
use zaga_nucleo::protocolo::Pedido;

impl Aplicativo {
    pub(crate) fn controle_atualizacao(&self, contexto: &mut Context<Self>) -> impl IntoElement + use<> {
        let reiniciar = matches!(self.situacao_atualizador, Situacao::Pronta(_));
        let ocupado = matches!(self.situacao_atualizador, Situacao::Verificando | Situacao::Baixando(_));
        let mensagem = match &self.situacao_atualizador {
            Situacao::Aguardando => "Atualizações oficiais pelo GitHub.".to_owned(),
            Situacao::Verificando => "Verificando atualizações...".into(),
            Situacao::Atualizado => "Você está na versão mais recente.".into(),
            Situacao::Baixando(versao) => format!("Baixando a versão {versao}..."),
            Situacao::Pronta(versao) if self.config_atual().atualizar_automaticamente => {
                format!("Versão {versao} pronta para instalar ao sair.")
            }
            Situacao::Pronta(versao) => format!("Versão {versao} pronta para instalar."),
            Situacao::Falha(mensagem) => mensagem.clone(),
        };
        let clicar = contexto.listener(move |aplicativo, _, _, contexto| {
            if reiniciar {
                if atualizador::solicitar_reinicio() {
                    aplicativo.fechar_editor();
                    aplicativo.enviar_pedido(Pedido::Encerrar, contexto);
                }
                return;
            }
            atualizador::verificar();
        });
        div()
            .flex()
            .items_center()
            .gap_3()
            .px_4()
            .py_3()
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_sm().child(concat!("ZAGA ", env!("CARGO_PKG_VERSION"))))
                    .child(div().text_xs().text_color(tema::texto_discreto()).child(mensagem)),
            )
            .child(botao_primario(
                "atualizar_aplicativo",
                None,
                if reiniciar {
                    "Atualizar e reiniciar"
                } else {
                    "Verificar"
                },
                !ocupado && !atualizador::isolado(),
                clicar,
            ))
    }
}
