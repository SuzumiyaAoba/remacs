//! remacs — an Emacs-compatible editor written in Rust.
//!
//! Architecture: a faithful Emacs *Lisp runtime* (values, reader,
//! evaluator, builtins) layered over a gap-buffer text core, an
//! editor layer (keymaps, command loop, windows, minibuffer), and a
//! terminal front-end that never blocks the UI thread.

pub mod buffer;
pub mod editor;
pub mod lisp;
pub mod term;

#[cfg(test)]
mod coverage_tests {
    //! Mirror of tests/probe_all.rs: the unit-test binary carries its
    //! own instrumented copy of this crate, so the probe suite must run
    //! here too for coverage to count.
    use crate::lisp::Interp;

    macro_rules! probe {
        ($name:ident, $file:literal) => {
            #[test]
            fn $name() {
                let mut i = Interp::new();
                match i.eval_str(include_str!(concat!("../tests/", $file))) {
                    Ok(_) => {}
                    Err(f) => panic!("probe {} failed: {:?}", $file, f),
                }
            }
        };
    }

    probe!(u_probe_all, "probe_all.el");
    probe!(u_probe_deep, "probe_deep.el");
    probe!(u_probe_edge, "probe_edge.el");
    probe!(u_probe_runtime, "probe_runtime.el");
    probe!(u_probe_editor, "probe_editor.el");
    probe!(u_probe_buffer2, "probe_buffer2.el");
    probe!(u_probe_lisp2, "probe_lisp2.el");
    probe!(u_probe_misc2, "probe_misc2.el");
    probe!(u_probe_misc3, "probe_misc3.el");
    probe!(u_probe_printread, "probe_printread.el");
    probe!(u_probe_eval2, "probe_eval2.el");
    probe!(u_probe_editor2, "probe_editor2.el");
    probe!(u_probe_buffer3, "probe_buffer3.el");
    probe!(u_probe_buffer4, "probe_buffer4.el");
    probe!(u_probe_buffer5, "probe_buffer5.el");
    probe!(u_probe_editor4, "probe_editor4.el");
    probe!(u_probe_editor5, "probe_editor5.el");
    probe!(u_probe_editor3, "probe_editor3.el");
    probe!(u_probe_editor6, "probe_editor6.el");
    probe!(u_probe_gap, "probe_gap.el");
    probe!(u_probe_lisp3, "probe_lisp3.el");
    probe!(u_probe_misc4, "probe_misc4.el");
    probe!(u_probe_eval3, "probe_eval3.el");
    probe!(u_probe_eval4, "probe_eval4.el");
    probe!(u_probe_eval5, "probe_eval5.el");
    probe!(u_probe_eval6, "probe_eval6.el");
    probe!(u_probe_eval7, "probe_eval7.el");
    probe!(u_probe_format, "probe_format.el");
}
