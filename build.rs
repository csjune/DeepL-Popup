use std::env;

fn main() {
    println!("cargo:rerun-if-changed=ui/app.slint");
    println!("cargo:rerun-if-changed=assets/app.ico");
    println!("cargo:rerun-if-changed=assets");
    println!("cargo:rerun-if-changed=build.rs");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("assets/app.ico")
            .set("FileDescription", "DeepL Popup")
            .set("ProductName", "DeepL Popup")
            .set("LegalCopyright", "Unofficial DeepL client");
        resource
            .compile()
            .expect("failed to compile Windows resources");
    }

    slint_build::compile("ui/app.slint").expect("failed to compile Slint UI");
}
