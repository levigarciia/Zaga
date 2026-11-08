use crate::aplicativo::Aplicativo;
use crate::components::base::{alternador, cartao, divisor, linha_de_ajuste};
use crate::recursos::icones;
use crate::tema;
use gpui::{AnyElement, Context, div, prelude::*};

impl Aplicativo {
    pub(crate) fn aba_autoclipagem(&self, contexto: &mut Context<Self>) -> AnyElement {
        let config = self.config_atual().autoclipagem;
        let alternar = contexto.listener(|app, _, _, cx| {
            app.alterar_config(|c| c.autoclipagem.ativa = !c.autoclipagem.ativa, cx);
        });
        let somente_jogos = contexto.listener(|app, _, _, cx| {
            app.alterar_config(|c| c.autoclipagem.somente_jogos = !c.autoclipagem.somente_jogos, cx);
        });
        let mut jogos = div()
            .flex()
            .flex_wrap()
            .gap_4()
            .text_xs()
            .text_color(tema::texto_discreto());
        let mut painel = cartao().flex().flex_col().flex_none().child(linha_de_ajuste(
            icones::tesoura,
            "Salvar destaques automaticamente",
            None,
            alternador("autoclipagem_ativa", config.ativa, alternar),
        ));
        for (indice, nome, habilitado) in [
            (0, "Counter Strike 2", config.cs2),
            (1, "League of Legends", config.lol),
            (2, "VALORANT", config.valorant),
            (3, "Fortnite", config.fortnite),
        ] {
            let alternar = contexto.listener(move |app, _, _, cx| {
                app.alterar_config(
                    |c| {
                        let auto = &mut c.autoclipagem;
                        match indice {
                            0 => auto.cs2 = !auto.cs2,
                            1 => auto.lol = !auto.lol,
                            2 => auto.valorant = !auto.valorant,
                            _ => auto.fortnite = !auto.fortnite,
                        }
                    },
                    cx,
                );
            });
            let id = match indice {
                0 => "auto_cs2",
                1 => "auto_lol",
                2 => "auto_valorant",
                _ => "auto_fortnite",
            };
            jogos = jogos.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(nome)
                    .child(alternador(id, habilitado, alternar)),
            );
        }
        painel = painel
            .child(div().px_4().pb_4().child(jogos))
            .child(divisor())
            .child(linha_de_ajuste(
                icones::tesoura,
                "Conquistas raras da Steam",
                Some("Acompanha novos desbloqueios com a Steam aberta. Conquistas antigas são ignoradas."),
                alternador(
                    "auto_steam",
                    config.steam,
                    contexto.listener(|app, _, _, cx| {
                        app.alterar_config(|c| c.autoclipagem.steam = !c.autoclipagem.steam, cx);
                    }),
                ),
            ))
            .child(divisor())
            .child(linha_de_ajuste(
                icones::tesoura,
                "Percentual global de desbloqueio",
                None,
                self.seletor_de(
                    "raridade_steam",
                    &[(5, "Abaixo de 5%"), (10, "Abaixo de 10%"), (25, "Abaixo de 25%")],
                    config.raridade_steam,
                    |c, n| c.autoclipagem.raridade_steam = n,
                    contexto,
                ),
            ))
            .child(divisor())
            .child(linha_de_ajuste(
                icones::monitor,
                "Gravar somente com um jogo aberto",
                None,
                alternador("somente_jogos", config.somente_jogos, somente_jogos),
            ))
            .child(divisor())
            .child(linha_de_ajuste(
                icones::relogio,
                "Contexto anterior",
                None,
                self.seletor_de(
                    "auto_antes",
                    &[(10, "10 segundos"), (20, "20 segundos"), (30, "30 segundos")],
                    config.anteriores,
                    |c, n| c.autoclipagem.anteriores = n,
                    contexto,
                ),
            ))
            .child(divisor())
            .child(linha_de_ajuste(
                icones::relogio,
                "Contexto posterior",
                None,
                self.seletor_de(
                    "auto_depois",
                    &[(5, "5 segundos"), (10, "10 segundos"), (15, "15 segundos")],
                    config.posteriores,
                    |c, n| c.autoclipagem.posteriores = n,
                    contexto,
                ),
            ))
            .child(divisor())
            .child(linha_de_ajuste(
                icones::tesoura,
                "Mínimo de eliminações",
                None,
                self.seletor_de(
                    "auto_minimo",
                    &[(1, "1K"), (2, "2K"), (3, "3K"), (4, "4K"), (5, "5K")],
                    config.minimo_eliminacoes,
                    |c, n| c.autoclipagem.minimo_eliminacoes = n,
                    contexto,
                ),
            ))
            .child(divisor())
            .child(linha_de_ajuste(
                icones::relogio,
                "Retenção após a partida",
                Some("Usada em VALORANT e Fortnite. O limite de espaço pode reduzir o tempo disponível."),
                self.seletor_de(
                    "auto_retencao",
                    &[(30, "30 minutos"), (45, "45 minutos"), (90, "90 minutos")],
                    config.retencao_minutos,
                    |c, n| c.autoclipagem.retencao_minutos = n,
                    contexto,
                ),
            ))
            .child(divisor())
            .child(linha_de_ajuste(
                icones::disco,
                "Limite do arquivo temporário",
                Some("Apagado ao encerrar a captura. Mudanças de gravação reiniciam o buffer."),
                self.seletor_de(
                    "auto_espaco",
                    &[(4, "4 GiB"), (8, "8 GiB"), (16, "16 GiB"), (32, "32 GiB")],
                    config.limite_gib,
                    |c, n| c.autoclipagem.limite_gib = n,
                    contexto,
                ),
            ));
        painel = painel.child(divisor()).child(linha_de_ajuste(
            icones::monitor,
            "Conta Epic",
            Some("Opcional se a identificação automática falhar. Cole o ID e pressione Enter para salvar."),
            div().w(gpui::px(300.0)).child(self.conta_epic.clone()),
        ));
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(painel)
            .child(
                div().text_sm().text_color(tema::texto_discreto()).child(
                    self.estado
                        .as_ref()
                        .map(|e| e.estado_autoclipagem.clone())
                        .unwrap_or_default(),
                ),
            )
            .into_any_element()
    }
}
