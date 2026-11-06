use gpui::{Background, Rgba, linear_color_stop, linear_gradient, rgb};

pub fn fundo() -> Rgba {
    rgb(0x09080f)
}

pub fn painel() -> Rgba {
    rgb(0x100e18)
}

pub fn superficie() -> Rgba {
    rgb(0x161322)
}

pub fn superficie_realce() -> Rgba {
    rgb(0x201c30)
}

pub fn borda() -> Rgba {
    rgb(0x262138)
}

pub fn texto() -> Rgba {
    rgb(0xf3f1f9)
}

pub fn texto_discreto() -> Rgba {
    rgb(0x9591a8)
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

pub fn gradiente_ativo() -> Background {
    linear_gradient(
        90.0,
        linear_color_stop(rgb(0x5a2bc4), 0.0),
        linear_color_stop(rgb(0x3a2070), 1.0),
    )
}

pub fn gradiente_heroi() -> Background {
    linear_gradient(
        120.0,
        linear_color_stop(rgb(0x100e18), 0.35),
        linear_color_stop(rgb(0x241446), 1.0),
    )
}
