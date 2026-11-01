use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r##"(progn
  (require 'cl-lib)
  (defun remacs-proced-2668-assert (condition message)
    (unless condition (error "%s" message)))
  (defun remacs-proced-2668-pids (processes)
    (mapcar #'car processes))
  (defun remacs-proced-2668-rank-lessp (left right)
    (let ((left-rank (% left 10))
          (right-rank (% right 10)))
      (cond ((= left-rank right-rank) 'equal)
            ((< left-rank right-rank) t)
            (t nil))))
  ;; Reproduction via the public sort API.  The pcpu grammar's normal
  ;; secondary key is PID, so equal/missing pcpu values must reach that key.
  (let ((processes
         '((2 . ((pcpu) (pid . 2)))
           (1 . ((pcpu) (pid . 1))))))
    (remacs-proced-2668-assert
     (equal (remacs-proced-2668-pids
             (proced-sort (copy-tree processes) 'pcpu nil))
            '(1 2))
     "both missing primary values did not fall through to the pid key")
    (remacs-proced-2668-assert
     (equal (remacs-proced-2668-pids
             (proced-sort (copy-tree processes) 'pcpu t))
            '(1 2))
     "descending primary mode changed the missing-value tie breaker"))
  ;; Existing numeric primary ordering and its reverse direction remain intact.
  (let ((processes
         '((2 . ((pcpu . 2) (pid . 2)))
           (1 . ((pcpu . 1) (pid . 1))))))
    (remacs-proced-2668-assert
     (equal (remacs-proced-2668-pids
             (proced-sort (copy-tree processes) 'pcpu nil))
            '(1 2))
     "ascending numeric primary sort changed")
    (remacs-proced-2668-assert
     (equal (remacs-proced-2668-pids
             (proced-sort (copy-tree processes) 'pcpu t))
            '(2 1))
     "descending numeric primary sort changed"))
  ;; A one-sided nil still stops at the missing key, like the existing
  ;; single-key predicate; it must not use PID as an accidental tie breaker.
  (let ((missing '(1 . ((pcpu) (pid . 1))))
        (present '(2 . ((pcpu . 9) (pid . 2)))))
    ;; A one-sided missing primary value must stop comparison with nil,
    ;; matching the single-key fast path in both directions and both orders.
    (dolist (reverse '(nil t))
      (remacs-proced-2668-assert
       (let ((proced-sort-internal
              (list (list 'pcpu 'proced-< reverse))))
         (and (null (proced-sort-p missing present))
              (null (proced-sort-p present missing))))
       "single-key missing-value policy changed")
      (remacs-proced-2668-assert
       (let ((proced-sort-internal
              (list (list 'pcpu 'proced-< reverse)
                    '(pid proced-< nil))))
         (and (null (proced-sort-p missing present))
              (null (proced-sort-p present missing))))
       "one-sided nil continued to a later key or applied reverse")))
  ;; String and custom predicates remain usable after a both-nil primary key.
  (let* ((proced-grammar-alist
          (append '((label "Label" nil left proced-string-lessp nil nil nil)
                    (rank "Rank" nil left remacs-proced-2668-rank-lessp nil nil nil))
                  proced-grammar-alist))
         (strings
          '((2 . ((pcpu) (label . "zeta") (pid . 2)))
            (1 . ((pcpu) (label . "alpha") (pid . 1))))))
    (remacs-proced-2668-assert
     (equal (remacs-proced-2668-pids
             (proced-sort (copy-tree strings) '(pcpu label) nil))
            '(1 2))
     "string secondary sort stopped at a both-nil primary key")
    (let ((ranked
           '((11 . ((pcpu) (rank . 12) (pid . 11)))
             (22 . ((pcpu) (rank . 21) (pid . 22))))))
      (remacs-proced-2668-assert
       (equal (remacs-proced-2668-pids
               (proced-sort (copy-tree ranked) '(pcpu rank pid) nil))
              '(22 11))
       "custom predicate did not sort after a both-nil key"))
    ;; All keys equal (including custom predicate's 'equal result) is a tie.
    (remacs-proced-2668-assert
     (let ((proced-sort-internal
            '((pcpu proced-< nil)
              (rank remacs-proced-2668-rank-lessp nil)))
           (p1 '(1 . ((pcpu) (rank . 21) (pid . 1))))
           (p2 '(2 . ((pcpu) (rank . 31) (pid . 2)))))
       (and (null (proced-sort-p p1 p2))
            (null (proced-sort-p p2 p1))))
     "all-equal keys stopped being a comparator tie"))
  t)"##;

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
fn proced_multi_key_sort_continues_when_both_values_are_missing() {
    let root = PathBuf::from(env!("REMACS_SOURCE_ROOT"));
    let mut interp = Interp::new();
    let path = root.join("lisp/proced.el");
    eval(
        &mut interp,
        &format!("(load {} nil t)", lisp_string(path.to_str().unwrap())),
    );
    assert_eq!(eval(&mut interp, CASES), "t");
}
