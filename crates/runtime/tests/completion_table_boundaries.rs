use remacs_runtime::lisp::{Flow, Interp, Value};

const CASES: &str = r#"
(progn
  (defun remacs-test-completion-equal (label actual expected)
    (unless (equal actual expected)
      (error "%S: expected %S, got %S" label expected actual)))
  (defun remacs-test-completion-empty (_string _pred action)
    (when (eq (car-safe action) 'boundaries) '(boundaries 0 . 0)))
  (defun remacs-test-completion-prefixed (_string _pred action)
    (cond ((eq action t) '("prefix/item"))
          ((eq (car-safe action) 'boundaries) '(boundaries 7 . 0))))
  (defun remacs-test-completion-filtered (string pred action)
    (if (eq (car-safe action) 'boundaries)
        '(boundaries 0 . 0)
      (complete-with-action action '("prefix/blocked") string pred)))
  (defvar remacs-test-completion-candidates nil)
  (defun remacs-test-completion-changing (string pred action)
    (if (eq (car-safe action) 'boundaries)
        '(boundaries 0 . 0)
      (complete-with-action action remacs-test-completion-candidates string pred)))
  (dolist (constructor '(completion-table-in-turn completion-table-merge))
    (let ((table (funcall constructor #'remacs-test-completion-empty
                          #'remacs-test-completion-prefixed)))
      (remacs-test-completion-equal constructor (all-completions "prefix/" table)
                                    '("prefix/item"))
      (remacs-test-completion-equal constructor (completion-boundaries "prefix/" table nil "")
                                    '(7 . 0)))
    (let ((table (funcall constructor #'remacs-test-completion-filtered
                          #'remacs-test-completion-prefixed)))
      (remacs-test-completion-equal 'unfiltered (completion-boundaries "prefix/" table nil "")
                                    '(0 . 0))
      (remacs-test-completion-equal 'filtered
                                    (completion-boundaries "prefix/" table
                                                           (lambda (s) (equal s "prefix/item")) "")
                                    '(7 . 0)))
    ;; A non-function table has implicit boundaries; its nil response must
    ;; not select a later table's explicit boundaries.
    (let ((table (funcall constructor '("prefix/default") #'remacs-test-completion-prefixed)))
      (remacs-test-completion-equal 'implicit (completion-boundaries "prefix/" table nil "suffix")
                                    '(0 . 6)))
    (remacs-test-completion-equal 'empty
                                  (completion-boundaries "" (funcall constructor) nil "suffix")
                                  '(0 . 6))
    (let ((table (funcall constructor #'remacs-test-completion-empty nil)))
      (remacs-test-completion-equal 'empty-fallback (completion-boundaries "prefix/" table nil "suffix")
                                    '(0 . 0)))
    (let ((remacs-test-completion-candidates nil)
          (table (funcall constructor #'remacs-test-completion-changing
                          #'remacs-test-completion-prefixed)))
      (remacs-test-completion-equal 'initial (completion-boundaries "prefix/" table nil "") '(7 . 0))
      (setq remacs-test-completion-candidates '("prefix/new"))
      (remacs-test-completion-equal 'changed (completion-boundaries "prefix/" table nil "") '(0 . 0))
      (setq remacs-test-completion-candidates nil)
      (remacs-test-completion-equal 'changed-back (completion-boundaries "prefix/" table nil "") '(7 . 0)))
    (let* ((string (propertize "prefix/" 'face 'bold))
           (suffix (propertize "item tail" 'face 'italic))
           (pred #'identity)
           (calls nil)
           (empty (lambda (_s _p action)
                    (push 'empty calls)
                    (unless (eq action t) (error "Queried an inactive table's boundaries"))))
           (active (lambda (s p action)
                     (unless (and (eq s string) (eq p pred)) (error "Changed completion arguments"))
                     (push 'active calls)
                     (if (eq action t) '("prefix/item")
                       (unless (and (eq (car action) 'boundaries) (eq (cdr action) suffix))
                         (error "Changed the suffix object"))
                       '(boundaries 7 . 4))))
           (table (funcall constructor empty active)))
      (remacs-test-completion-equal 'suffix (completion-boundaries string table pred suffix) '(7 . 4))
      (remacs-test-completion-equal 'callback-order (nreverse calls) '(empty active active)))
    ;; Boundary probing must not add candidate queries to the other actions.
    (let* ((calls nil)
           (first (lambda (_s _p action)
                    (push action calls)
                    (when (eq action 'metadata) '(metadata (category . first)))))
           (second (lambda (_s _p action)
                     (push action calls)
                     (cond ((null action) "alpha") ((eq action t) '("alpha"))
                           ((eq action 'lambda) t))))
           (table (funcall constructor first second)))
      (remacs-test-completion-equal 'try (try-completion "al" table) "alpha")
      (remacs-test-completion-equal 'try-calls (nreverse calls) '(nil nil))
      (setq calls nil)
      (remacs-test-completion-equal 'all (all-completions "al" table) '("alpha"))
      (remacs-test-completion-equal 'all-calls (nreverse calls) '(t t))
      (setq calls nil)
      (remacs-test-completion-equal 'test (test-completion "alpha" table) t)
      (remacs-test-completion-equal 'test-calls (nreverse calls) '(lambda lambda))
      (setq calls nil)
      (remacs-test-completion-equal 'metadata (completion-metadata "al" table nil)
                                    '(metadata (category . first)))
      (remacs-test-completion-equal 'metadata-calls calls '(metadata)))
    (remacs-test-completion-equal 'collection-order
                                  (all-completions "" (funcall constructor '("alpha") '("beta")))
                                  (if (eq constructor 'completion-table-merge)
                                      '("alpha" "beta") '("alpha"))))
  t)
"#;

const FILE_CASES: &str = r#"
(let* ((directory (make-temp-file "remacs-completion-boundaries-" t))
       (prefix (file-name-as-directory directory))
       (variable "REMACS_COMPLETION_BOUNDARY_TEST")
       (previous (getenv variable)))
  (unwind-protect
      (progn
        (with-temp-file (concat prefix "alpha") (insert "data"))
        (remacs-test-completion-equal
         'file-match (completion-boundaries (concat prefix "alp") #'read-file-name-internal nil "")
         (cons (length prefix) 0))
        (remacs-test-completion-equal
         'file-no-match (completion-boundaries (concat prefix "missing") #'read-file-name-internal nil "")
         (cons (length prefix) 0))
        (setenv variable directory)
        (remacs-test-completion-equal
         'environment (completion-boundaries "$REMACS_COMPLETION_BOUNDARY_T" #'read-file-name-internal nil "")
         '(1 . 0))
        t)
    (setenv variable previous)
    (delete-directory directory t)))
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
fn table_boundaries_follow_matching_candidates_and_preserve_file_fallbacks() {
    let mut interp = Interp::new();
    assert!(eval(&mut interp, CASES).truthy());
    assert!(eval(&mut interp, FILE_CASES).truthy());

    // The prelude has bootstrap closure construction; test the shipped
    // lexical definitions independently without reloading unrelated settings.
    let source = include_str!("../../../lisp/minibuffer.el");
    let start = source
        .find("(defun completion--table-in-turn-boundaries ")
        .unwrap();
    let end = source[start..]
        .find("\n(defun completion-table-with-quoting ")
        .unwrap()
        + start;
    eval(&mut interp, &format!("(eval '(progn {}) t)", &source[start..end]));
    assert!(eval(&mut interp, CASES).truthy());
    assert!(eval(&mut interp, FILE_CASES).truthy());
}
