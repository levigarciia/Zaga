//! Combinações de teclas globais configuráveis pelo usuário.

use serde::{Deserialize, Serialize};

/// Combinação de teclas, com a tecla principal no nome em minúsculas usado pela interface, como `f9` ou `k`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Atalho {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub tecla: String,
}

impl Atalho {
    /// Atalhos de fábrica, na mesma ordem de [`crate::config::DURACOES_CLIPE`].
    pub fn padroes() -> [Atalho; 3] {
        ["f9", "f10", "f11"].map(|tecla| Atalho {
            ctrl: false,
            alt: true,
            shift: false,
            tecla: tecla.to_owned(),
        })
    }

    /// Código de tecla virtual do Windows, ou `None` se a tecla não puder ser usada como atalho.
    pub fn codigo_virtual(&self) -> Option<u32> {
        let tecla = self.tecla.as_str();
        if let [caractere] = tecla.as_bytes()
            && caractere.is_ascii_alphanumeric()
        {
            return Some(u32::from(caractere.to_ascii_uppercase()));
        }
        if let Some(numero) = numero_de_tecla_de_funcao(tecla) {
            return Some(0x6F + numero);
        }
        match tecla {
            "space" => Some(0x20),
            "pageup" => Some(0x21),
            "pagedown" => Some(0x22),
            "end" => Some(0x23),
            "home" => Some(0x24),
            "left" => Some(0x25),
            "up" => Some(0x26),
            "right" => Some(0x27),
            "down" => Some(0x28),
            "insert" => Some(0x2D),
            "delete" => Some(0x2E),
            _ => None,
        }
    }

    /// Informa se a combinação serve como atalho global sem atrapalhar a digitação em outros programas.
    pub fn valido(&self) -> bool {
        self.codigo_virtual().is_some() && (self.ctrl || self.alt || numero_de_tecla_de_funcao(&self.tecla).is_some())
    }

    /// Texto da combinação para exibir ao usuário.
    pub fn descricao(&self) -> String {
        let mut partes = Vec::new();
        if self.ctrl {
            partes.push("Ctrl".to_owned());
        }
        if self.alt {
            partes.push("Alt".to_owned());
        }
        if self.shift {
            partes.push("Shift".to_owned());
        }
        partes.push(nome_da_tecla(&self.tecla));
        partes.join(" + ")
    }
}

fn numero_de_tecla_de_funcao(tecla: &str) -> Option<u32> {
    let numero: u32 = tecla.strip_prefix('f')?.parse().ok()?;
    (1..=24).contains(&numero).then_some(numero)
}

fn nome_da_tecla(tecla: &str) -> String {
    match tecla {
        "space" => "Espaço".to_owned(),
        "pageup" => "PgUp".to_owned(),
        "pagedown" => "PgDn".to_owned(),
        "end" => "End".to_owned(),
        "home" => "Home".to_owned(),
        "left" => "Esquerda".to_owned(),
        "up" => "Cima".to_owned(),
        "right" => "Direita".to_owned(),
        "down" => "Baixo".to_owned(),
        "insert" => "Insert".to_owned(),
        "delete" => "Delete".to_owned(),
        outra => outra.to_uppercase(),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn atalho(ctrl: bool, alt: bool, shift: bool, tecla: &str) -> Atalho {
        Atalho {
            ctrl,
            alt,
            shift,
            tecla: tecla.to_owned(),
        }
    }

    #[test]
    fn traduz_teclas_para_codigos_do_windows() {
        assert_eq!(atalho(false, true, false, "f9").codigo_virtual(), Some(0x78));
        assert_eq!(atalho(false, true, false, "f24").codigo_virtual(), Some(0x87));
        assert_eq!(atalho(true, false, false, "k").codigo_virtual(), Some(0x4B));
        assert_eq!(atalho(true, false, false, "7").codigo_virtual(), Some(0x37));
        assert_eq!(atalho(true, false, false, "space").codigo_virtual(), Some(0x20));
        assert_eq!(atalho(true, false, false, "f25").codigo_virtual(), None);
        assert_eq!(atalho(true, false, false, "escape").codigo_virtual(), None);
    }

    #[test]
    fn letra_sozinha_nao_vale_porque_atrapalharia_a_digitacao() {
        assert!(!atalho(false, false, false, "k").valido());
        assert!(!atalho(false, false, true, "k").valido());
        assert!(atalho(true, false, false, "k").valido());
        assert!(atalho(false, false, false, "f7").valido());
    }

    #[test]
    fn descricao_lista_modificadores_em_ordem() {
        assert_eq!(atalho(true, true, true, "k").descricao(), "Ctrl + Alt + Shift + K");
        assert_eq!(atalho(false, true, false, "f10").descricao(), "Alt + F10");
    }
}
