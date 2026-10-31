use remacs_runtime::lisp::{Flow, Interp, Value};

const CASES: &str = r#"
(progn
  (defun remacs-test-comment-search (text needle expected &rest options)
    (with-temp-buffer
      (let ((table (make-syntax-table)))
        (when (plist-get options :plain)
          (modify-syntax-entry ?\n " " table))
        (unless (plist-get options :plain)
          (modify-syntax-entry ?/ ". 124b" table)
          (modify-syntax-entry ?* ". 23" table)
          (modify-syntax-entry ?\n
                               (if (or (eq (plist-get options :style) 'line)
                                       (plist-get options :misleading)) ">" " ")
                               table)
          (when (eq (plist-get options :style) 'line)
            (modify-syntax-entry ?\; "<" table)))
        (set-syntax-table table))
      (setq-local comment-use-syntax (plist-get options :syntax)
                  comment-use-global-state t
                  comment-start "/*" comment-end "*/"
                  comment-start-skip "/\\*+" comment-end-skip "\\*/")
      (pcase (plist-get options :style)
        ('line
         (setq-local comment-start ";" comment-end ""
                     comment-start-skip ";+" comment-end-skip "\n"))
        ('html
         (setq-local comment-start "<!--" comment-end "-->"
                     comment-start-skip "<!--" comment-end-skip "-->"))
        ('same
         (setq-local comment-start "@" comment-end "@"
                     comment-start-skip "@" comment-end-skip "@")))
      (when (plist-get options :skip)
        (setq-local comment-start-skip (plist-get options :skip)))
      (when (plist-get options :normalize)
        (setq-local comment-start-skip nil comment-end-skip nil)
        (comment-normalize-vars))
      (insert text)
      (let ((narrow (plist-get options :narrow)))
        (when narrow
          (narrow-to-region (car narrow) (or (cadr narrow) (point-max)))))
      (goto-char (point-min))
      (cond ((numberp needle) (goto-char needle))
            (needle (search-forward needle))
            (t (goto-char (point-max))))
      (let* ((limit (plist-get options :limit))
             (bound (if (and limit (plist-get options :marker))
                        (copy-marker limit) limit))
             (actual
              (condition-case err
                  (list (if (plist-get options :beginning)
                            (comment-beginning)
                          (comment-search-backward bound
                                                   (not (plist-get options :error))))
                        (point))
                (error (list err (point))))))
        (when (markerp bound) (set-marker bound nil))
        (unless (equal actual expected)
          (error "Comment search %S %S %S: expected %S, got %S"
                 text needle options expected actual)))))
  (let ((cases
         '(("/* outer\n text /* fake\n tail */" "fake" (1 3))
           ("/* outer\n text /* fake\n tail */" "fake" (1 3) :misleading t)
           ("/* outer\n text /* fake\n tail */" "fake" (1 3) :plain t)
           ("/* outer\n text /* fake\n tail */" nil (1 3))
           ("/* outer\n text /* fake" nil (1 3))
           ("/* outer\n text /* fake" nil (1 3) :plain t)
           ("/* outer /* fake */" "fake" (1 3))
           ("/* first */\n/* second\n /* fake */" "fake" (13 15))
           ("/* one */\n/* two */" nil (11 13))
           ("/*one*/ /*two*/" nil (9 11))
           ("/*one*/ /*two*/" nil (9 11) :limit 9)
           ("/* outer\n text /* fake\n tail */" "fake" (1 3) :limit 1)
           ("/* outer\n text /* fake\n tail */" "fake" (nil 2) :limit 2)
           ("/* outer\n text /* fake\n tail */" "fake" (nil 12) :limit 12 :marker t)
           ("/* outer\n text /* fake\n tail */" "fake" ((error "No comment") 12)
            :limit 12 :error t)
           ("plain text" nil (nil 1))
           ("plain text" nil (nil 4) :limit 4)
           ("plain text" nil ((error "No comment") 1) :error t)
           ("  /* outer\n   /* fake */" "fake" (3 5)
            :skip "\\(^\\|[^\\\\\n]\\)/\\*+")
           ("/* outer\n text /* fake */" "fake" (1 4) :normalize t :plain t)
           ("head\n/* outer\n /* fake */" "fake" (6 8) :narrow (6))
           ("先頭\n/* 外側\n /* fake */" "fake" (4 6))
           (";;; outer ; fake\n;; next" "fake" (1 4) :style line)
           ("; one\n; two" nil (7 8) :style line)
           ("; one\n; two" nil (7 8) :style line :plain t)
           ("<!-- outer\n text <!-- fake -->" "fake" (1 5) :style html :plain t)
           ("@ first @ code @ second @" nil (16 17) :style same :plain t)
           ("@ first @" nil (1 2) :style same :plain t)
           ("/* outer\n text /* fake\n tail */" "fake" (1 3) :beginning t)
           ("/* outer\n text /* fake" "fake" (1 3) :beginning t :plain t)
           ("/* outer\n text /* fake\n tail */" "fake" (1 3) :syntax t)
           ("/* one */\n/* two */" nil (11 13) :syntax t))))
    (dolist (case cases) (apply #'remacs-test-comment-search case))
    (length cases)))
"#;

fn eval(interp: &mut Interp, source: &str) -> Value {
    interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    })
}

#[test]
fn backward_comment_search_ignores_starters_in_comment_bodies() {
    let mut interp = Interp::new();
    assert_eq!(eval(&mut interp, CASES).int(), Some(32));

    // Also check the actual shipped definition, independently of the AOT twin.
    let source = include_str!("../../../lisp/newcomment.el");
    let start = source.find("(defun comment-search-backward ").unwrap();
    let end = source[start..].find("\n(defun comment-beginning ").unwrap() + start;
    eval(&mut interp, &source[start..end]);
    assert_eq!(eval(&mut interp, CASES).int(), Some(32));
}
