use anyhow::{Context, Result, ensure};
use reqwest::blocking::{Client, RequestBuilder};
use serde_json::Value;
use std::io::Read;
use std::time::Duration;

pub(super) fn cliente(local: bool) -> Result<Client> {
    Ok(Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(4))
        .connect_timeout(Duration::from_secs(2))
        .danger_accept_invalid_certs(local)
        .no_proxy()
        .build()?)
}

pub(super) fn json(pedido: RequestBuilder) -> Result<Value> {
    let resposta = pedido.send().context("Serviço do jogo indisponível")?;
    ler_json(resposta)
}

pub(super) fn ler_json(resposta: reqwest::blocking::Response) -> Result<Value> {
    ensure!(resposta.status().is_success(), "O serviço do jogo recusou a consulta");
    let mut dados = Vec::new();
    resposta.take(4 * 1024 * 1024 + 1).read_to_end(&mut dados)?;
    ensure!(dados.len() <= 4 * 1024 * 1024, "Resposta do jogo excede o limite");
    Ok(serde_json::from_slice(&dados)?)
}

pub(super) fn texto(valor: &Value, campo: &str) -> String {
    valor.get(campo).and_then(Value::as_str).unwrap_or_default().to_owned()
}
