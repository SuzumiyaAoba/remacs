use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r#"
(progn
  (require 'project)
  (require 'etags-regen)
  (defvar remacs-etags-root nil)
  (defvar remacs-etags-files nil)
  (defvar remacs-etags-tags-buffer nil)
  (defvar remacs-etags-appended nil)
  (defun remacs-etags-all-files (root ignores)
    (let ((remacs-etags-root root)
          (etags-regen-ignores ignores))
      (cl-letf (((symbol-function 'project-root)
                 (lambda (_project) remacs-etags-root))
                ((symbol-function 'project-files)
                 (lambda (_project) (copy-sequence remacs-etags-files)))
                )
        (etags-regen--all-files 'fixture))))
  (defun remacs-etags-update-file (root ignore file)
    (let ((etags-regen--tags-root root)
          (etags-regen--tags-file (expand-file-name "TAGS" root))
          (etags-regen-ignores (list ignore))
          (etags-regen--new-file nil)
          (remacs-etags-tags-buffer
           (generate-new-buffer " *etags-regen-ignore-test*"))
          (remacs-etags-appended nil))
      (unwind-protect
          (with-temp-buffer
            (setq buffer-file-name file)
            (cl-letf (((symbol-function 'get-file-buffer)
                       (lambda (_file) remacs-etags-tags-buffer))
                      ((symbol-function 'etags-regen--remove-tag)
                       (lambda (_file) t))
                      ((symbol-function 'etags-regen--append-tags)
                       (lambda (file) (push file remacs-etags-appended)))
                      ((symbol-function 'save-buffer) #'ignore))
              (etags-regen--update-file))
            (nreverse remacs-etags-appended))
        (kill-buffer remacs-etags-tags-buffer))))
  (let* ((remacs-etags-files
          '("/repo/build/generated.c"
            "/repo/src/build/generated.c"
            "/repo/builder/generated.c"
            "/repo/src/builder/generated.c"
            "/repo/build.c"
            "/repo/root.c"
            "/repo/src/nested.c"
            "/repo/src/deeper/file.c"
            "/repo/sub/src/nested.c"
            "/repo/.cache/hidden.c"
            "/repo/src/.cache/hidden.c"
            "/repo/build-sub/generated.c"))
         (root-build
          '("/repo/src/build/generated.c"
            "/repo/builder/generated.c"
            "/repo/src/builder/generated.c"
            "/repo/build.c"
            "/repo/root.c"
            "/repo/src/nested.c"
            "/repo/src/deeper/file.c"
            "/repo/sub/src/nested.c"
            "/repo/.cache/hidden.c"
            "/repo/src/.cache/hidden.c"
            "/repo/build-sub/generated.c"))
         (all-builds
          '("/repo/builder/generated.c"
            "/repo/src/builder/generated.c"
            "/repo/build.c"
            "/repo/root.c"
            "/repo/src/nested.c"
            "/repo/src/deeper/file.c"
            "/repo/sub/src/nested.c"
            "/repo/.cache/hidden.c"
            "/repo/src/.cache/hidden.c"
            "/repo/build-sub/generated.c"))
         (root-build-dirs
          '("/repo/src/build/generated.c"
            "/repo/src/builder/generated.c"
            "/repo/build.c"
            "/repo/root.c"
            "/repo/src/nested.c"
            "/repo/src/deeper/file.c"
            "/repo/sub/src/nested.c"
            "/repo/.cache/hidden.c"
            "/repo/src/.cache/hidden.c"))
         (all-build-dirs
          '("/repo/build.c"
            "/repo/root.c"
            "/repo/src/nested.c"
            "/repo/src/deeper/file.c"
            "/repo/sub/src/nested.c"
            "/repo/.cache/hidden.c"
            "/repo/src/.cache/hidden.c"))
         (root-src-c
          '("/repo/build/generated.c"
            "/repo/builder/generated.c"
            "/repo/build.c"
            "/repo/root.c"
            "/repo/sub/src/nested.c"
            "/repo/.cache/hidden.c"
            "/repo/build-sub/generated.c"))
         (all-src-c
          '("/repo/build/generated.c"
            "/repo/builder/generated.c"
            "/repo/build.c"
            "/repo/root.c"
            "/repo/.cache/hidden.c"
            "/repo/build-sub/generated.c"))
         (root-cache
          '("/repo/build/generated.c"
            "/repo/src/build/generated.c"
            "/repo/builder/generated.c"
            "/repo/src/builder/generated.c"
            "/repo/build.c"
            "/repo/root.c"
            "/repo/src/nested.c"
            "/repo/src/deeper/file.c"
            "/repo/sub/src/nested.c"
            "/repo/src/.cache/hidden.c"
            "/repo/build-sub/generated.c"))
         (all-cache
          '("/repo/build/generated.c"
            "/repo/src/build/generated.c"
            "/repo/builder/generated.c"
            "/repo/src/builder/generated.c"
            "/repo/build.c"
            "/repo/root.c"
            "/repo/src/nested.c"
            "/repo/src/deeper/file.c"
            "/repo/sub/src/nested.c"
            "/repo/build-sub/generated.c")))
    (list
     (equal (remacs-etags-all-files "/repo" '("./build")) root-build)
     (equal (remacs-etags-all-files "/repo/" '("./build")) root-build)
     (equal (remacs-etags-all-files "/repo" '("build")) all-builds)
     (equal (remacs-etags-all-files "/repo" '("./build/")) root-build)
     (equal (remacs-etags-all-files "/repo" '("build*/")) all-build-dirs)
     (equal (remacs-etags-all-files "/repo" '("./build*/")) root-build-dirs)
     (equal (remacs-etags-all-files "/repo" '("./src/*.c")) root-src-c)
     (equal (remacs-etags-all-files "/repo" '("src/*.c")) all-src-c)
     (equal (remacs-etags-all-files "/repo" '("./.cache")) root-cache)
     (equal (remacs-etags-all-files "/repo" '(".cache")) all-cache)
     (list
      (equal (remacs-etags-update-file "/repo" "./build"
                                       "/repo/build/generated.c") nil)
      (equal (remacs-etags-update-file "/repo" "./build"
                                       "/repo/src/build/generated.c")
             '("/repo/src/build/generated.c"))
      (equal (remacs-etags-update-file "/repo" "build"
                                       "/repo/build/generated.c") nil)
      (equal (remacs-etags-update-file "/repo" "build"
                                       "/repo/src/build/generated.c") nil)
      (equal (remacs-etags-update-file "/repo" "build"
                                       "/repo/builder/generated.c")
             '("/repo/builder/generated.c"))
      (equal (remacs-etags-update-file "/repo" "./build/"
                                       "/repo/build")
             '("/repo/build"))
      (equal (remacs-etags-update-file "/repo" "./build"
                                       "/repo/build") nil)
      (equal (remacs-etags-update-file "/repo/" "./build"
                                       "/repo/build/generated.c") nil)
      (equal (remacs-etags-update-file "/repo" "./src/*.c"
                                       "/repo/src/nested.c") nil)
      (equal (remacs-etags-update-file "/repo" "./src/*.c"
                                       "/repo/sub/src/nested.c")
             '("/repo/sub/src/nested.c"))))))
"#;

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
fn etags_ignores_match_project_root_and_incremental_paths() {
    let mut interp = Interp::new();
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT")).join("lisp/etags-regen.el");
    eval(&mut interp, &format!("(load {} nil t)", lisp_string(source.to_str().unwrap())));
    let expected = "(t t t t t t t t t t (t t t t t t t t t t))";
    assert_eq!(eval(&mut interp, CASES), expected);
}
