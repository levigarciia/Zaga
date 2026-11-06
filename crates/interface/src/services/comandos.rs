use std::io;
use zaga_nucleo::protocolo::{Pedido, Resposta};

/// Executa comandos fora da thread da interface para não bloquear o desenho.
pub fn enviar(pedido: &Pedido) -> io::Result<Resposta> {
    zaga_motor::enviar(pedido.clone()).map_err(|erro| io::Error::other(format!("{erro:#}")))
}
