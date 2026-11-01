use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn eval(interp: &mut Interp, source: &str) -> String {
    let value = interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    format!("{}", interp.prin1_to_string(&value))
}

#[test]
fn requiring_gnus_resolves_the_eagerly_expanded_reader_function() {
    let mut interp = Interp::new();
    eprintln!("Gnus load probe: bootstrap complete");
    assert_eq!(
        eval(
            &mut interp,
            "(progn (require 'gnus)\n                   (list (featurep 'gnus) (fboundp 'gnus-read-group)))"
        ),
        "(t t)"
    );
}

#[test]
fn mh_e_compile_time_gnus_dependency_loads_in_a_fresh_interpreter() {
    let mut interp = Interp::new();
    eprintln!("Gnus load probe: bootstrap complete");
    assert_eq!(
        eval(
            &mut interp,
            "(progn (require 'mh-e)\n                   (list (featurep 'mh-e) (featurep 'gnus)\n                         (fboundp 'gnus-read-group)))"
        ),
        "(t t t)"
    );
}

#[test]
fn gnus_aot_load_is_a_control_for_the_source_eager_expansion_path() {
    let mut interp = Interp::new();
    eprintln!("Gnus load probe: bootstrap complete");
    let compiled = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp-elc/gnus.elc")
        .canonicalize()
        .unwrap();
    let code = format!(
        "(progn (load {} nil t)\n               (list (featurep 'gnus) (fboundp 'gnus-read-group)))",
        lisp_string(compiled.to_str().unwrap())
    );
    assert_eq!(eval(&mut interp, &code), "(t t)");
}
