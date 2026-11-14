use remacs_runtime::lisp::Interp;
use std::path::PathBuf;

#[test]
fn vc_arch_library_loads_completely() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../lisp/vc-arch.el")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    std::thread::Builder::new()
        .name("vc-arch-load".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let mut interp = Interp::new();
            let form = format!("(progn (load-file {:?}) (featurep 'vc-arch))", source);
            let result = interp
                .eval_str(&form)
                .unwrap_or_else(|flow| panic!("{form}: {flow:?}"));
            assert!(result.truthy(), "vc-arch.el did not provide its feature");
        })
        .unwrap()
        .join()
        .expect("vc-arch load test thread panicked");
}
