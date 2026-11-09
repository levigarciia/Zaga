use crate::components::campo_texto::{CampoTexto, EventoCampo};
use crate::recursos::icones;
use crate::services::biblioteca::{Clipe, OrdemClipes, listar_clipes};
use crate::services::{comandos as cliente_motor, miniaturas};
use crate::tema;
use crate::views::editor::Editor;
use gpui::{
    AnyElement, Context, Entity, FocusHandle, FontWeight, MouseButton, Render, SharedString, Task, Window, deferred,
    div, prelude::*, px,
};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;
use zaga_nucleo::config::Config;
use zaga_nucleo::protocolo::{EstadoMotor, Pedido, Resposta};

const INTERVALO_DE_ATUALIZACAO: Duration = Duration::from_secs(1);
const DURACAO_DO_AVISO: Duration = Duration::from_secs(4);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pagina {
    Inicio,
    Clipes,
    Configuracoes,
    Editor,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AbaConfig {
    Captura,
    Autoclipagem,
    Desempenho,
    Geral,
    Atalhos,
}

/// Estado raiz da interface: reflete o motor, guarda a navegação e encaminha os comandos do usuário.
pub struct Aplicativo {
    pub(crate) situacao_atualizador: crate::atualizador::Situacao,
    pub(crate) estado: Option<EstadoMotor>,
    pub(crate) clipes: Vec<Clipe>,
    pub(crate) filtro_jogo_inicio: Option<String>,
    pub(crate) pagina: Pagina,
    pub(crate) aba: AbaConfig,
    pub(crate) menu_aberto: Option<SharedString>,
    pub(crate) busca: Entity<CampoTexto>,
    pub(crate) conta_epic: Entity<CampoTexto>,
    pub(crate) ordem: OrdemClipes,
    pub(crate) selecionando: bool,
    pub(crate) selecionados: BTreeSet<PathBuf>,
    pub(crate) editor: Option<Editor>,
    pub(crate) gravando_atalho: Option<usize>,
    pub(crate) foco_atalho: FocusHandle,
    aviso: Option<(SharedString, u64)>,
    proximo_aviso: u64,
    pedidos: Option<Task<()>>,
    _atualizacao: Task<()>,
}

impl Aplicativo {
    pub fn new(contexto: &mut Context<Self>) -> Self {
        let busca = contexto.new(|contexto| CampoTexto::new("Buscar clipes...", Some(icones::busca), contexto));
        contexto.observe(&busca, |_, _, contexto| contexto.notify()).detach();
        let conta_epic = contexto.new(|cx| CampoTexto::new("ID da conta Epic, opcional", None, cx));
        contexto
            .subscribe(&conta_epic, |app, campo, evento, cx| {
                if matches!(evento, EventoCampo::Confirmado) {
                    let conta = campo.read(cx).texto().trim().to_owned();
                    app.alterar_config(|c| c.autoclipagem.conta_epic = conta, cx);
                }
            })
            .detach();
        Self {
            situacao_atualizador: crate::atualizador::situacao(),
            estado: None,
            clipes: Vec::new(),
            filtro_jogo_inicio: None,
            pagina: Pagina::Inicio,
            aba: AbaConfig::Captura,
            menu_aberto: None,
            busca,
            conta_epic,
            ordem: OrdemClipes::Recentes,
            selecionando: false,
            selecionados: BTreeSet::new(),
            editor: None,
            gravando_atalho: None,
            foco_atalho: contexto.focus_handle(),
            aviso: None,
            proximo_aviso: 0,
            pedidos: None,
            _atualizacao: iniciar_atualizacao(contexto),
        }
    }

    pub(crate) fn pasta_cache(&self) -> Option<PathBuf> {
        self.estado.as_ref().map(|estado| caminho_cache(&estado.pasta_dados))
    }

    pub(crate) fn ir_para(&mut self, pagina: Pagina, contexto: &mut Context<Self>) {
        self.cancelar_gravacao_de_atalho(contexto);
        self.fechar_editor();
        self.pagina = pagina;
        if pagina != Pagina::Clipes {
            self.selecionando = false;
            self.selecionados.clear();
        }
        self.menu_aberto = None;
        contexto.notify();
    }

    pub(crate) fn alternar_menu(&mut self, id: impl Into<SharedString>, contexto: &mut Context<Self>) {
        let id = id.into();
        self.menu_aberto = if self.menu_aberto.as_ref() == Some(&id) {
            None
        } else {
            Some(id)
        };
        contexto.notify();
    }

    pub(crate) fn menu_esta_aberto(&self, id: &str) -> bool {
        self.menu_aberto.as_ref().is_some_and(|aberto| aberto == id)
    }

    pub(crate) fn enviar_pedido(&mut self, pedido: Pedido, contexto: &mut Context<Self>) {
        let anterior = self.pedidos.take();
        self.pedidos = Some(contexto.spawn(async move |aplicativo, contexto| {
            if let Some(anterior) = anterior {
                anterior.await;
            }
            let resposta = contexto
                .background_executor()
                .spawn(async move { cliente_motor::enviar(&pedido) })
                .await;
            let _ = aplicativo.update(contexto, |aplicativo, contexto| {
                if let Some(falha) = descrever_falha(resposta) {
                    aplicativo.avisar(falha, contexto);
                }
            });
        }));
    }

    pub(crate) fn alterar_config(&mut self, alterar: impl FnOnce(&mut Config), contexto: &mut Context<Self>) {
        let Some(estado) = &mut self.estado else {
            return;
        };
        alterar(&mut estado.config);
        let pedido = Pedido::AplicarConfig(estado.config.clone());
        self.menu_aberto = None;
        contexto.notify();
        self.enviar_pedido(pedido, contexto);
    }

    pub(crate) fn avisar(&mut self, mensagem: impl Into<SharedString>, contexto: &mut Context<Self>) {
        self.proximo_aviso += 1;
        let identificador = self.proximo_aviso;
        self.aviso = Some((mensagem.into(), identificador));
        contexto.notify();
        contexto
            .spawn(async move |aplicativo, contexto| {
                contexto.background_executor().timer(DURACAO_DO_AVISO).await;
                let _ = aplicativo.update(contexto, |aplicativo, contexto| {
                    if aplicativo
                        .aviso
                        .as_ref()
                        .is_some_and(|(_, atual)| *atual == identificador)
                    {
                        aplicativo.aviso = None;
                        contexto.notify();
                    }
                });
            })
            .detach();
    }

    fn aplicar_fotografia(&mut self, estado: Option<EstadoMotor>, clipes: Vec<Clipe>, contexto: &mut Context<Self>) {
        let situacao = crate::atualizador::situacao();
        if self.situacao_atualizador != situacao {
            self.situacao_atualizador = situacao;
            contexto.notify();
        }
        let Some(estado_recebido) = estado else {
            if self.estado.take().is_some() {
                contexto.notify();
            }
            return;
        };
        if self.estado.is_none() {
            self.conta_epic.update(contexto, |campo, _| {
                campo.definir_texto(&estado_recebido.config.autoclipagem.conta_epic);
            });
        }
        let pendentes = clipes
            .iter()
            .filter(|clipe| clipe.miniatura.is_none() || clipe.duracao.is_none())
            .map(|clipe| clipe.caminho.clone())
            .collect();
        miniaturas::gerar_miniaturas_pendentes(pendentes, caminho_cache(&estado_recebido.pasta_dados));
        if self.estado.as_ref() != Some(&estado_recebido) || self.clipes != clipes {
            self.estado = Some(estado_recebido);
            self.clipes = clipes;
            contexto.notify();
        }
    }

    fn pagina_atual(&mut self, janela: &mut Window, contexto: &mut Context<Self>) -> AnyElement {
        match self.pagina {
            Pagina::Inicio => self.pagina_inicio(janela, contexto),
            Pagina::Clipes => self.pagina_clipes(janela, contexto),
            Pagina::Configuracoes => self.pagina_configuracoes(contexto),
            Pagina::Editor => self.pagina_editor(contexto),
        }
    }

    fn fundo_de_menu(&self, contexto: &mut Context<Self>) -> Option<AnyElement> {
        self.menu_aberto.as_ref()?;
        let fechar = contexto.listener(|aplicativo, _, _janela, contexto| {
            aplicativo.menu_aberto = None;
            contexto.notify();
        });
        let cobertura = div()
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(MouseButton::Left, fechar);
        Some(deferred(cobertura).with_priority(1).into_any_element())
    }

    fn aviso_flutuante(&self) -> Option<impl IntoElement + use<>> {
        let (mensagem, _) = self.aviso.as_ref()?;
        Some(
            div()
                .absolute()
                .bottom_3()
                .left_3()
                .max_w(px(420.0))
                .px_4()
                .py_3()
                .rounded_lg()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .bg(tema::superficie_realce())
                .border_1()
                .border_color(tema::roxo())
                .shadow_lg()
                .child(mensagem.clone()),
        )
    }
}

impl Render for Aplicativo {
    fn render(&mut self, janela: &mut Window, contexto: &mut Context<Self>) -> impl IntoElement {
        let pagina = self.pagina_atual(janela, contexto);
        let fundo_menu = self.fundo_de_menu(contexto);
        div()
            .relative()
            .size_full()
            .flex()
            .bg(tema::fundo())
            .text_color(tema::texto())
            .font_family(tema::FONTE_INTERFACE)
            .child(self.barra_lateral(contexto))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(self.barra_superior(contexto))
                    .child(div().flex_1().min_h_0().pt_4().child(pagina)),
            )
            .children(fundo_menu)
            .children(self.aviso_flutuante())
    }
}

fn caminho_cache(pasta_dados: &str) -> PathBuf {
    PathBuf::from(pasta_dados).join("miniaturas")
}

fn iniciar_atualizacao(contexto: &mut Context<Aplicativo>) -> Task<()> {
    contexto.spawn(async move |aplicativo, contexto| {
        loop {
            let fotografia = contexto.background_executor().spawn(async { obter_fotografia() }).await;
            let viva = aplicativo.update(contexto, |aplicativo, contexto| {
                aplicativo.aplicar_fotografia(fotografia.0, fotografia.1, contexto);
            });
            if viva.is_err() {
                return;
            }
            contexto.background_executor().timer(INTERVALO_DE_ATUALIZACAO).await;
        }
    })
}

fn obter_fotografia() -> (Option<EstadoMotor>, Vec<Clipe>) {
    let Ok(Resposta::Estado(estado)) = cliente_motor::enviar(&Pedido::ObterEstado) else {
        return (None, Vec::new());
    };
    let cache = caminho_cache(&estado.pasta_dados);
    let clipes = listar_clipes(
        &PathBuf::from(&estado.pasta_clipes),
        &cache,
        &PathBuf::from(&estado.pasta_dados),
    );
    (Some(estado), clipes)
}

fn descrever_falha(resposta: std::io::Result<Resposta>) -> Option<String> {
    match resposta {
        Ok(Resposta::Falha(mensagem)) => Some(mensagem),
        Err(erro) => Some(format!("Motor indisponível: {erro}")),
        Ok(_) => None,
    }
}
