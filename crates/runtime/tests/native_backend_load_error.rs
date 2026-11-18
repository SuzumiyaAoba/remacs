use remacs_runtime::lisp::{Flow, Interp};

// A defun declaration action runs after its function cell is installed.
// If it signals while comp.el loads, the Remacs C backend must be restored.
#[test]
fn failed_comp_source_load_restores_the_native_compile_backend() {
    let comp_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../lisp/comp.el")
        .canonicalize()
        .expect("bundled comp.el");
    let comp_path = comp_path.to_string_lossy().into_owned();

    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let mut interp = Interp::new_with_noninteractive(true);
            let source = format!(
                "(progn \
                   (defun qa-native-bridge-fail-ftype (name _arglist &rest _args) \
                     (if (eq name 'native-compile) \
                         '(error \"forced ftype declaration error\") nil)) \
                   (let* ((entry (assq 'ftype defun-declarations-alist)) \
                          (old (cdr entry)) \
                          (before (symbol-function 'native-compile))) \
                     (unwind-protect \
                         (progn \
                           (setcdr entry (list 'qa-native-bridge-fail-ftype)) \
                           (let ((load-result \
                                  (condition-case err \
                                      (load {comp_path:?} nil t t) \
                                    (error (list (car err) (cadr err)))))) \
                             (list load-result \
                                   (eq before (symbol-function 'native-compile))))) \
                       (setcdr entry old))))"
            );

            let result = match interp.eval_str(&source) {
                Ok(result) => result,
                Err(Flow::Signal(signal)) => panic!(
                    "probe signal {} {}",
                    interp.prin1_to_string(&signal.symbol()),
                    interp.prin1_to_string(&signal.data())
                ),
                Err(flow) => panic!("probe flow {flow:?}"),
            };
            assert_eq!(
                interp.prin1_to_string(&result).to_unicode_lossy(),
                "((error \"forced ftype declaration error\") t)"
            );
        })
        .expect("spawn load probe")
        .join()
        .expect("load probe thread");
}
