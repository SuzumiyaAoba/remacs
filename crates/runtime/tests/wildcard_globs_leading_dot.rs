use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r###"(progn
  (defun remacs-3232-check (label condition)
    (unless condition
      (error "wildcard-to-regexp #3232 failed: %S" label))
    t)
  (defun remacs-3232-match (pattern text)
    (not (null (string-match-p (wildcard-to-regexp pattern) text))))
  (defun remacs-3232-regexp-with-fold (pattern fold)
    (let ((case-fold-search fold))
      (wildcard-to-regexp pattern)))
  (defun remacs-3232-regexp-matches-with-fold (regexp text fold)
    (let ((case-fold-search fold))
      (string-match-p regexp text)))
  (defun remacs-3232-casefold-grid ()
    (dolist (generation '(nil t))
      (dolist (pair '(("[a]" . "[a]")
                      ("[A-Z]" . "[A-Z]")
                      ("[!a]" . "[^a]")
                      ("[!A-Z]" . "[^A-Z]")
                      ("[[:alpha:]]" . "[[:alpha:]]")))
        (let ((regexp (remacs-3232-regexp-with-fold (car pair) generation)))
          (dolist (matching '(nil t))
            (let ((case-fold-search matching))
              (dolist (text '("a" "A" "b" "B" "0" "!" "." "/" "ä" "Ä"))
                (let ((expected (and (not (member text '("." "/")))
                                     (string-match-p (cdr pair) text)))
                      (actual (string-match-p regexp text)))
                  (unless (eq (not (null expected)) (not (null actual)))
                    (error "Class polarity gen=%S match=%S pattern=%S text=%S"
                           generation matching (car pair) text)))))))))
    t)

  (defun remacs-3232-separator-quote-parity ()
    (and (equal (wildcard--glob-unquote-path-separators "a\\/b") "a/b")
         (equal (wildcard--glob-unquote-path-separators "a\\\\/b")
                "a\\\\/b")
         (equal (wildcard--glob-unquote-path-separators "a\\\\\\/b")
                "a\\\\/b")))
  (defun remacs-3232-file-names (directory pattern &optional regexp)
    (mapcar #'file-name-nondirectory
            (file-expand-wildcards
             (expand-file-name pattern directory) nil regexp)))
  (defun remacs-3232-negative-class-ascii-membership ()
    (let* ((case-fold-search nil)
           (regexp (wildcard-to-regexp "a[!\\-\\\\]"))
           (code 0)
           (ok t))
      (while (< code 128)
        (let ((expected (not (memq code '(45 47 92))))
              (text (concat "a" (char-to-string code))))
          (unless (eq (not (null (string-match-p regexp text))) expected)
            (setq ok nil)))
        (setq code (1+ code)))
      ok))
  (defun remacs-3232-current-dot-slash-aliases-hidden ()
    (with-temp-buffer
      (let* ((standard (standard-case-table))
             (table (copy-case-table standard))
             (canonical (copy-sequence (char-table-extra-slot standard 1)))
             (equivalent (copy-sequence (char-table-extra-slot standard 2)))
             (case-fold-search t))
        (aset canonical ?. ?@)
        (aset canonical ?/ ?@)
        (aset canonical ?@ ?@)
        ;; Keep a well-formed three-character equivalence cycle.
        (aset equivalent ?@ ?.)
        (aset equivalent ?. ?/)
        (aset equivalent ?/ ?@)
        (set-char-table-extra-slot table 1 canonical)
        (set-char-table-extra-slot table 2 equivalent)
        (set-case-table table)
        (let ((regexp (wildcard-to-regexp "[[:punct:]]"))
              (star (wildcard-to-regexp "*")))
          (and (not (string-match-p regexp "@"))
               (not (string-match-p star "@"))
               (string-match-p regexp "!"))))))
  (defun remacs-3232-byte8-negative-class ()
    (let* ((case-fold-search nil)
           (byte8 (char-to-string #x3fff81))
           (other (char-to-string #x3fff82))
           (pattern (concat "a[!" byte8 "]"))
           (regexp (wildcard-to-regexp pattern)))
      (and (not (string-match-p regexp (concat "a" byte8)))
           (string-match-p regexp (concat "a" other)))))
  (defun remacs-3232-high-encoding-punctuation-snapshot ()
    (let* ((case-fold-search nil)
           (class "[[:punct:]]")
           (regexp (wildcard-to-regexp class))
           (characters (mapcar #'char-to-string
                               '(#x3fff7f #x3fff80 #x3fff81 #x3fffff))))
      (not (catch 'unexpected-match
             (dolist (character characters)
               (when (or (string-match-p class character)
                         (string-match-p regexp character))
                 (throw 'unexpected-match t)))
             nil))))
  (defun remacs-3232-syntax-class-snapshot ()
    (with-temp-buffer
      (let ((table (copy-syntax-table (standard-syntax-table)))
            (case-fold-search nil))
        (modify-syntax-entry ?. "w" table)
        (modify-syntax-entry ?@ "w" table)
        (set-syntax-table table)
        (let ((regexp (wildcard-to-regexp "[[:word:]]")))
          (set-syntax-table (standard-syntax-table))
          (and (string-match-p regexp "@")
               (not (string-match-p regexp "."))
               (not (string-match-p regexp "/")))))))
  (remacs-3232-check 'slash-quote-removes-one-only-for-odd-runs
                     (remacs-3232-separator-quote-parity))
  (let ((case-fold-search nil))
    (remacs-3232-check 'star-hides-dot
                       (not (remacs-3232-match "*" ".hidden")))
    (remacs-3232-check 'star-matches-visible
                       (remacs-3232-match "*" "visible"))
    (remacs-3232-check 'star-never-crosses-slash
                       (not (remacs-3232-match "*" "/")))
    (remacs-3232-check 'star-matches-empty
                       (remacs-3232-match "*" ""))
    (remacs-3232-check 'question-hides-dot
                       (not (remacs-3232-match "?" ".")))
    (remacs-3232-check 'question-never-crosses-slash
                       (not (remacs-3232-match "?" "/")))
    (remacs-3232-check 'question-matches-one
                       (remacs-3232-match "?" "x"))
    (remacs-3232-check 'dot-is-explicit
                       (remacs-3232-match ".*" ".hidden"))
    (remacs-3232-check 'escaped-dot-is-explicit
                       (remacs-3232-match "\\.*" ".hidden"))
    (remacs-3232-check 'bracket-dot-is-not-explicit
                       (not (remacs-3232-match "[.]hidden" ".hidden")))
    (remacs-3232-check 'star-dot-star-hides-leading-dot
                       (not (remacs-3232-match "*.*" ".hidden")))
    (remacs-3232-check 'star-dot-star-still-matches
                       (remacs-3232-match "*.*" "a.hidden"))
    (remacs-3232-check 'zero-star-branch-preserved
                       (remacs-3232-match "*foo" "foo"))
    (remacs-3232-check 'nonempty-star-branch-preserved
                       (remacs-3232-match "*foo" "xfoo"))
    (remacs-3232-check 'leading-star-dot-still-hidden
                       (not (remacs-3232-match "*foo" ".foo")))
    (remacs-3232-check 'leading-star-question-visible
                       (remacs-3232-match "*?*" "ab"))
    (remacs-3232-check 'leading-star-question-hides-dot
                       (not (remacs-3232-match "*?*" ".ab")))
    (remacs-3232-check 'later-dot-still-matches
                       (remacs-3232-match "*?*" "a.b"))
    (remacs-3232-check 'multiple-star-hides-dot
                       (not (remacs-3232-match "**" ".hidden")))
    (remacs-3232-check 'range-class-hides-dot
                       (not (remacs-3232-match "[.-z]" ".")))
    (remacs-3232-check 'range-class-hides-slash
                       (not (remacs-3232-match "[.-z]" "/")))
    (remacs-3232-check 'range-class-keeps-ordinary-members
                       (and (remacs-3232-match "[.-z]" "0")
                            (remacs-3232-match "[.-z]" "a")))
    (remacs-3232-check 'positive-posix-class-hides-dot
                       (not (remacs-3232-match "[[:punct:]]" ".")))
    (remacs-3232-check 'positive-posix-class-hides-slash
                       (not (remacs-3232-match "[[:punct:]]" "/")))
    (remacs-3232-check 'positive-posix-class-keeps-punctuation
                       (remacs-3232-match "[[:punct:]]" "!"))
    (remacs-3232-check 'negative-class-hides-dot
                       (not (remacs-3232-match "[!a]" ".")))
    (remacs-3232-check 'negative-class-hides-slash
                       (not (remacs-3232-match "[!a]" "/")))
    (remacs-3232-check 'negative-class-keeps-other-members
                       (remacs-3232-match "[!a]" "b"))
    (remacs-3232-check 'explicit-directory-component
                       (and (remacs-3232-match "./[!a]" "./b")
                            (not (remacs-3232-match "./[!a]" "./.hidden"))))
    (remacs-3232-check 'slash-resets-component-state
                       (and (remacs-3232-match "a/[!a]" "a/b")
                            (not (remacs-3232-match "a/[!a]" "a/.hidden"))))
    (remacs-3232-check 'explicit-dot-after-slash
                       (and (remacs-3232-match "./.*" "./.hidden")
                            (remacs-3232-match "a/.*" "a/.hidden")))
    (remacs-3232-check 'slash-in-class-falls-back-to-literal-components
                       (and (remacs-3232-match "a[/.]b" "a[/.]b")
                            (not (remacs-3232-match "a[/.]b" "a.b"))))
    (remacs-3232-check 'escaped-slash-in-class-still-separates-components
                       (and (remacs-3232-match "a[\\/.]b" "a[/.]b")
                            (not (remacs-3232-match "a[\\/.]b" "a.b"))))
    (remacs-3232-check 'range-class-with-slash-member-stays-a-class
                       (and (remacs-3232-match "[.-0]" "0")
                            (not (remacs-3232-match "[.-0]" "/"))
                            (not (remacs-3232-match "[.-0]" "."))))
    ;; Keep the earlier parser fixes working through the new tokenizer.
    (remacs-3232-check 'escaped-star
                       (and (remacs-3232-match "foo\\*" "foo*")
                            (not (remacs-3232-match "foo\\*" "foox"))))
    (remacs-3232-check 'unmatched-open-bracket
                       (remacs-3232-match "foo[bar" "foo[bar"))
    (remacs-3232-check 'leading-close-bracket
                       (and (remacs-3232-match "[]]" "]")
                            (not (remacs-3232-match "[]]" "a"))))
    (remacs-3232-check 'empty-negated-class-is-literal
                       (remacs-3232-match "[!]" "[!]"))
    (remacs-3232-check 'descending-range-is-empty
                       (not (remacs-3232-match "[a-\\]]" "]"))))

  (let ((case-fold-search t))
    (remacs-3232-check 'casefold-still-applies-to-generated-literals
                       (remacs-3232-match "A*" "aTail"))
    (remacs-3232-check 'casefold-does-not-admit-leading-dot
                       (not (remacs-3232-match "*" ".hidden"))))
  (remacs-3232-check 'class-generated-folded-does-not-freeze-folding
                     (let ((regexp (remacs-3232-regexp-with-fold "[A-Z]" t))
                           (case-fold-search nil))
                       (not (string-match-p regexp "a"))))
  (remacs-3232-check 'class-generated-sensitive-folds-at-match-time
                     (let ((regexp (remacs-3232-regexp-with-fold "[A-Z]" nil))
                           (case-fold-search t))
                       (string-match-p regexp "a")))
  (remacs-3232-check 'negated-class-keeps-source-polarity-under-folding
                     (let ((negated-lower
                            (remacs-3232-regexp-with-fold "[!a]" t))
                           (negated-range
                            (remacs-3232-regexp-with-fold "[!A-Z]" t)))
                       (and (not (remacs-3232-regexp-matches-with-fold
                                  negated-lower "a" t))
                            (not (remacs-3232-regexp-matches-with-fold
                                  negated-lower "A" t))
                            (not (remacs-3232-regexp-matches-with-fold
                                  negated-lower "a" nil))
                            (remacs-3232-regexp-matches-with-fold
                             negated-lower "A" nil)
                            (not (remacs-3232-regexp-matches-with-fold
                                  negated-range "a" t))
                            (not (remacs-3232-regexp-matches-with-fold
                                  negated-range "A" t))
                            (remacs-3232-regexp-matches-with-fold
                             negated-range "a" nil))))
  (remacs-3232-check 'case-folding-matrix-preserves-source-class-polarity
                     (remacs-3232-casefold-grid))
  (remacs-3232-check 'wildcard-to-regexp-preserves-stringp-error
                     (condition-case err
                         (progn (wildcard-to-regexp 7) nil)
                       (wrong-type-argument
                        (eq (car err) 'wrong-type-argument))))
  (remacs-3232-check 'invalid-class-error-remains-deferred
                     (let ((regexp (wildcard-to-regexp "[[:bogus:]]")))
                       (condition-case err
                           (progn (string-match-p regexp "x") nil)
                         (invalid-regexp
                          (eq (car err) 'invalid-regexp)))))
  (remacs-3232-check 'wildcard-to-regexp-match-data-side-effect
                     (let ((pattern "a[0]"))
                       (string-match "z" "z")
                       (wildcard-to-regexp pattern)
                       (equal (match-string 0 pattern) "[")))

  (remacs-3232-check 'class-membership-snapshotted-at-call-time
                     (remacs-3232-syntax-class-snapshot))
  (remacs-3232-check 'current-dot-slash-aliases-excluded
                     (remacs-3232-current-dot-slash-aliases-hidden))
  (remacs-3232-check 'negative-class-backslash-hyphen-ascii-set
                     (remacs-3232-negative-class-ascii-membership))
  (remacs-3232-check 'byte8-and-mule-posix-class-ranges-preserved
                     (remacs-3232-high-encoding-punctuation-snapshot))
  (remacs-3232-check 'negative-class-byte8-literal-exclusion
                     (remacs-3232-byte8-negative-class))

  (let* ((byte8 (char-to-string #x3fff81))
         (byte8-regexp (wildcard-to-regexp (concat "[" byte8 "]")))
         (mule (char-to-string #x110001))
         (mule-regexp (wildcard-to-regexp (concat "[" mule "]"))))
    (remacs-3232-check 'byte8-character-survives-range-serialization
                       (and (string-match-p byte8-regexp byte8)
                            (not (string-match-p byte8-regexp "x"))))
    (remacs-3232-check 'mule-character-survives-range-serialization
                       (and (string-match-p mule-regexp mule)
                            (not (string-match-p mule-regexp "x")))))

  (let ((case-fold-search nil)
        (directory (make-temp-file "remacs-wildcard-3232-" t)))
    (unwind-protect
        (progn
          (with-temp-file (expand-file-name ".hidden" directory) (insert ""))
          (with-temp-file (expand-file-name "visible" directory) (insert ""))
          (with-temp-file (expand-file-name "a.hidden" directory) (insert ""))
          (with-temp-file (expand-file-name "visible.txt" directory) (insert ""))
          (with-temp-file (expand-file-name "!punct" directory) (insert ""))
          (make-directory (expand-file-name "q" directory))
          (make-directory (expand-file-name (concat "q" (string 92)) directory))
          (with-temp-file (expand-file-name "q/leaf-plain" directory) (insert ""))
          (with-temp-file (expand-file-name (concat "q" (string 92) "/leaf-quoted") directory)
            (insert ""))
          (with-temp-file (expand-file-name "a.b" directory) (insert ""))
          (let ((slash-directory (expand-file-name "a[" directory)))
            (make-directory slash-directory)
            (with-temp-file (expand-file-name ".]b" slash-directory)
              (insert "")))
          (let ((star (remacs-3232-file-names directory "*"))
                (star-dot (remacs-3232-file-names directory "*.*"))
                (punct (remacs-3232-file-names directory "[[:punct:]]*"))
                (slash-class (remacs-3232-file-names directory "a[/.]b"))
                (escaped-slash-class
                 (remacs-3232-file-names directory "a[\\/.]b"))
                (quoted-absolute
                 (file-expand-wildcards
                  (concat "\\" (expand-file-name "a[/.]b" directory)) t))
                (expected-child (expand-file-name ".]b"
                                                  (expand-file-name "a[" directory)))
                (regexp (remacs-3232-file-names directory ".*" t)))
            (remacs-3232-check 'file-expand-star
                               (and (member "visible" star)
                                    (member "a.hidden" star)
                                    (not (member ".hidden" star))))
            (remacs-3232-check 'file-expand-star-dot
                               (and (member "a.hidden" star-dot)
                                    (member "visible.txt" star-dot)
                                    (not (member ".hidden" star-dot))))
            (remacs-3232-check 'file-expand-posix-class
                               (and (member "!punct" punct)
                                    (not (member ".hidden" punct))))
            (remacs-3232-check 'slash-class-expands-physical-components
                               (and (member ".]b" slash-class)
                                    (not (member "a.b" slash-class))))
            (remacs-3232-check 'escaped-slash-class-expands-physical-components
                               (and (member ".]b" escaped-slash-class)
                                    (not (member "a.b" escaped-slash-class))))
            (remacs-3232-check 'quoted-leading-slash-remains-absolute
                               (member expected-child quoted-absolute))
            (dolist (count '(1 2 3))
              (remacs-3232-check
               'literal-backslash-directory-preserves-quote-parity
               (equal (remacs-3232-file-names
                       directory (concat "q" (make-string count 92) "/leaf*"))
                      (if (= count 1) '("leaf-plain") '("leaf-quoted")))))
            (remacs-3232-check 'regexp-mode-bypasses-glob-dot-rule
                               (member ".hidden" regexp))))
      (delete-directory directory t)))
  t)"###;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn check(interp: &mut Interp, source: &str) {
    let result = interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    assert!(result.truthy());
}

#[test]
fn wildcard_globs_apply_leading_dot_rules_to_direct_and_component_matches() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/files.el")
        .canonicalize()
        .unwrap();
    let code = format!(
        "(progn (setq noninteractive t) (load {} nil t t) {})",
        lisp_string(source.to_str().unwrap()),
        CASES
    );
    let mut interp = Interp::new();
    check(&mut interp, CASES);
    check(&mut interp, &code);
}
