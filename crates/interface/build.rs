fn main() {
    println!("cargo:rerun-if-changed=../../assets/marca/zaga.ico");
    let mut recursos = winresource::WindowsResource::new();
    recursos.set_icon("../../assets/marca/zaga.ico");
    recursos.set("FileDescription", "ZAGA");
    recursos.set("ProductName", "ZAGA");
    recursos.set("InternalName", "ZAGA");
    recursos.set("OriginalFilename", "ZAGA.exe");
    recursos.compile().expect("falha ao embutir o icone do ZAGA");
}
