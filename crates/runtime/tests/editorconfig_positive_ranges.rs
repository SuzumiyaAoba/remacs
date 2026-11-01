use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r###"(progn
  (require 'cl-lib)
  (defun remacs-3057-check (label expected name pattern)
    (let ((actual (editorconfig-fnmatch-p name pattern)))
      (unless (equal actual (and expected 0))
        (error "EditorConfig numeric range #3057 failed: %S -> %S" label actual))
      t))
  (let ((case-fold-search nil))
    (remacs-3057-check 'canonical-in-range t "2" "{1..3}")
    (remacs-3057-check 'explicit-plus-in-range t "+2" "{1..3}")
    (remacs-3057-check 'explicit-plus-with-leading-zero t "+02" "{1..3}")
    (remacs-3057-check 'explicit-plus-with-many-leading-zeroes t "+0002" "{1..3}")
    (remacs-3057-check 'explicit-plus-lower-bound t "+1" "{1..3}")
    (remacs-3057-check 'explicit-plus-upper-bound t "+3" "{1..3}")
    (remacs-3057-check 'explicit-plus-out-of-range nil "+4" "{1..3}")
    (remacs-3057-check 'negative-not-in-positive-range nil "-2" "{1..3}")
    (remacs-3057-check 'signed-endpoints-canonical t "2" "{+1..+3}")
    (remacs-3057-check 'signed-endpoints-plus-candidate t "+2" "{+1..+3}")
    (remacs-3057-check 'mixed-endpoint-signs t "+1" "{-1..+1}")
    (remacs-3057-check 'explicit-plus-zero t "+0" "{-1..1}")
    (remacs-3057-check 'explicit-plus-zero-with-padding t "+000" "{-1..1}")
    (remacs-3057-check 'canonical-zero t "0" "{-1..1}")
    (remacs-3057-check 'negative-range-value t "-2" "{-3..-1}")
    (remacs-3057-check 'positive-sign-outside-negative-range nil "+2" "{-3..-1}")
    ;; Keep the current canonicalization of range endpoints with leading zeros.
    (remacs-3057-check 'leading-zero-endpoint-canonical t "2" "{01..03}")
    (remacs-3057-check 'leading-zero-input-stays-unmatched nil "02" "{01..03}")
    (remacs-3057-check 'plus-leading-zero-endpoint t "+02" "{01..03}")
    (remacs-3057-check 'plus-with-leading-zero-endpoint t "+2" "{01..03}")
    (remacs-3057-check 'negative-leading-zero-endpoint-canonical t "-2" "{-03..-01}")
    (remacs-3057-check 'negative-leading-zero-input-stays-unmatched nil "-02" "{-03..-01}")
    ;; The existing ascending endpoint order is retained.
    (remacs-3057-check 'reversed-range-stays-unmatched nil "2" "{3..1}")
    ;; Numeric expansion remains composable with other brace features.
    (remacs-3057-check 'string-alternatives-unchanged t "b" "{a,b}")
    (remacs-3057-check 'nested-numeric-range-plus t "file+2.txt" "{plain,file{1..3}.txt}")
    (remacs-3057-check 'ordinary-wildcard-unchanged t "foo.c" "*.c"))
  t)"###;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[test]
fn editorconfig_fnmatch_numeric_ranges_accept_explicit_plus_values() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/editorconfig-fnmatch.el")
        .canonicalize()
        .unwrap();
    let code = format!(
        "(progn (setq noninteractive t) (load {} nil t t) {})",
        lisp_string(source.to_str().unwrap()),
        CASES
    );
    let mut interp = Interp::new();
    let result = interp.eval_str(&code).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    assert!(result.truthy());
}
