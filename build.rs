use std::env;

fn main() {
    println!("cargo:rerun-if-changed=ui/app.slint");
    println!("cargo:rerun-if-changed=assets/app.ico");
    println!("cargo:rerun-if-changed=assets");
    println!("cargo:rerun-if-changed=lang");
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

    // English is the source language; the catalogs in lang/ are built into the
    // executable. Strings are matched without a context, so the same text has
    // one translation wherever it appears.
    let config = slint_build::CompilerConfiguration::new()
        .with_bundled_translations("lang")
        .with_default_translation_context(slint_build::DefaultTranslationContext::None);
    slint_build::compile_with_config("ui/app.slint", config).expect("failed to compile Slint UI");
}
