;;; -*- lexical-binding: t -*-
(require 'imenu)

(defun qa-imenu--collision-selections (replacement names)
  (let ((original (symbol-function 'completing-read))
        (index (let ((position 0))
                 (mapcar (lambda (name) (cons name (setq position (1+ position))))
                         names)))
        labels positions)
    (unwind-protect
        (with-temp-buffer
          (let ((imenu-space-replacement replacement))
            (dotimes (selected (length names))
              (fset 'completing-read
                    (lambda (_prompt collection &rest _args)
                      (setq labels (all-completions "" collection))
                      ;; Completion frontends need not preserve text properties.
                      (substring-no-properties (nth selected labels))))
              (push (cdr (imenu--completion-buffer index "Pick")) positions)))
          (list (mapcar #'substring-no-properties labels)
                (nreverse positions) (mapcar #'car index)))
      (fset 'completing-read original))))

(qa (qa-imenu--collision-selections "." '("a b" "a.b"))
    '(("a.b" "a.b<2>") (1 2) ("a b" "a.b")))
(qa (qa-imenu--collision-selections "." '("a.b" "a b"))
    '(("a.b" "a.b<2>") (1 2) ("a.b" "a b")))
(qa (qa-imenu--collision-selections "." '("a b" "a.b" "a.b<2>"))
    '(("a.b" "a.b<3>" "a.b<2>") (1 2 3) ("a b" "a.b" "a.b<2>")))
(qa (qa-imenu--collision-selections "."
                                    '("a  b" "a .b" "a..b" "a..b<2>" "a..b<3>"))
    '(("a..b" "a..b<4>" "a..b<5>" "a..b<2>" "a..b<3>") (1 2 3 4 5)
      ("a  b" "a .b" "a..b" "a..b<2>" "a..b<3>")))
(qa (qa-imenu--collision-selections "_x" '("a b" "a_xb" "a  b"))
    '(("a_xb" "a_xb<2>" "a_x_xb") (1 2 3) ("a b" "a_xb" "a  b")))
(qa (qa-imenu--collision-selections "" '("a b" "ab" " a b "))
    '(("ab" "ab<2>" "ab<3>") (1 2 3) ("a b" "ab" " a b ")))
(qa (qa-imenu--collision-selections "" '(" " "  " "<2>"))
    '(("" "<3>" "<2>") (1 2 3) (" " "  " "<2>")))
(qa (qa-imenu--collision-selections "間" '("名 前" "名間前"))
    '(("名間前" "名間前<2>") (1 2) ("名 前" "名間前")))
(qa (qa-imenu--collision-selections nil '("a b" "a.b"))
    '(("a b" "a.b") (1 2) ("a b" "a.b")))

(qa (let ((original (symbol-function 'completing-read))
          (index '(("a b" ("leaf one" . 11)) ("a.b" ("leaf two" . 22))))
          (depth 0))
      (unwind-protect
          (progn
            (fset 'completing-read
                  (lambda (_prompt collection &rest _args)
                    (let ((labels (all-completions "" collection)))
                      (prog1 (substring-no-properties
                              (nth (if (= depth 0) 1 0) labels))
                        (setq depth (1+ depth))))))
            (with-temp-buffer
              (let ((imenu-space-replacement "."))
                (cdr (imenu--completion-buffer index "Pick")))))
        (fset 'completing-read original)))
    22)

(qa (let* ((original (symbol-function 'completing-read))
           (target '("a.b" . 2))
           (index (list '("a b" . 1)
                        (cons (propertize "a.b" 'imenu-choice target
                                          'imenu-section "Section") 2)))
           section result)
      (unwind-protect
          (progn
            (fset 'completing-read
                  (lambda (_prompt collection &rest _args)
                    (let ((label (cadr (all-completions "" collection))))
                      (setq section (get-text-property 0 'imenu-section label))
                      label)))
            (with-temp-buffer
              (let ((imenu-space-replacement ".") (imenu-flatten 'group))
                (setq result (imenu--completion-buffer index "Pick"))))
            (list section (eq result target)))
        (fset 'completing-read original)))
    '("Section" t))

(princ (qa--done))
