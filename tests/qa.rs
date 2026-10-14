//! GNU-compat QA suite runner: each test loads `qa_framework.el`
//! (assertion harness) plus one `qa_*.el` file of `(qa FORM EXPECTED)`
//! checks and asserts the summary "N checks passed".  Every expected
//! value is verified against GNU Emacs 31.1 (`emacs -Q --batch`).
//!
//! The qa_*.el files are also directly loadable:
//!   emacs -Q --batch -l tests/qa_framework.el -l tests/qa_<dom>.el
//!   remacs --batch --load tests/qa_framework.el --load tests/qa_<dom>.el
#![recursion_limit = "4096"]
mod common;

use common::*;

/// Load framework + test file through `load' so the file sees the same
/// environment as the GNU `-l' run (noninteractive=t, load context).
fn run(file: &'static str) {
    let src = format!(
        "(progn (setq noninteractive t) \
                (load {:?}) (load {:?}) nil)",
        format!("{}/tests/qa_framework.el", env!("CARGO_MANIFEST_DIR")),
        format!("{}/tests/{}", env!("CARGO_MANIFEST_DIR"), file)
    );
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let (mut i, out) = interp();
            i.noninteractive = true;
            match i.eval_str(&src) {
                Ok(_) => {
                    let o = out.borrow();
                    assert!(o.contains("checks passed"), "qa {file} summary: {o}");
                }
                Err(_f) => panic!("qa {file} eval error; captured output:\n{}", out.borrow()),
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

macro_rules! qa {
    ($name:ident, $file:literal) => {
        #[test]
        fn $name() {
            run($file);
        }
    };
}

qa!(qa_abbrev, "qa_abbrev.el");
qa!(qa_abbrev2, "qa_abbrev2.el");
qa!(qa_absolute_name, "qa_absolute_name.el");
qa!(qa_advice, "qa_advice.el");
qa!(qa_apply, "qa_apply.el");
qa!(qa_apply2, "qa_apply2.el");
qa!(qa_arith, "qa_arith.el");
qa!(qa_array, "qa_array.el");
qa!(qa_backquote, "qa_backquote.el");
qa!(qa_backup, "qa_backup.el");
qa!(qa_buffer, "qa_buffer.el");
qa!(qa_buffer2, "qa_buffer2.el");
qa!(qa_bufferfile, "qa_bufferfile.el");
qa!(qa_buffmenu, "qa_buffmenu.el");
qa!(qa_case, "qa_case.el");
qa!(qa_char, "qa_char.el");
qa!(qa_charset, "qa_charset.el");
qa!(
    qa_decomposition_description,
    "qa_decomposition_description.el"
);
qa!(qa_chartables, "qa_chartables.el");
qa!(qa_clloop, "qa_clloop.el");
qa!(qa_clloop2, "qa_clloop2.el");
qa!(qa_clloop3, "qa_clloop3.el");
qa!(qa_clloop4, "qa_clloop4.el");
qa!(qa_clloop5, "qa_clloop5.el");
qa!(qa_clloop6, "qa_clloop6.el");
qa!(qa_clloop7, "qa_clloop7.el");
qa!(qa_clmisc, "qa_clmisc.el");
qa!(qa_clprint, "qa_clprint.el");
qa!(qa_clseq, "qa_clseq.el");
qa!(qa_clseq2, "qa_clseq2.el");
qa!(qa_cmds, "qa_cmds.el");
qa!(qa_coding, "qa_coding.el");
qa!(qa_coding2, "qa_coding2.el");
qa!(qa_completion, "qa_completion.el");
qa!(qa_ctl, "qa_ctl.el");
qa!(qa_custom, "qa_custom.el");
qa!(qa_custom2, "qa_custom2.el");
qa!(qa_descr, "qa_descr.el");
qa!(qa_doc, "qa_doc.el");
qa!(qa_eieio, "qa_eieio.el");
qa!(qa_eieio2, "qa_eieio2.el");
qa!(qa_err2, "qa_err2.el");
qa!(qa_errors, "qa_errors.el");
qa!(qa_eval, "qa_eval.el");
qa!(qa_face, "qa_face.el");
qa!(qa_face3, "qa_face3.el");
qa!(qa_faces2, "qa_faces2.el");
qa!(qa_fileattr, "qa_fileattr.el");
qa!(qa_file_modes, "qa_file_modes.el");
qa!(qa_filelocal, "qa_filelocal.el");
qa!(qa_filesys, "qa_filesys.el");
qa!(qa_fill, "qa_fill.el");
qa!(qa_fontlock, "qa_fontlock.el");
qa!(qa_format, "qa_format.el");
qa!(qa_globdir, "qa_globdir.el");
qa!(qa_gv, "qa_gv.el");
qa!(qa_hash, "qa_hash.el");
qa!(qa_hash2, "qa_hash2.el");
qa!(qa_help, "qa_help.el");
qa!(qa_hook, "qa_hook.el");
qa!(qa_hooks2, "qa_hooks2.el");
qa!(qa_indent, "qa_indent.el");
qa!(qa_indentcmds, "qa_indentcmds.el");
qa!(qa_indentfill, "qa_indentfill.el");
qa!(qa_input, "qa_input.el");
qa!(qa_json, "qa_json.el");
qa!(qa_kbd, "qa_kbd.el");
qa!(qa_keymap, "qa_keymap.el");
qa!(qa_keymap2, "qa_keymap2.el");
qa!(qa_keymap3, "qa_keymap3.el");
qa!(qa_kmacro, "qa_kmacro.el");
qa!(qa_list, "qa_list.el");
qa!(qa_lists, "qa_lists.el");
qa!(qa_lists2, "qa_lists2.el");
qa!(qa_load, "qa_load.el");
qa!(qa_logb, "qa_logb.el");
qa!(qa_locvar, "qa_locvar.el");
qa!(qa_macro, "qa_macro.el");
qa!(qa_map, "qa_map.el");
qa!(qa_map2, "qa_map2.el");
qa!(qa_mapring, "qa_mapring.el");
qa!(qa_mark, "qa_mark.el");
qa!(qa_marker, "qa_marker.el");
qa!(qa_markers, "qa_markers.el");
qa!(qa_match, "qa_match.el");
qa!(qa_math, "qa_math.el");
qa!(qa_minibuf, "qa_minibuf.el");
qa!(qa_minibuffer, "qa_minibuffer.el");
qa!(qa_misc2, "qa_misc2.el");
qa!(qa_misc3, "qa_misc3.el");
qa!(qa_miscfn, "qa_miscfn.el");
qa!(qa_modes, "qa_modes.el");
qa!(qa_narrow, "qa_narrow.el");
qa!(qa_numbers, "qa_numbers.el");
qa!(qa_obarray, "qa_obarray.el");
qa!(qa_overlay, "qa_overlay.el");
qa!(qa_overlays, "qa_overlays.el");
qa!(qa_pcase, "qa_pcase.el");
qa!(qa_plists, "qa_plists.el");
qa!(qa_point, "qa_point.el");
qa!(qa_pp, "qa_pp.el");
qa!(qa_printvars, "qa_printvars.el");
qa!(qa_process, "qa_process.el");
qa!(qa_radix, "qa_radix.el");
qa!(qa_read, "qa_read.el");
qa!(qa_readprint, "qa_readprint.el");
qa!(qa_rect, "qa_rect.el");
qa!(qa_regex, "qa_regex.el");
qa!(qa_regexpopt, "qa_regexpopt.el");
qa!(qa_replace, "qa_replace.el");
qa!(qa_ring, "qa_ring.el");
qa!(qa_rx, "qa_rx.el");
qa!(qa_search, "qa_search.el");
qa!(qa_seq, "qa_seq.el");
qa!(qa_seq2, "qa_seq2.el");
qa!(qa_sort, "qa_sort.el");
qa!(qa_special, "qa_special.el");
qa!(qa_special2, "qa_special2.el");
qa!(qa_string, "qa_string.el");
qa!(qa_string_search, "qa_string_search.el");
qa!(qa_strings, "qa_strings.el");
qa!(qa_strings2, "qa_strings2.el");
qa!(qa_subrinfo, "qa_subrinfo.el");
qa!(qa_subrx, "qa_subrx.el");
qa!(qa_subrx2, "qa_subrx2.el");
qa!(qa_subrx3, "qa_subrx3.el");
qa!(qa_symbol, "qa_symbol.el");
qa!(qa_syntax, "qa_syntax.el");
qa!(qa_syntax2, "qa_syntax2.el");
qa!(qa_syntaxtable, "qa_syntaxtable.el");
qa!(qa_system, "qa_system.el");
qa!(qa_tablist, "qa_tablist.el");
qa!(qa_textprops, "qa_textprops.el");
qa!(qa_thing, "qa_thing.el");
qa!(qa_thing2, "qa_thing2.el");
qa!(qa_diffface, "qa_diffface.el");
qa!(qa_time, "qa_time.el");
qa!(qa_timefmt, "qa_timefmt.el");
qa!(qa_timer, "qa_timer.el");
qa!(qa_types2, "qa_types2.el");
qa!(qa_ucs, "qa_ucs.el");
qa!(qa_undo, "qa_undo.el");
qa!(qa_vector, "qa_vector.el");
qa!(qa_window, "qa_window.el");
qa!(qa_winstate, "qa_winstate.el");
qa!(qa_xref, "qa_xref.el");
