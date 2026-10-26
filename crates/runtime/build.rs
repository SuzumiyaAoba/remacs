#[path = "../../build.rs"]
mod build_identity;

fn main() {
    build_identity::main();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let source = build_identity::source_root().join("crates/runtime/src/lisp/process/macos.c");
        println!("cargo:rerun-if-changed={}", source.display());
        cc::Build::new()
            .file(source)
            .compile("remacs_macos_process");
        println!("cargo:rustc-link-lib=proc");
    }
}
