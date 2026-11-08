use crate::aplicativo::{AbaConfig, Aplicativo};
use crate::components::base::{alternador, botao_secundario, cartao, divisor, linha_de_ajuste};
use crate::components::seletor::{OpcaoMenu, seletor};
use crate::recursos::icones;
use crate::tema;
use gpui::{AnyElement, Context, ElementId, FontWeight, KeyDownEvent, Window, div, prelude::*, px};
use zaga_nucleo::atalho::Atalho;
use zaga_nucleo::config::{Codec, Config, DURACOES_CLIPE, Qualidade, Resolucao};
use zaga_nucleo::protocolo::Pedido;

const ABAS: [(AbaConfig, &str); 5] = [
    (AbaConfig::Captura, "Captura"),
    (AbaConfig::Autoclipagem, "Destaques"),
    (AbaConfig::Desempenho, "Desempenho"),
    (AbaConfig::Geral, "Geral"),
    (AbaConfig::Atalhos, "Atalhos"),
];
const RESOLUCOES: [(Resolucao, &str); 4] = [
    (Resolucao::Nativa, "Nativa do monitor"),
    (Resolucao::P1440, "1440p (Quad HD)"),
    (Resolucao::P1080, "1080p (Full HD)"),
    (Resolucao::P720, "720p (HD)"),
];
const TAXAS_FPS: [(u32, &str); 3] = [(30, "30 FPS"), (60, "60 FPS"), (120, "120 FPS")];
const QUALIDADES: [(Qualidade, &str); 3] = [
    (Qualidade::Economica, "Econômica"),
    (Qualidade::Alta, "Alta"),
    (Qualidade::Maxima, "Máxima"),
];
const CODECS: [(Codec, &str); 2] = [(Codec::H264, "H.264 (compatível)"), (Codec::Hevc, "HEVC (menor)")];
const RETENCOES: [(u32, &str); 3] = [(30, "30 segundos"), (60, "60 segundos"), (120, "120 segundos")];
const ROTULOS_ATALHOS: [&str; 3] = [
    "Salvar últimos 30 segundos",
    "Salvar últimos 60 segundos",
    "Salvar últimos 120 segundos",
];

impl Aplicativo {
    pub(crate) fn pagina_configuracoes(&mut self, contexto: &mut Context<Self>) -> AnyElement {
        let conteudo = match self.aba {
            AbaConfig::Captura => self.aba_captura(contexto),
            AbaConfig::Autoclipagem => self.aba_autoclipagem(contexto),
            AbaConfig::Desempenho => self.aba_desempenho(contexto),
            AbaConfig::Geral => self.aba_geral(contexto),
            AbaConfig::Atalhos => self.aba_atalhos(contexto),
        };
        div()
            .id("pagina_configuracoes")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_4()
            .px_6()
            .pb_6()
            .child(
                div()
                    .text_size(px(24.0))
                    .font_family(tema::FONTE_TITULOS)
                    .font_weight(FontWeight::BOLD)
                    .child("Configurações"),
            )
            .child(self.barra_de_abas(contexto))
            .child(conteudo)
            .into_any_element()
    }

    fn barra_de_abas(&self, contexto: &mut Context<Self>) -> impl IntoElement {
        let abas = ABAS.into_iter().map(|(aba, rotulo)| {
            let base = div()
                .id(ElementId::Name(rotulo.into()))
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .h(px(40.0))
                .rounded_lg()
                .text_sm()
                .cursor_pointer()
                .on_click(contexto.listener(move |aplicativo, _, _janela, contexto| {
                    aplicativo.cancelar_gravacao_de_atalho(contexto);
                    aplicativo.aba = aba;
                    aplicativo.menu_aberto = None;
                    contexto.notify();
                }))
                .child(rotulo);
            if self.aba == aba {
                base.bg(tema::gradiente_primario()).font_weight(FontWeight::MEDIUM)
            } else {
                base.text_color(tema::texto_discreto())
                    .hover(|estilo| estilo.text_color(tema::texto()))
            }
        });
        cartao().flex().flex_none().gap_1().p_1().children(abas)
    }

    pub(crate) fn config_atual(&self) -> Config {
        self.estado
            .as_ref()
            .map(|estado| estado.config.clone())
            .unwrap_or_default()
    }

    fn aba_captura(&self, contexto: &mut Context<Self>) -> AnyElement {
        let config = self.config_atual();
        let duracoes: Vec<(u32, String)> = DURACOES_CLIPE
            .iter()
            .map(|segundos| (*segundos, format!("{segundos} segundos")))
            .collect();
        let duracoes: Vec<(u32, &str)> = duracoes
            .iter()
            .map(|(valor, rotulo)| (*valor, rotulo.as_str()))
            .collect();
        let alternar = contexto.listener(|aplicativo, _, _janela, contexto| {
            aplicativo.alterar_config(|config| config.captura_ativa = !config.captura_ativa, contexto);
        });
        cartao()
            .flex()
            .flex_col()
            .flex_none()
            .child(linha_de_ajuste(
                icones::inicializar,
                "Captura automática",
                Some("Mantém os últimos segundos da gameplay prontos para salvar."),
                alternador("config_captura", config.captura_ativa, alternar),
            ))
            .child(divisor())
            .child(linha_de_ajuste(
                icones::relogio,
                "Duração do clipe",
                Some("Usada pelo botão Salvar em Início."),
                self.seletor_de(
                    "duracao",
                    &duracoes,
                    config.duracao_clipe,
                    |config, valor| config.duracao_clipe = valor,
                    contexto,
                ),
            ))
            .child(divisor())
            .child(linha_de_ajuste(
                icones::monitor,
                "Resolução",
                None,
                self.seletor_de(
                    "resolucao",
                    &RESOLUCOES,
                    config.resolucao,
                    |config, valor| config.resolucao = valor,
                    contexto,
                ),
            ))
            .child(divisor())
            .child(linha_de_ajuste(
                icones::onda,
                "Taxa de FPS",
                None,
                self.seletor_de(
                    "fps",
                    &TAXAS_FPS,
                    config.fps,
                    |config, valor| config.fps = valor,
                    contexto,
                ),
            ))
            .into_any_element()
    }

    fn aba_desempenho(&self, contexto: &mut Context<Self>) -> AnyElement {
        let config = self.config_atual();
        cartao()
            .flex()
            .flex_col()
            .flex_none()
            .child(linha_de_ajuste(
                icones::qualidade,
                "Qualidade do vídeo",
                Some("Mais qualidade gera arquivos maiores."),
                self.seletor_de(
                    "qualidade",
                    &QUALIDADES,
                    config.qualidade,
                    |config, valor| config.qualidade = valor,
                    contexto,
                ),
            ))
            .child(divisor())
            .child(linha_de_ajuste(
                icones::chip,
                "Codec",
                self.resumo_do_encoder(),
                self.seletor_de(
                    "codec",
                    &CODECS,
                    config.codec,
                    |config, valor| config.codec = valor,
                    contexto,
                ),
            ))
            .child(divisor())
            .child(linha_de_ajuste(
                icones::disco,
                "Buffer em disco",
                Some("Tempo máximo que pode ser salvo. Menos tempo reserva menos espaço."),
                self.seletor_de(
                    "retencao",
                    &RETENCOES,
                    config.retencao_segundos,
                    |config, valor| config.retencao_segundos = valor,
                    contexto,
                ),
            ))
            .into_any_element()
    }

    fn resumo_do_encoder(&self) -> Option<&'static str> {
        self.estado
            .as_ref()
            .and_then(|estado| estado.sessao.as_ref())
            .map(|_| "Codificado na GPU, sem custo para o jogo.")
    }

    fn aba_geral(&self, contexto: &mut Context<Self>) -> AnyElement {
        let config = self.config_atual();
        let audio = contexto.listener(|aplicativo, _, _janela, contexto| {
            aplicativo.alterar_config(|config| config.capturar_audio = !config.capturar_audio, contexto);
        });
        let microfone = contexto.listener(|aplicativo, _, _janela, contexto| {
            aplicativo.alterar_config(
                |config| config.capturar_microfone = !config.capturar_microfone,
                contexto,
            );
        });
        let cursor = contexto.listener(|aplicativo, _, _janela, contexto| {
            aplicativo.alterar_config(|config| config.capturar_cursor = !config.capturar_cursor, contexto);
        });
        let inicio = contexto.listener(|aplicativo, _, _janela, contexto| {
            aplicativo.alterar_config(
                |config| config.iniciar_com_windows = !config.iniciar_com_windows,
                contexto,
            );
        });
        let atualizacoes = contexto.listener(|aplicativo, _, _janela, contexto| {
            aplicativo.alterar_config(
                |config| config.atualizar_automaticamente = !config.atualizar_automaticamente,
                contexto,
            );
        });
        let pasta = self
            .estado
            .as_ref()
            .map(|estado| estado.pasta_clipes.clone())
            .unwrap_or_default();
        let abrir_pasta = {
            let pasta = pasta.clone();
            move |_: &gpui::ClickEvent, _: &mut gpui::Window, contexto: &mut gpui::App| {
                let _ = std::fs::create_dir_all(&pasta);
                contexto.open_url(&pasta);
            }
        };
        cartao()
            .flex()
            .flex_col()
            .flex_none()
            .child(linha_de_ajuste(
                icones::alto_falante,
                "Áudio do sistema",
                None,
                alternador("config_audio", config.capturar_audio, audio),
            ))
            .child(divisor())
            .child(linha_de_ajuste(
                icones::microfone,
                "Microfone",
                Some("Grava sua voz junto com o jogo, usando o microfone padrão do Windows."),
                alternador("config_microfone", config.capturar_microfone, microfone),
            ))
            .child(divisor())
            .child(linha_de_ajuste(
                icones::cursor,
                "Mostrar o cursor",
                None,
                alternador("config_cursor", config.capturar_cursor, cursor),
            ))
            .child(divisor())
            .child(linha_de_ajuste(
                icones::inicializar,
                "Iniciar com o Windows",
                None,
                alternador("config_inicio", config.iniciar_com_windows, inicio),
            ))
            .child(divisor())
            .child(linha_de_ajuste(
                icones::inicializar,
                "Atualizações automáticas",
                Some("Baixa em segundo plano e aplica ao sair do ZAGA."),
                alternador("config_atualizacoes", config.atualizar_automaticamente, atualizacoes),
            ))
            .child(divisor())
            .child(self.controle_atualizacao(contexto))
            .child(divisor())
            .child(
                linha_de_ajuste(
                    icones::pasta,
                    "Pasta dos clipes",
                    None,
                    botao_secundario("abrir_pasta", None, "Abrir", abrir_pasta),
                )
                .child(div().hidden().child(pasta)),
            )
            .into_any_element()
    }

    pub(crate) fn seletor_de<T: Copy + PartialEq + 'static>(
        &self,
        id: &'static str,
        opcoes: &[(T, &str)],
        atual: T,
        aplicar: fn(&mut Config, T),
        contexto: &mut Context<Self>,
    ) -> impl IntoElement {
        let rotulo_atual = opcoes
            .iter()
            .find(|(valor, _)| *valor == atual)
            .map_or_else(String::new, |(_, rotulo)| (*rotulo).to_owned());
        let alternar = contexto.listener(move |aplicativo, _, _janela, contexto| {
            aplicativo.alternar_menu(id, contexto);
        });
        let entidade = contexto.entity();
        let itens: Vec<(T, String)> = opcoes
            .iter()
            .map(|(valor, rotulo)| (*valor, (*rotulo).to_owned()))
            .collect();
        seletor(id, rotulo_atual, self.menu_esta_aberto(id), alternar, move || {
            itens
                .into_iter()
                .map(|(valor, rotulo)| {
                    let entidade = entidade.clone();
                    OpcaoMenu::new(rotulo, valor == atual, move |_janela, contexto| {
                        entidade.update(contexto, |aplicativo, contexto| {
                            aplicativo.alterar_config(|config| aplicar(config, valor), contexto);
                        });
                    })
                })
                .collect()
        })
    }
}

impl Aplicativo {
    fn aba_atalhos(&self, contexto: &mut Context<Self>) -> AnyElement {
        let config = self.config_atual();
        let indisponiveis = self
            .estado
            .as_ref()
            .map(|estado| estado.atalhos_indisponiveis.clone())
            .unwrap_or_default();
        let mut cartao_de_atalhos = cartao().flex().flex_col();
        for (indice, rotulo) in ROTULOS_ATALHOS.into_iter().enumerate() {
            if indice > 0 {
                cartao_de_atalhos = cartao_de_atalhos.child(divisor());
            }
            let aviso = indisponiveis
                .contains(&indice)
                .then_some("Em uso por outro programa. Escolha outra combinação.");
            let texto = if self.gravando_atalho == Some(indice) {
                "Pressione as teclas...".to_owned()
            } else {
                config.atalhos[indice].descricao()
            };
            cartao_de_atalhos = cartao_de_atalhos.child(linha_de_ajuste(
                icones::teclado,
                rotulo,
                aviso,
                self.botao_de_atalho(indice, texto, contexto),
            ));
        }
        let restaurar = contexto.listener(|aplicativo, _, _janela, contexto| {
            aplicativo.cancelar_gravacao_de_atalho(contexto);
            aplicativo.alterar_config(|config| config.atalhos = Atalho::padroes(), contexto);
        });
        div()
            .track_focus(&self.foco_atalho)
            .on_key_down(contexto.listener(Self::receber_tecla_de_atalho))
            .flex()
            .flex_col()
            .flex_none()
            .items_end()
            .gap_3()
            .child(cartao_de_atalhos.w_full())
            .child(botao_secundario(
                "restaurar_atalhos",
                None,
                "Restaurar padrão",
                restaurar,
            ))
            .into_any_element()
    }

    fn botao_de_atalho(&self, indice: usize, texto: String, contexto: &mut Context<Self>) -> impl IntoElement {
        let gravando = self.gravando_atalho == Some(indice);
        div()
            .id(ElementId::NamedInteger("atalho".into(), indice as u64))
            .flex_none()
            .px_3()
            .py_1()
            .rounded_md()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .bg(tema::superficie_realce())
            .border_1()
            .border_color(if gravando { tema::roxo_claro() } else { tema::borda() })
            .cursor_pointer()
            .hover(|estilo| estilo.border_color(tema::roxo()))
            .child(texto)
            .on_click(contexto.listener(move |aplicativo, _, janela, contexto| {
                aplicativo.iniciar_gravacao_de_atalho(indice, janela, contexto);
            }))
    }

    pub(crate) fn iniciar_gravacao_de_atalho(
        &mut self,
        indice: usize,
        janela: &mut Window,
        contexto: &mut Context<Self>,
    ) {
        if self.gravando_atalho.is_none() {
            self.enviar_pedido(Pedido::SuspenderAtalhos(true), contexto);
        }
        self.gravando_atalho = Some(indice);
        self.foco_atalho.focus(janela);
        contexto.notify();
    }

    pub(crate) fn cancelar_gravacao_de_atalho(&mut self, contexto: &mut Context<Self>) {
        if self.gravando_atalho.take().is_some() {
            self.enviar_pedido(Pedido::SuspenderAtalhos(false), contexto);
            contexto.notify();
        }
    }

    fn receber_tecla_de_atalho(&mut self, evento: &KeyDownEvent, _janela: &mut Window, contexto: &mut Context<Self>) {
        let Some(indice) = self.gravando_atalho else {
            return;
        };
        let tecla = &evento.keystroke;
        if tecla.key == "escape" {
            self.cancelar_gravacao_de_atalho(contexto);
            return;
        }
        let novo = Atalho {
            ctrl: tecla.modifiers.control,
            alt: tecla.modifiers.alt,
            shift: tecla.modifiers.shift,
            tecla: tecla.key.to_lowercase(),
        };
        if !novo.valido() {
            self.avisar("Use Ctrl ou Alt junto com a tecla, ou uma tecla de F1 a F24", contexto);
            return;
        }
        let config = self.config_atual();
        let repetido = config
            .atalhos
            .iter()
            .enumerate()
            .any(|(outro, atalho)| outro != indice && *atalho == novo);
        if repetido {
            self.avisar("Essa combinação já está em uso por outro atalho", contexto);
            return;
        }
        self.alterar_config(|config| config.atalhos[indice] = novo, contexto);
        self.cancelar_gravacao_de_atalho(contexto);
    }
}
