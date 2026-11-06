//! Extração dos parâmetros de sequência a partir de um quadro chave em formato Annex B.

use crate::config::Codec;

const CODIGO_INICIO: [u8; 4] = [0, 0, 0, 1];

/// Copia as unidades NAL de configuração do codec, necessárias para descrever a trilha no MP4.
///
/// Use quando o encoder não expõe o cabeçalho no tipo de mídia. Retorna `None` se o quadro não tiver nenhuma delas.
pub fn extrair_cabecalho(codec: Codec, quadro_annex_b: &[u8]) -> Option<Vec<u8>> {
    let mut cabecalho = Vec::new();
    for unidade in unidades_nal(quadro_annex_b) {
        if unidade
            .first()
            .is_some_and(|primeiro| eh_parametro_de_sequencia(codec, *primeiro))
        {
            cabecalho.extend_from_slice(&CODIGO_INICIO);
            cabecalho.extend_from_slice(unidade);
        }
    }
    (!cabecalho.is_empty()).then_some(cabecalho)
}

fn eh_parametro_de_sequencia(codec: Codec, primeiro_byte: u8) -> bool {
    match codec {
        Codec::H264 => matches!(primeiro_byte & 0x1F, 7 | 8),
        Codec::Hevc => matches!((primeiro_byte >> 1) & 0x3F, 32..=34),
    }
}

fn unidades_nal(dados: &[u8]) -> Vec<&[u8]> {
    let inicios = posicoes_apos_codigo_inicio(dados);
    inicios
        .iter()
        .enumerate()
        .map(|(indice, inicio)| {
            let proximo = inicios.get(indice + 1).map_or(dados.len(), |proximo| proximo - 3);
            remover_zeros_finais(&dados[*inicio..proximo])
        })
        .collect()
}

fn posicoes_apos_codigo_inicio(dados: &[u8]) -> Vec<usize> {
    dados
        .windows(3)
        .enumerate()
        .filter(|(_, janela)| *janela == [0, 0, 1])
        .map(|(posicao, _)| posicao + 3)
        .collect()
}

fn remover_zeros_finais(unidade: &[u8]) -> &[u8] {
    let fim = unidade
        .iter()
        .rposition(|byte| *byte != 0)
        .map_or(0, |posicao| posicao + 1);
    &unidade[..fim]
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn extrai_sps_e_pps_de_h264() {
        let quadro = [0, 0, 0, 1, 0x67, 1, 2, 0, 0, 0, 1, 0x68, 3, 0, 0, 1, 0x65, 9, 9];
        let cabecalho = extrair_cabecalho(Codec::H264, &quadro).unwrap();
        assert_eq!(cabecalho, vec![0, 0, 0, 1, 0x67, 1, 2, 0, 0, 0, 1, 0x68, 3]);
    }

    #[test]
    fn extrai_vps_sps_e_pps_de_hevc() {
        let quadro = [0, 0, 1, 0x40, 1, 0, 0, 1, 0x42, 2, 0, 0, 1, 0x44, 3, 0, 0, 1, 0x26, 9];
        let cabecalho = extrair_cabecalho(Codec::Hevc, &quadro).unwrap();
        assert_eq!(
            cabecalho,
            vec![0, 0, 0, 1, 0x40, 1, 0, 0, 0, 1, 0x42, 2, 0, 0, 0, 1, 0x44, 3]
        );
    }

    #[test]
    fn quadro_sem_parametros_nao_tem_cabecalho() {
        assert_eq!(extrair_cabecalho(Codec::H264, &[0, 0, 1, 0x41, 7]), None);
        assert_eq!(extrair_cabecalho(Codec::H264, &[]), None);
    }
}
