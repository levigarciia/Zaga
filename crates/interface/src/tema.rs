use gpui::{Background, Rgba, linear_color_stop, linear_gradient, rgb};

pub const FONTE_INTERFACE: &str = "Inter";
pub const FONTE_TITULOS: &str = "Space Grotesk";

pub fn fundo() -> Rgba {
    rgb(0x080808)
}

pub fn painel() -> Rgba {
    rgb(0x141414)
}

pub fn superficie() -> Rgba {
    rgb(0x1b1b1b)
}

pub fn superficie_realce() -> Rgba {
    rgb(0x262626)
}

pub fn borda() -> Rgba {
    rgb(0x2d2d2d)
}

pub fn texto() -> Rgba {
    rgb(0xf3f3f3)
}

pub fn texto_discreto() -> Rgba {
    rgb(0x999999)
}

pub fn roxo() -> Rgba {
    rgb(0x7436ff)
}

pub fn roxo_claro() -> Rgba {
    rgb(0x9a66ff)
}

pub fn verde() -> Rgba {
    rgb(0x35d07f)
}

pub fn vermelho() -> Rgba {
    rgb(0xf0544f)
}

pub fn erro_fundo() -> Rgba {
    rgb(0x34161d)
}

pub fn gradiente_primario() -> Background {
    linear_gradient(
        135.0,
        linear_color_stop(roxo_claro(), 0.0),
        linear_color_stop(roxo(), 1.0),
    )
}
