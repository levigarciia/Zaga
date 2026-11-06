use crate::aplicativo::{Aplicativo, Pagina};
use crate::components::base::{botao_icone, botao_primario, botao_secundario, cartao};
use crate::components::campo_texto::{CampoTexto, EventoCampo};
use crate::recursos::icones;
use crate::services::biblioteca::{formatar_duracao, titulo_do_arquivo};
use crate::services::corte::salvar_trecho;
use crate::services::miniaturas::{self, QUADROS_NA_TIRA};
use crate::services::reprodutor::Reprodutor;
use crate::services::sistema::identificador_da_janela;
use crate::tema;
use gpui::{
    AnyElement, Bounds, Context, Entity, FontWeight, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ObjectFit, Pixels, Task, Window, canvas, div, img, prelude::*, px, relative,
};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use zaga_nucleo::UNIDADES_POR_SEGUNDO;

const INTERVALO_DO_PLAYER: Duration = Duration::from_millis(100);
const DISTANCIA_PARA_PEGAR_ALCA: f32 = 14.0;
const DURACAO_MINIMA_DO_CORTE: i64 = UNIDADES_POR_SEGUNDO;
const ALTURA_DA_TIRA: f32 = 56.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Arrasto {
    Cabeca,
    InicioDoCorte,
    FimDoCorte,
}

type Medida = Rc<Cell<Bounds<Pixels>>>;

/// Estado da tela de edição de um clipe.
pub struct Editor {
    caminho: PathBuf,
    reprodutor: Option<Rc<Reprodutor>>,
    campo_nome: Option<Entity<CampoTexto>>,
    duracao: i64,
    posicao: i64,
    inicio_corte: i64,
    fim_corte: i64,
    tocando: bool,
    mudo: bool,
    salvando: bool,
    arrasto: Option<Arrasto>,
    area_barra: Medida,
    area_tira: Medida,
    quadros_da_tira: Vec<PathBuf>,
    tira_esperada: Vec<PathBuf>,
    _relogio: Task<()>,
}

impl Aplicativo {
    pub(crate) fn abrir_editor(&mut self, caminho: &Path, janela: &mut Window, contexto: &mut Context<Self>) {
        let reprodutor = match abrir_reprodutor(caminho, janela) {
            Ok(reprodutor) => reprodutor,
            Err(erro) => {
                self.avisar(format!("Não foi possível abrir o clipe: {erro:#}"), contexto);
                return;
            }
        };
        let cache = self.pasta_cache().unwrap_or_default();
        let tira_esperada = miniaturas::chave_do_arquivo(caminho)
            .map(|chave| miniaturas::caminhos_da_tira(&cache, &chave))
            .unwrap_or_default();
        miniaturas::gerar_tira(caminho.to_path_buf(), cache);
        self.editor = Some(Editor {
            caminho: caminho.to_path_buf(),
            reprodutor: Some(reprodutor),
            campo_nome: None,
            duracao: 0,
            posicao: 0,
            inicio_corte: 0,
            fim_corte: 0,
            tocando: true,
            mudo: false,
            salvando: false,
            arrasto: None,
            area_barra: Rc::default(),
            area_tira: Rc::default(),
            quadros_da_tira: Vec::new(),
            tira_esperada,
            _relogio: iniciar_relogio_do_player(contexto),
        });
        self.pagina = Pagina::Editor;
        contexto.notify();
    }

    pub(crate) fn fechar_editor(&mut self) {
        self.editor = None;
    }

    pub(crate) fn pagina_editor(&mut self, contexto: &mut Context<Self>) -> AnyElement {
        let Some(editor) = &self.editor else {
            return div().into_any_element();
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .px_6()
            .pb_5()
            .on_mouse_move(contexto.listener(Self::mover_arrasto))
            .on_mouse_up(MouseButton::Left, contexto.listener(Self::soltar_arrasto))
            .on_mouse_up_out(MouseButton::Left, contexto.listener(Self::soltar_arrasto))
            .child(cabecalho_do_editor(editor, contexto))
            .child(area_de_video(editor))
            .child(controles_do_player(editor, contexto))
            .child(tira_de_corte(editor, contexto))
            .child(rodape_do_editor(editor, contexto))
            .into_any_element()
    }

    fn atualizar_player(&mut self, contexto: &mut Context<Self>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let Some(reprodutor) = editor.reprodutor.clone() else {
            return;
        };
        reprodutor.acompanhar_janela();
        let mut mudou = false;
        if editor.duracao == 0 {
            if let Some(duracao) = reprodutor.duracao() {
                editor.duracao = duracao;
                editor.fim_corte = duracao;
                mudou = true;
            }
        }
        if editor.arrasto.is_none() {
            if let Some(posicao) = reprodutor.posicao() {
                if posicao != editor.posicao {
                    editor.posicao = posicao;
                    mudou = true;
                }
            }
        }
        if editor.tocando && editor.fim_corte > 0 && editor.posicao >= editor.fim_corte {
            reprodutor.buscar(editor.inicio_corte);
            reprodutor.tocar();
            editor.posicao = editor.inicio_corte;
            mudou = true;
        }
        if editor.quadros_da_tira.len() < QUADROS_NA_TIRA {
            let prontos: Vec<PathBuf> = editor
                .tira_esperada
                .iter()
                .filter(|caminho| caminho.exists())
                .cloned()
                .collect();
            if prontos.len() != editor.quadros_da_tira.len() {
                editor.quadros_da_tira = prontos;
                mudou = true;
            }
        }
        if mudou {
            contexto.notify();
        }
    }

    fn alternar_reproducao(&mut self, contexto: &mut Context<Self>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        editor.tocando = !editor.tocando;
        if let Some(reprodutor) = &editor.reprodutor {
            if editor.tocando {
                reprodutor.tocar();
            } else {
                reprodutor.pausar();
            }
        }
        contexto.notify();
    }

    fn alternar_mudo(&mut self, contexto: &mut Context<Self>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        editor.mudo = !editor.mudo;
        if let Some(reprodutor) = &editor.reprodutor {
            reprodutor.definir_mudo(editor.mudo);
        }
        contexto.notify();
    }

    fn pressionar_barra(&mut self, evento: &MouseDownEvent, _janela: &mut Window, contexto: &mut Context<Self>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        editor.arrasto = Some(Arrasto::Cabeca);
        self.mover_para(f32::from(evento.position.x), contexto);
    }

    fn pressionar_tira(&mut self, evento: &MouseDownEvent, _janela: &mut Window, contexto: &mut Context<Self>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let limites = editor.area_tira.get();
        let x = f32::from(evento.position.x);
        let largura = f32::from(limites.size.width).max(1.0);
        let esquerda = f32::from(limites.origin.x);
        let x_inicio = esquerda + largura * fracao(editor.inicio_corte, editor.duracao);
        let x_fim = esquerda + largura * fracao(editor.fim_corte, editor.duracao);
        let (distancia_inicio, distancia_fim) = ((x - x_inicio).abs(), (x - x_fim).abs());
        editor.arrasto = Some(if distancia_inicio.min(distancia_fim) > DISTANCIA_PARA_PEGAR_ALCA {
            Arrasto::Cabeca
        } else if distancia_inicio <= distancia_fim {
            Arrasto::InicioDoCorte
        } else {
            Arrasto::FimDoCorte
        });
        self.mover_para(x, contexto);
    }

    fn mover_arrasto(&mut self, evento: &MouseMoveEvent, _janela: &mut Window, contexto: &mut Context<Self>) {
        if self.editor.as_ref().is_some_and(|editor| editor.arrasto.is_some()) {
            self.mover_para(f32::from(evento.position.x), contexto);
        }
    }

    fn soltar_arrasto(&mut self, _evento: &MouseUpEvent, _janela: &mut Window, contexto: &mut Context<Self>) {
        if let Some(editor) = &mut self.editor {
            if editor.arrasto.take().is_some() {
                contexto.notify();
            }
        }
    }

    fn mover_para(&mut self, x: f32, contexto: &mut Context<Self>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let Some(arrasto) = editor.arrasto else {
            return;
        };
        if editor.duracao == 0 {
            return;
        }
        let area = if arrasto == Arrasto::Cabeca && editor.area_barra.get().size.width > px(0.0) {
            editor.area_barra.get()
        } else {
            editor.area_tira.get()
        };
        let posicao = posicao_no_trilho(x, f32::from(area.origin.x), f32::from(area.size.width), editor.duracao);
        match arrasto {
            Arrasto::Cabeca => editor.posicao = posicao,
            Arrasto::InicioDoCorte => {
                editor.inicio_corte = posicao.min(editor.fim_corte - DURACAO_MINIMA_DO_CORTE).max(0);
                editor.posicao = editor.inicio_corte;
            }
            Arrasto::FimDoCorte => {
                editor.fim_corte = posicao
                    .max(editor.inicio_corte + DURACAO_MINIMA_DO_CORTE)
                    .min(editor.duracao);
                editor.posicao = editor.fim_corte;
            }
        }
        if let Some(reprodutor) = &editor.reprodutor {
            reprodutor.buscar(editor.posicao);
        }
        contexto.notify();
    }

    fn redefinir_corte(&mut self, contexto: &mut Context<Self>) {
        if let Some(editor) = &mut self.editor {
            editor.inicio_corte = 0;
            editor.fim_corte = editor.duracao;
            contexto.notify();
        }
    }

    fn salvar_corte(&mut self, contexto: &mut Context<Self>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        if editor.salvando {
            return;
        }
        editor.salvando = true;
        let (origem, inicio, fim) = (editor.caminho.clone(), editor.inicio_corte, editor.fim_corte);
        contexto.notify();
        let trabalho = contexto.background_executor().spawn(async move {
            let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            salvar_trecho(&origem, inicio, fim)
        });
        contexto
            .spawn(async move |aplicativo, contexto| {
                let resultado = trabalho.await;
                let _ = aplicativo.update(contexto, |aplicativo, contexto| {
                    if let Some(editor) = &mut aplicativo.editor {
                        editor.salvando = false;
                    }
                    match resultado {
                        Ok(_) => aplicativo.avisar("Novo clipe salvo na biblioteca", contexto),
                        Err(erro) => aplicativo.avisar(format!("Falha ao cortar: {erro:#}"), contexto),
                    }
                });
            })
            .detach();
    }

    fn excluir_do_editor(&mut self, contexto: &mut Context<Self>) {
        let Some(editor) = self.editor.take() else {
            return;
        };
        let caminho = editor.caminho.clone();
        drop(editor);
        self.pagina = Pagina::Clipes;
        self.excluir_clipes(&[caminho], contexto);
    }

    fn iniciar_renomeacao(&mut self, janela: &mut Window, contexto: &mut Context<Self>) {
        let Some(titulo) = self.editor.as_ref().map(|editor| titulo_visivel(&editor.caminho)) else {
            return;
        };
        let campo = contexto.new(|contexto| {
            let mut campo = CampoTexto::new("Nome do clipe", None, contexto);
            campo.definir_texto(&titulo);
            campo
        });
        contexto
            .subscribe_in(
                &campo,
                janela,
                |aplicativo, campo, evento, janela, contexto| match evento {
                    EventoCampo::Confirmado => {
                        let novo = campo.read(contexto).texto().to_owned();
                        aplicativo.concluir_renomeacao(&novo, janela, contexto);
                    }
                    EventoCampo::Cancelado => {
                        if let Some(editor) = &mut aplicativo.editor {
                            editor.campo_nome = None;
                        }
                        contexto.notify();
                    }
                },
            )
            .detach();
        campo.read(contexto).focar(janela);
        if let Some(editor) = &mut self.editor {
            editor.campo_nome = Some(campo);
        }
        contexto.notify();
    }

    fn concluir_renomeacao(&mut self, novo: &str, janela: &mut Window, contexto: &mut Context<Self>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        editor.campo_nome = None;
        let Some(destino) = caminho_renomeado(&editor.caminho, novo) else {
            self.avisar("Use um nome válido e que ainda não exista", contexto);
            return;
        };
        let posicao = editor.posicao;
        editor.reprodutor = None;
        if let Err(erro) = std::fs::rename(&editor.caminho, &destino) {
            let atual = editor.caminho.clone();
            editor.reprodutor = abrir_reprodutor(&atual, janela).ok();
            self.avisar(format!("Falha ao renomear: {erro}"), contexto);
            return;
        }
        editor.caminho = destino.clone();
        editor.reprodutor = abrir_reprodutor(&destino, janela).ok();
        if let Some(reprodutor) = &editor.reprodutor {
            reprodutor.buscar(posicao);
        }
        contexto.notify();
    }
}

fn abrir_reprodutor(caminho: &Path, janela: &Window) -> anyhow::Result<Rc<Reprodutor>> {
    Ok(Rc::new(Reprodutor::abrir(identificador_da_janela(janela)?, caminho)?))
}

fn iniciar_relogio_do_player(contexto: &mut Context<Aplicativo>) -> Task<()> {
    contexto.spawn(async move |aplicativo, contexto| {
        loop {
            contexto.background_executor().timer(INTERVALO_DO_PLAYER).await;
            let viva = aplicativo.update(contexto, |aplicativo, contexto| aplicativo.atualizar_player(contexto));
            if viva.is_err() {
                return;
            }
        }
    })
}

fn titulo_visivel(caminho: &Path) -> String {
    let nome = caminho.file_stem().and_then(|nome| nome.to_str()).unwrap_or("Clipe");
    titulo_do_arquivo(nome).to_owned()
}

fn caminho_renomeado(atual: &Path, novo: &str) -> Option<PathBuf> {
    let limpo = novo.trim().trim_end_matches('.').trim();
    if limpo.is_empty() || limpo.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|']) {
        return None;
    }
    let destino = atual.with_file_name(format!("{limpo}.mp4"));
    (destino != atual && !destino.exists()).then_some(destino)
}

fn fracao(valor: i64, total: i64) -> f32 {
    if total <= 0 {
        return 0.0;
    }
    (valor as f64 / total as f64).clamp(0.0, 1.0) as f32
}

fn posicao_no_trilho(x: f32, esquerda: f32, largura: f32, duracao: i64) -> i64 {
    let proporcao = ((x - esquerda) / largura.max(1.0)).clamp(0.0, 1.0);
    (f64::from(proporcao) * duracao as f64) as i64
}

fn medidor(area: Medida) -> impl IntoElement {
    canvas(move |limites, _, _| area.set(limites), |_, _, _, _| {})
        .absolute()
        .size_full()
}

fn cabecalho_do_editor(editor: &Editor, contexto: &mut Context<Aplicativo>) -> impl IntoElement + use<> {
    let voltar = contexto.listener(|aplicativo, _, _janela, contexto| aplicativo.ir_para(Pagina::Clipes, contexto));
    let excluir = contexto.listener(|aplicativo, _, _janela, contexto| aplicativo.excluir_do_editor(contexto));
    let renomear = contexto.listener(|aplicativo, _, janela, contexto| aplicativo.iniciar_renomeacao(janela, contexto));
    let caminho = editor.caminho.clone();
    let compartilhar = contexto.listener(move |aplicativo, _, _janela, contexto| {
        aplicativo.copiar_clipe(&caminho, contexto);
    });
    let titulo: AnyElement = match &editor.campo_nome {
        Some(campo) => div().w(px(320.0)).child(campo.clone()).into_any_element(),
        None => div()
            .flex()
            .items_center()
            .gap_1()
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(titulo_visivel(&editor.caminho)),
            )
            .child(botao_icone("renomear", icones::lapis, renomear))
            .into_any_element(),
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap_2()
        .child(botao_icone("voltar", icones::voltar, voltar))
        .child(titulo)
        .child(div().flex_1())
        .child(botao_icone("excluir_clipe", icones::lixeira, excluir))
        .child(botao_primario(
            "compartilhar",
            Some(icones::compartilhar),
            "Compartilhar",
            true,
            compartilhar,
        ))
}

fn area_de_video(editor: &Editor) -> impl IntoElement + use<> {
    let reprodutor = editor.reprodutor.clone();
    let posicionador = canvas(
        move |limites, janela, _| {
            let Some(reprodutor) = reprodutor else {
                return;
            };
            let escala = janela.scale_factor();
            reprodutor.posicionar(
                (f32::from(limites.origin.x) * escala).round() as i32,
                (f32::from(limites.origin.y) * escala).round() as i32,
                (f32::from(limites.size.width) * escala).round() as i32,
                (f32::from(limites.size.height) * escala).round() as i32,
            );
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full();
    div()
        .relative()
        .flex_1()
        .min_h(px(160.0))
        .rounded_lg()
        .bg(gpui::black())
        .child(posicionador)
}

fn controles_do_player(editor: &Editor, contexto: &mut Context<Aplicativo>) -> impl IntoElement + use<> {
    let simbolo_tocar = if editor.tocando { icones::pausar } else { icones::tocar };
    let simbolo_som = if editor.mudo { icones::mudo } else { icones::volume };
    let tocar = contexto.listener(|aplicativo, _, _janela, contexto| aplicativo.alternar_reproducao(contexto));
    let som = contexto.listener(|aplicativo, _, _janela, contexto| aplicativo.alternar_mudo(contexto));
    let progresso = fracao(editor.posicao, editor.duracao);
    let barra = div()
        .id("barra_de_progresso")
        .relative()
        .flex()
        .items_center()
        .flex_1()
        .h(px(20.0))
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, contexto.listener(Aplicativo::pressionar_barra))
        .child(medidor(editor.area_barra.clone()))
        .child(
            div()
                .relative()
                .w_full()
                .h(px(4.0))
                .rounded_full()
                .bg(tema::superficie_realce())
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .h_full()
                        .w(relative(progresso))
                        .rounded_full()
                        .bg(tema::roxo_claro()),
                ),
        );
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap_2()
        .child(botao_icone("tocar_pausar", simbolo_tocar, tocar))
        .child(botao_icone("som", simbolo_som, som))
        .child(
            div()
                .flex_none()
                .text_sm()
                .text_color(tema::texto_discreto())
                .child(format!(
                    "{} / {}",
                    formatar_duracao(editor.posicao),
                    formatar_duracao(editor.duracao)
                )),
        )
        .child(barra.ml_2())
}

fn tira_de_corte(editor: &Editor, contexto: &mut Context<Aplicativo>) -> impl IntoElement + use<> {
    let (inicio, fim) = (
        fracao(editor.inicio_corte, editor.duracao),
        fracao(editor.fim_corte, editor.duracao),
    );
    let quadros = editor.quadros_da_tira.iter().cloned().map(|caminho| {
        img(caminho)
            .flex_1()
            .min_w_0()
            .h_full()
            .object_fit(ObjectFit::Cover)
            .into_any_element()
    });
    let sombra = |esquerda: f32, largura: f32| {
        div()
            .absolute()
            .top_0()
            .h_full()
            .left(relative(esquerda))
            .w(relative(largura))
            .bg(gpui::rgba(0x000000b0))
    };
    let alca = |posicao: f32| {
        div()
            .absolute()
            .top_0()
            .h_full()
            .left(relative(posicao))
            .ml(px(-3.0))
            .w(px(6.0))
            .rounded_sm()
            .bg(tema::roxo_claro())
    };
    let moldura = div()
        .absolute()
        .top_0()
        .h_full()
        .left(relative(inicio))
        .w(relative((fim - inicio).max(0.0)))
        .border_t_2()
        .border_b_2()
        .border_color(tema::roxo_claro());
    cartao()
        .id("tira_de_corte")
        .relative()
        .flex()
        .flex_none()
        .w_full()
        .h(px(ALTURA_DA_TIRA))
        .overflow_hidden()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, contexto.listener(Aplicativo::pressionar_tira))
        .children(quadros)
        .child(sombra(0.0, inicio))
        .child(sombra(fim, (1.0 - fim).max(0.0)))
        .child(moldura)
        .child(alca(inicio))
        .child(alca(fim))
        .child(medidor(editor.area_tira.clone()))
}

fn rodape_do_editor(editor: &Editor, contexto: &mut Context<Aplicativo>) -> impl IntoElement + use<> {
    let redefinir = contexto.listener(|aplicativo, _, _janela, contexto| aplicativo.redefinir_corte(contexto));
    let salvar = contexto.listener(|aplicativo, _, _janela, contexto| aplicativo.salvar_corte(contexto));
    let trecho = editor.fim_corte - editor.inicio_corte;
    let resumo = format!(
        "{} até {} ({} no total). O corte começa no quadro chave mais próximo.",
        formatar_duracao(editor.inicio_corte),
        formatar_duracao(editor.fim_corte),
        formatar_duracao(trecho),
    );
    let pode_salvar = !editor.salvando && editor.duracao > 0 && trecho >= DURACAO_MINIMA_DO_CORTE;
    let rotulo = if editor.salvando {
        "Salvando..."
    } else {
        "Salvar como novo"
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap_2()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_xs()
                .text_color(tema::texto_discreto())
                .child(resumo),
        )
        .child(botao_secundario("redefinir_corte", None, "Redefinir", redefinir))
        .child(botao_primario(
            "salvar_corte",
            Some(icones::tesoura),
            rotulo,
            pode_salvar,
            salvar,
        ))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn trilho_limita_a_posicao_ao_clipe() {
        assert_eq!(posicao_no_trilho(50.0, 0.0, 100.0, 1000), 500);
        assert_eq!(posicao_no_trilho(-20.0, 0.0, 100.0, 1000), 0);
        assert_eq!(posicao_no_trilho(500.0, 0.0, 100.0, 1000), 1000);
    }

    #[test]
    fn fracao_sem_duracao_e_zero() {
        assert_eq!(fracao(10, 0), 0.0);
        assert_eq!(fracao(5, 10), 0.5);
    }

    #[test]
    fn nome_invalido_ou_igual_e_recusado() {
        let atual = std::env::temp_dir().join("zaga_teste_nome_que_nao_existe.mp4");
        assert!(caminho_renomeado(&atual, "  ").is_none());
        assert!(caminho_renomeado(&atual, "a/b").is_none());
        assert!(caminho_renomeado(&atual, "zaga_teste_nome_que_nao_existe").is_none());
        let novo = caminho_renomeado(&atual, "Zaga insana 1v4").unwrap();
        assert!(novo.ends_with("Zaga insana 1v4.mp4"));
    }
}
