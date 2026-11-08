use gpui::{AssetSource, SharedString};
use std::borrow::Cow;

pub const LOGO: &str = "marca/logo.png";
pub const SIMBOLO: &str = "marca/simbolo.png";
pub const BANNER_INICIO: &str = "illustrations/inicio.png";

pub(crate) fn simbolo_do_jogo(nome: &str, tamanho: f32) -> gpui::AnyElement {
    use crate::components::base::icone;
    use gpui::{ObjectFit, img, prelude::*, px};
    match icone_do_jogo(nome) {
        Some((caminho, Some(cor))) => icone(caminho)
            .size(px(tamanho))
            .text_color(gpui::rgb(cor))
            .into_any_element(),
        Some((caminho, None)) => img(caminho)
            .size(px(tamanho))
            .rounded_sm()
            .object_fit(ObjectFit::Contain)
            .into_any_element(),
        None => icone(icones::monitor).size(px(tamanho)).into_any_element(),
    }
}

macro_rules! declarar_jogos {
    ($(($nome:literal, $arquivo:literal, $cor:expr)),* $(,)?) => {
        const TABELA_JOGOS: &[(&str, &[u8])] = &[
            $((concat!("games/", $arquivo), include_bytes!(concat!("../../../assets/games/", $arquivo)))),*
        ];

        /// Localiza a marca embutida para identificar um jogo nos filtros da biblioteca.
        pub fn icone_do_jogo(nome: &str) -> Option<(&'static str, Option<u32>)> {
            match nome {
                $($nome => Some((concat!("games/", $arquivo), $cor)),)*
                _ => None,
            }
        }
    };
}

declarar_jogos!(
    ("Fortnite", "fortnite.svg", Some(0xffffff)),
    ("VALORANT", "valorant.svg", Some(0xff4655)),
    ("Counter Strike 2", "counterstrike.svg", Some(0xf3f1f9)),
    ("League of Legends", "leagueoflegends.svg", Some(0xc8aa6e)),
    ("Dota 2", "dota2.svg", Some(0xe04a3a)),
    ("PUBG", "pubg.svg", Some(0xf2a900)),
    ("Apex Legends", "apexlegends.jpg", None),
    ("Rocket League", "rocketleague.jpg", None),
    ("Overwatch 2", "overwatch.jpg", None),
    ("Rainbow Six Siege", "rainbowsixsiege.jpg", None),
    ("GTA V", "grandtheftauto.jpg", None),
);

macro_rules! declarar_icones {
    ($($nome:ident),* $(,)?) => {
        #[allow(non_upper_case_globals)]
        pub mod icones {
            $(pub const $nome: &str = concat!("icones/", stringify!($nome), ".svg");)*
        }

        const TABELA_ICONES: &[(&str, &[u8])] = &[
            $((
                concat!("icones/", stringify!($nome), ".svg"),
                include_bytes!(concat!("../../../assets/icones/", stringify!($nome), ".svg")),
            ),)*
        ];
    };
}

declarar_icones!(
    alto_falante,
    busca,
    casa,
    chip,
    clipes,
    compartilhar,
    confirmar,
    cursor,
    disco,
    divisa,
    engrenagem,
    fechar,
    filtro,
    inicializar,
    lapis,
    lixeira,
    mais,
    microfone,
    maximizar,
    minimizar,
    monitor,
    mudo,
    onda,
    pasta,
    pausar,
    qualidade,
    relogio,
    teclado,
    tesoura,
    tocar,
    volume,
    voltar,
);

const TABELA_MARCA: &[(&str, &[u8])] = &[
    (
        BANNER_INICIO,
        include_bytes!("../../../assets/illustrations/inicio.png"),
    ),
    (LOGO, include_bytes!("../../../assets/marca/logo.png")),
    (SIMBOLO, include_bytes!("../../../assets/marca/simbolo.png")),
];

/// Logos e ícones embutidos no executável, para que a interface não dependa de arquivos soltos.
pub struct Recursos;

impl AssetSource for Recursos {
    fn load(&self, caminho: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        let encontrado = TABELA_ICONES
            .iter()
            .chain(TABELA_MARCA)
            .chain(TABELA_JOGOS)
            .find(|(nome, _)| *nome == caminho);
        Ok(encontrado.map(|(_, dados)| Cow::Borrowed(*dados)))
    }

    fn list(&self, _caminho: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn todo_jogo_suportado_tem_icone_embutido() {
        for jogo in zaga_nucleo::jogos::nomes_suportados() {
            let (caminho, _) = icone_do_jogo(jogo).expect("Jogo suportado sem ícone");
            let dados = Recursos.load(caminho).unwrap().expect("Ícone ausente no executável");
            assert!(!dados.is_empty(), "Ícone vazio para {jogo}");
        }
    }
}
