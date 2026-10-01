;; Dump xfail/xpass/fail details after loading a qa file.
;; Usage: remacs --batch -l qa_framework.el -l qa_run_detail.el -l qa_FILE.el
;; Actually load order: framework, then this defines nothing extra; the file
;; under test ends with (princ (qa--done)). Instead we redefine qa--done first.
(defun qa--done ()
  (dolist (x (reverse qa--xpass))
    (princ (format "XPASS %S\n" x)))
  (dolist (x (reverse qa--xfails))
    (princ (format "XFAIL %S\n" x)))
  (dolist (x (reverse qa--fails))
    (princ (format "FAIL %S\n" x)))
  (format "%d checks (%d xfail %d xpass %d fail)"
          qa--count (length qa--xfails) (length qa--xpass) (length qa--fails)))
