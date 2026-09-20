//! Coverage probe: call every safe subr once with plausible args,
//! wrapped in ignore-errors. Exercises dispatch/validation paths and
//! catches panics (which abort the whole test).
mod common;

use common::*;

macro_rules! probe {
    ($name:ident, $file:literal) => {
        #[test]
        fn $name() {
            let (mut i, _) = interp();
            match i.eval_str(include_str!($file)) {
                Ok(_) => {}
                Err(f) => panic!("probe failed: {:?}", f),
            }
        }
    };
}

probe!(probe_all_subrs_no_panic, "probe_all.el");
probe!(probe_deep_editing_session, "probe_deep.el");
probe!(probe_edge_typed_args, "probe_edge.el");
probe!(probe_runtime_semantics, "probe_runtime.el");
probe!(probe_editor_module, "probe_editor.el");
probe!(probe_buffer_ops, "probe_buffer2.el");
probe!(probe_lisp_deep, "probe_lisp2.el");
probe!(probe_misc_builtins, "probe_misc2.el");
probe!(probe_misc_builtins2, "probe_misc3.el");
probe!(probe_print_read, "probe_printread.el");
probe!(probe_eval_data, "probe_eval2.el");
probe!(probe_editor_ops, "probe_editor2.el");
probe!(probe_buffer_deep, "probe_buffer3.el");
probe!(probe_buffer_props, "probe_buffer4.el");
probe!(probe_editor_files, "probe_editor4.el");
probe!(probe_editor_branches, "probe_editor5.el");
probe!(probe_editor_deep, "probe_editor3.el");
probe!(probe_uncovered_subrs, "probe_gap.el");
probe!(probe_lisp_edges, "probe_lisp3.el");
probe!(probe_editor_kill_files, "probe_editor6.el");
probe!(probe_buffer_motion, "probe_buffer5.el");
probe!(probe_misc_builtins3, "probe_misc4.el");
probe!(probe_eval_branches, "probe_eval3.el");
probe!(probe_format_specs, "probe_format.el");
probe!(probe_evalfn_arith, "probe_eval4.el");
probe!(probe_eval_misc5, "probe_eval5.el");
probe!(probe_reader_data6, "probe_eval6.el");
probe!(probe_eval_misc7, "probe_eval7.el");
probe!(probe_eval_misc8, "probe_eval8.el");
probe!(probe_eval_misc9, "probe_eval9.el");
probe!(probe_process_charset10, "probe_eval10.el");
probe!(probe_process_charset11, "probe_eval11.el");
probe!(probe_process_cov12, "probe_eval12.el");
probe!(probe_eval13_misc, "probe_eval13.el");
probe!(probe_registers, "probe_registers.el");
