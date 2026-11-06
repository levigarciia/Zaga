fn main() {
    println!("cargo:rerun-if-changed=../../assets/marca/zaga.ico");
    let mut recursos = winresource::WindowsResource::new();
    recursos.set_icon("../../assets/marca/zaga.ico");
    recursos.compile().expect("falha ao embutir o icone do ZAGA");
}
