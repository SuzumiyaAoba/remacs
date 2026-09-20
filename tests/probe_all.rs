//! Coverage probe: call every safe subr once with plausible args,
//! wrapped in ignore-errors. Exercises dispatch/validation paths and
//! catches panics (which abort the whole test).
mod common;

use common::*;

#[test]
fn probe_all_subrs_no_panic() {
    let (mut i, _) = interp();
    let src = include_str!("probe_all.el");
    match i.eval_str(src) {
        Ok(_) => {}
        Err(f) => panic!("probe failed: {:?}", f),
    }
}

#[test]
fn probe_deep_editing_session() {
    let (mut i, _) = interp();
    let src = include_str!("probe_deep.el");
    match i.eval_str(src) {
        Ok(_) => {}
        Err(f) => panic!("probe failed: {:?}", f),
    }
}

#[test]
fn probe_edge_typed_args() {
    let (mut i, _) = interp();
    let src = include_str!("probe_edge.el");
    match i.eval_str(src) {
        Ok(_) => {}
        Err(f) => panic!("probe failed: {:?}", f),
    }
}

#[test]
fn probe_runtime_semantics() {
    let (mut i, _) = interp();
    let src = include_str!("probe_runtime.el");
    match i.eval_str(src) {
        Ok(_) => {}
        Err(f) => panic!("probe failed: {:?}", f),
    }
}

#[test]
fn probe_editor_module() {
    let (mut i, _) = interp();
    let src = include_str!("probe_editor.el");
    match i.eval_str(src) {
        Ok(_) => {}
        Err(f) => panic!("probe failed: {:?}", f),
    }
}

#[test]
fn probe_buffer_ops() {
    let (mut i, _) = interp();
    let src = include_str!("probe_buffer2.el");
    match i.eval_str(src) {
        Ok(_) => {}
        Err(f) => panic!("probe failed: {:?}", f),
    }
}

#[test]
fn probe_lisp_deep() {
    let (mut i, _) = interp();
    let src = include_str!("probe_lisp2.el");
    match i.eval_str(src) {
        Ok(_) => {}
        Err(f) => panic!("probe failed: {:?}", f),
    }
}

#[test]
fn probe_misc_builtins() {
    let (mut i, _) = interp();
    let src = include_str!("probe_misc2.el");
    match i.eval_str(src) {
        Ok(_) => {}
        Err(f) => panic!("probe failed: {:?}", f),
    }
}

#[test]
fn probe_print_read() {
    let (mut i, _) = interp();
    let src = include_str!("probe_printread.el");
    match i.eval_str(src) {
        Ok(_) => {}
        Err(f) => panic!("probe failed: {:?}", f),
    }
}

#[test]
fn probe_eval_data() {
    let (mut i, _) = interp();
    let src = include_str!("probe_eval2.el");
    match i.eval_str(src) {
        Ok(_) => {}
        Err(f) => panic!("probe failed: {:?}", f),
    }
}

#[test]
fn probe_editor_ops() {
    let (mut i, _) = interp();
    let src = include_str!("probe_editor2.el");
    match i.eval_str(src) {
        Ok(_) => {}
        Err(f) => panic!("probe failed: {:?}", f),
    }
}

#[test]
fn probe_buffer_deep() {
    let (mut i, _) = interp();
    let src = include_str!("probe_buffer3.el");
    match i.eval_str(src) {
        Ok(_) => {}
        Err(f) => panic!("probe failed: {:?}", f),
    }
}
