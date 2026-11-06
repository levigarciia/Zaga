use gpui::{AssetSource, SharedString};
use std::borrow::Cow;

pub const LOGO: &str = "marca/logo.png";
pub const SIMBOLO: &str = "marca/simbolo.png";

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
    folha,
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
            .find(|(nome, _)| *nome == caminho);
        Ok(encontrado.map(|(_, dados)| Cow::Borrowed(*dados)))
    }

    fn list(&self, _caminho: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}
