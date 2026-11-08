//! Buffer circular de pacotes já comprimidos.
//!
//! Os bytes ficam em um [`Armazenamento`] de capacidade fixa e apenas o índice fica em memória,
//! o que mantém o uso de RAM independente da duração retida.

use std::collections::VecDeque;
use std::{fmt, io};

/// Identifica a trilha de origem de um pacote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fluxo {
    Video,
    Audio,
}

/// Descreve um pacote retido, sem carregar seus bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MetaPacote {
    pub fluxo: Fluxo,
    pub pts: i64,
    pub chave: bool,
    deslocamento_virtual: u64,
    pub tamanho: u32,
}

/// Meio de capacidade fixa onde os bytes dos pacotes são gravados.
///
/// Implemente para trocar o destino dos dados, como um arquivo pré alocado em produção ou memória nos testes.
pub trait Armazenamento {
    fn capacidade(&self) -> u64;
    fn escrever(&mut self, deslocamento: u64, dados: &[u8]) -> io::Result<()>;
    fn ler(&mut self, deslocamento: u64, destino: &mut [u8]) -> io::Result<()>;
}

/// Falhas possíveis ao gravar ou recuperar pacotes do anel.
#[derive(Debug)]
pub enum ErroAnel {
    PacoteMaiorQueCapacidade { tamanho: usize, capacidade: u64 },
    PacoteSobrescrito,
    TamanhoDestinoIncorreto,
    Io(io::Error),
}

impl fmt::Display for ErroAnel {
    fn fmt(&self, formatador: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PacoteMaiorQueCapacidade { tamanho, capacidade } => {
                write!(
                    formatador,
                    "pacote de {tamanho} bytes excede a capacidade de {capacidade} bytes"
                )
            }
            Self::PacoteSobrescrito => write!(formatador, "pacote sobrescrito antes de ser lido"),
            Self::TamanhoDestinoIncorreto => write!(formatador, "destino com tamanho diferente do pacote"),
            Self::Io(erro) => write!(formatador, "falha de E/S no anel: {erro}"),
        }
    }
}

impl std::error::Error for ErroAnel {}

impl From<io::Error> for ErroAnel {
    fn from(erro: io::Error) -> Self {
        Self::Io(erro)
    }
}

/// Trecho selecionado do anel, pronto para virar um arquivo de vídeo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelecaoClipe {
    pub inicio: i64,
    pub fim: i64,
    pub pacotes: Vec<MetaPacote>,
}

/// Mantém os pacotes mais recentes dentro de um limite de tempo e de bytes.
///
/// Crie um anel por sessão de captura e chame [`AnelPacotes::inserir`] a partir das trilhas de vídeo e áudio.
pub struct AnelPacotes<A: Armazenamento> {
    armazenamento: A,
    metas: VecDeque<MetaPacote>,
    retencao: i64,
    proximo_deslocamento: u64,
}

impl<A: Armazenamento> AnelPacotes<A> {
    /// Troca o destino sem perder os pacotes que cabem nos novos limites.
    /// Use somente quando não houver seleções em leitura. Falhas preservam o anel original.
    pub fn migrar(&mut self, armazenamento: A, retencao: i64) -> Result<(), ErroAnel> {
        let mut novo = Self::new(armazenamento, retencao);
        let mut dados = Vec::new();
        let limite = self.metas.iter().map(|meta| meta.pts).max().unwrap_or(0) - retencao;
        for meta in self.metas.iter().filter(|meta| meta.pts >= limite) {
            dados.resize(meta.tamanho as usize, 0);
            self.armazenamento
                .ler(meta.deslocamento_virtual % self.armazenamento.capacidade(), &mut dados)?;
            novo.inserir(meta.fluxo, meta.pts, meta.chave, &dados)?;
        }
        *self = novo;
        Ok(())
    }

    pub fn new(armazenamento: A, retencao: i64) -> Self {
        assert!(armazenamento.capacidade() > 0, "armazenamento sem capacidade");
        assert!(retencao > 0, "retenção precisa ser positiva");
        Self {
            armazenamento,
            metas: VecDeque::new(),
            retencao,
            proximo_deslocamento: 0,
        }
    }

    /// Grava um pacote e descarta os mais antigos que excederem o tempo de retenção ou a capacidade.
    pub fn inserir(&mut self, fluxo: Fluxo, pts: i64, chave: bool, dados: &[u8]) -> Result<(), ErroAnel> {
        let capacidade = self.armazenamento.capacidade();
        let tamanho = dados.len() as u64;
        if tamanho > capacidade {
            return Err(ErroAnel::PacoteMaiorQueCapacidade {
                tamanho: dados.len(),
                capacidade,
            });
        }

        let deslocamento_virtual = self.posicao_contigua(tamanho, capacidade);
        self.descartar_vencidos(pts);
        self.liberar_espaco(deslocamento_virtual + tamanho, capacidade);

        self.armazenamento.escrever(deslocamento_virtual % capacidade, dados)?;
        self.metas.push_back(MetaPacote {
            fluxo,
            pts,
            chave,
            deslocamento_virtual,
            tamanho: dados.len() as u32,
        });
        self.proximo_deslocamento = deslocamento_virtual + tamanho;
        Ok(())
    }

    /// Copia os bytes de um pacote selecionado, falhando se ele já tiver sido sobrescrito.
    pub fn ler(&mut self, meta: &MetaPacote, destino: &mut [u8]) -> Result<(), ErroAnel> {
        if destino.len() != meta.tamanho as usize {
            return Err(ErroAnel::TamanhoDestinoIncorreto);
        }
        let mais_antigo = self.metas.front().map(|frente| frente.deslocamento_virtual);
        if mais_antigo.is_none_or(|antigo| meta.deslocamento_virtual < antigo) {
            return Err(ErroAnel::PacoteSobrescrito);
        }
        let fisico = meta.deslocamento_virtual % self.armazenamento.capacidade();
        self.armazenamento.ler(fisico, destino)?;
        Ok(())
    }

    /// Escolhe os pacotes dos últimos `duracao` instantes, começando sempre em um quadro chave.
    ///
    /// O início recua até o quadro chave anterior ao alvo para que o clipe nunca fique menor que o pedido,
    /// desde que haja dados suficientes retidos.
    pub fn selecionar_clipe(&self, duracao: i64) -> Option<SelecaoClipe> {
        assert!(duracao > 0, "duração do clipe precisa ser positiva");
        let videos = || self.metas.iter().filter(|meta| meta.fluxo == Fluxo::Video);
        let fim = videos().map(|meta| meta.pts).max()?;
        let alvo = fim - duracao;
        let chaves = || videos().filter(|meta| meta.chave).map(|meta| meta.pts);
        let inicio = chaves().filter(|pts| *pts <= alvo).max().or_else(|| chaves().min())?;

        let mut pacotes: Vec<MetaPacote> = self
            .metas
            .iter()
            .filter(|meta| meta.pts >= inicio && meta.pts <= fim)
            .copied()
            .collect();
        pacotes.sort_by_key(|meta| meta.pts);
        Some(SelecaoClipe { inicio, fim, pacotes })
    }

    /// Seleciona um intervalo histórico completo. Recusa trechos que já foram sobrescritos ou ainda não chegaram.
    pub fn selecionar_intervalo(&self, alvo_inicio: i64, alvo_fim: i64) -> Option<SelecaoClipe> {
        if alvo_inicio >= alvo_fim {
            return None;
        }
        let videos = || self.metas.iter().filter(|meta| meta.fluxo == Fluxo::Video);
        if videos().map(|meta| meta.pts).max()? < alvo_fim {
            return None;
        }
        let inicio = videos()
            .filter(|meta| meta.chave && meta.pts <= alvo_inicio)
            .map(|meta| meta.pts)
            .max()?;
        let mut pacotes: Vec<_> = self
            .metas
            .iter()
            .filter(|meta| meta.pts >= inicio && meta.pts <= alvo_fim)
            .copied()
            .collect();
        pacotes.sort_by_key(|meta| meta.pts);
        Some(SelecaoClipe {
            inicio,
            fim: alvo_fim,
            pacotes,
        })
    }

    /// Informa quanto tempo de vídeo está disponível para um clipe agora.
    pub fn duracao_disponivel(&self) -> i64 {
        let mut chaves = self
            .metas
            .iter()
            .filter(|meta| meta.fluxo == Fluxo::Video && meta.chave);
        let Some(primeira) = chaves.next() else {
            return 0;
        };
        let ultimo = self.metas.iter().rev().find(|meta| meta.fluxo == Fluxo::Video);
        ultimo.map_or(0, |meta| meta.pts - primeira.pts)
    }

    fn posicao_contigua(&self, tamanho: u64, capacidade: u64) -> u64 {
        let posicao = self.proximo_deslocamento;
        let restante = capacidade - posicao % capacidade;
        if tamanho > restante {
            posicao + restante
        } else {
            posicao
        }
    }

    fn descartar_vencidos(&mut self, pts_atual: i64) {
        let limite = pts_atual - self.retencao;
        while self.metas.front().is_some_and(|meta| meta.pts < limite) {
            self.metas.pop_front();
        }
    }

    fn liberar_espaco(&mut self, fim_virtual: u64, capacidade: u64) {
        while self
            .metas
            .front()
            .is_some_and(|meta| meta.deslocamento_virtual + capacidade < fim_virtual)
        {
            self.metas.pop_front();
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::UNIDADES_POR_SEGUNDO as SEGUNDO;

    struct Memoria(Vec<u8>);

    impl Armazenamento for Memoria {
        fn capacidade(&self) -> u64 {
            self.0.len() as u64
        }

        fn escrever(&mut self, deslocamento: u64, dados: &[u8]) -> io::Result<()> {
            let inicio = deslocamento as usize;
            self.0[inicio..inicio + dados.len()].copy_from_slice(dados);
            Ok(())
        }

        fn ler(&mut self, deslocamento: u64, destino: &mut [u8]) -> io::Result<()> {
            let inicio = deslocamento as usize;
            destino.copy_from_slice(&self.0[inicio..inicio + destino.len()]);
            Ok(())
        }
    }

    fn anel(capacidade: usize, retencao_segundos: i64) -> AnelPacotes<Memoria> {
        AnelPacotes::new(Memoria(vec![0; capacidade]), retencao_segundos * SEGUNDO)
    }

    fn ler_tudo(anel: &mut AnelPacotes<Memoria>, selecao: &SelecaoClipe) -> Vec<Vec<u8>> {
        selecao
            .pacotes
            .iter()
            .map(|meta| {
                let mut destino = vec![0; meta.tamanho as usize];
                anel.ler(meta, &mut destino).unwrap();
                destino
            })
            .collect()
    }

    #[test]
    fn migracao_amplia_buffer_sem_perder_video_e_audio() {
        let mut anel = anel(12, 10);
        for segundo in 0..8 {
            anel.inserir(Fluxo::Video, segundo * SEGUNDO, true, &[segundo as u8; 2])
                .unwrap();
        }
        anel.inserir(Fluxo::Audio, 7 * SEGUNDO, false, &[90]).unwrap();
        let antes = anel.selecionar_clipe(10 * SEGUNDO).unwrap();
        let bytes = ler_tudo(&mut anel, &antes);
        anel.migrar(Memoria(vec![0; 64]), 60 * SEGUNDO).unwrap();
        let depois = anel.selecionar_clipe(10 * SEGUNDO).unwrap();
        assert_eq!((antes.inicio, antes.fim), (depois.inicio, depois.fim));
        assert_eq!(ler_tudo(&mut anel, &depois), bytes);
        anel.inserir(Fluxo::Video, 30 * SEGUNDO, true, &[30]).unwrap();
        assert!(anel.selecionar_intervalo(antes.inicio, 30 * SEGUNDO).is_some());
    }

    #[test]
    fn migracao_reduz_retencao_e_preserva_bytes_recentes() {
        let mut anel = anel(128, 60);
        for segundo in 0..=30 {
            anel.inserir(Fluxo::Video, segundo * SEGUNDO, true, &[segundo as u8])
                .unwrap();
        }
        anel.migrar(Memoria(vec![0; 16]), 5 * SEGUNDO).unwrap();
        let selecao = anel.selecionar_clipe(60 * SEGUNDO).unwrap();
        assert_eq!((selecao.inicio, selecao.fim), (25 * SEGUNDO, 30 * SEGUNDO));
        assert_eq!(
            ler_tudo(&mut anel, &selecao),
            (25..=30).map(|n| vec![n]).collect::<Vec<_>>()
        );
    }

    #[test]
    fn falha_na_migracao_preserva_anel_original() {
        let mut anel = anel(16, 10);
        anel.inserir(Fluxo::Video, 0, true, &[1, 2, 3, 4]).unwrap();
        let selecao = anel.selecionar_clipe(SEGUNDO).unwrap();
        assert!(anel.migrar(Memoria(vec![0; 2]), 60 * SEGUNDO).is_err());
        assert_eq!(ler_tudo(&mut anel, &selecao), vec![vec![1, 2, 3, 4]]);
    }

    #[test]
    fn descarta_pacotes_alem_da_retencao() {
        let mut anel = anel(1024, 10);
        for segundo in 0..=20 {
            anel.inserir(Fluxo::Video, segundo * SEGUNDO, true, &[segundo as u8])
                .unwrap();
        }
        let selecao = anel.selecionar_clipe(100 * SEGUNDO).unwrap();
        assert_eq!(selecao.inicio, 10 * SEGUNDO);
        assert_eq!(selecao.fim, 20 * SEGUNDO);
    }

    #[test]
    fn descarta_pacotes_quando_falta_capacidade() {
        let mut anel = anel(10, 1000);
        for indice in 0..10u8 {
            anel.inserir(Fluxo::Video, indice as i64 * SEGUNDO, true, &[indice; 3])
                .unwrap();
        }
        let selecao = anel.selecionar_clipe(1000 * SEGUNDO).unwrap();
        let bytes_retidos: u32 = selecao.pacotes.iter().map(|meta| meta.tamanho).sum();
        assert!(bytes_retidos <= 10);
        assert_eq!(ler_tudo(&mut anel, &selecao).last().unwrap(), &vec![9u8; 3]);
    }

    #[test]
    fn pacote_nao_e_dividido_na_virada_do_arquivo() {
        let mut anel = anel(10, 1000);
        anel.inserir(Fluxo::Video, 0, true, &[1; 7]).unwrap();
        anel.inserir(Fluxo::Video, SEGUNDO, true, &[2; 5]).unwrap();
        let selecao = anel.selecionar_clipe(1000 * SEGUNDO).unwrap();
        assert_eq!(ler_tudo(&mut anel, &selecao), vec![vec![2u8; 5]]);
    }

    #[test]
    fn clipe_comeca_no_quadro_chave_anterior_ao_alvo() {
        let mut anel = anel(1024, 100);
        for segundo in 0..=30 {
            anel.inserir(Fluxo::Video, segundo * SEGUNDO, segundo % 4 == 0, &[0])
                .unwrap();
        }
        let selecao = anel.selecionar_clipe(10 * SEGUNDO).unwrap();
        assert_eq!(selecao.inicio, 20 * SEGUNDO);
        assert!(selecao.pacotes[0].chave);
    }

    #[test]
    fn audio_fica_restrito_ao_intervalo_do_video() {
        let mut anel = anel(1024, 100);
        anel.inserir(Fluxo::Audio, SEGUNDO, false, &[0]).unwrap();
        anel.inserir(Fluxo::Video, 2 * SEGUNDO, true, &[0]).unwrap();
        anel.inserir(Fluxo::Audio, 3 * SEGUNDO, false, &[0]).unwrap();
        anel.inserir(Fluxo::Video, 4 * SEGUNDO, false, &[0]).unwrap();
        anel.inserir(Fluxo::Audio, 5 * SEGUNDO, false, &[0]).unwrap();
        let selecao = anel.selecionar_clipe(100 * SEGUNDO).unwrap();
        let pts: Vec<i64> = selecao.pacotes.iter().map(|meta| meta.pts / SEGUNDO).collect();
        assert_eq!(pts, vec![2, 3, 4]);
    }

    #[test]
    fn leitura_de_pacote_sobrescrito_falha() {
        let mut anel = anel(4, 1000);
        anel.inserir(Fluxo::Video, 0, true, &[1; 4]).unwrap();
        let selecao = anel.selecionar_clipe(SEGUNDO).unwrap();
        anel.inserir(Fluxo::Video, SEGUNDO, true, &[2; 4]).unwrap();
        let mut destino = [0; 4];
        assert!(matches!(
            anel.ler(&selecao.pacotes[0], &mut destino),
            Err(ErroAnel::PacoteSobrescrito)
        ));
    }

    #[test]
    fn pacote_maior_que_capacidade_e_recusado() {
        let mut anel = anel(4, 1000);
        assert!(matches!(
            anel.inserir(Fluxo::Video, 0, true, &[0; 5]),
            Err(ErroAnel::PacoteMaiorQueCapacidade { .. })
        ));
    }

    #[test]
    fn sem_quadro_chave_nao_ha_clipe() {
        let mut anel = anel(1024, 100);
        anel.inserir(Fluxo::Video, 0, false, &[0]).unwrap();
        anel.inserir(Fluxo::Audio, 0, false, &[0]).unwrap();
        assert!(anel.selecionar_clipe(SEGUNDO).is_none());
        assert_eq!(anel.duracao_disponivel(), 0);
    }

    #[test]
    fn intervalo_historico_nao_vira_os_ultimos_segundos() {
        let mut anel = anel(1024, 600);
        for segundo in 0..=300 {
            anel.inserir(Fluxo::Video, segundo * SEGUNDO, segundo % 2 == 0, &[1])
                .unwrap();
            anel.inserir(Fluxo::Audio, segundo * SEGUNDO, false, &[2]).unwrap();
        }
        let s = anel.selecionar_intervalo(31 * SEGUNDO, 45 * SEGUNDO).unwrap();
        assert_eq!((s.inicio, s.fim), (30 * SEGUNDO, 45 * SEGUNDO));
        assert!(s.pacotes.iter().all(|p| p.pts <= 45 * SEGUNDO));
        assert!(s.pacotes.iter().any(|p| p.fluxo == Fluxo::Audio));
        assert!(anel.selecionar_intervalo(-SEGUNDO, 10 * SEGUNDO).is_none());
        assert!(anel.selecionar_intervalo(299 * SEGUNDO, 301 * SEGUNDO).is_none());
    }

    #[test]
    fn intervalo_sobrescrito_nao_exporta_trecho_recente_com_titulo_antigo() {
        let mut anel = anel(1024, 30);
        for segundo in 0..=300 {
            anel.inserir(Fluxo::Video, segundo * SEGUNDO, segundo % 2 == 0, &[1])
                .unwrap();
        }
        assert!(anel.selecionar_intervalo(30 * SEGUNDO, 45 * SEGUNDO).is_none());
    }
}
