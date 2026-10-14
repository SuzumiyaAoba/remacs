;;; -*- lexical-binding: nil; -*-
;; Issue #102: an unchanged ambiguous prefix is a string, not t.
(let ((qa-fcp-dir (make-temp-file "qa-fcp-" t))
      (completion-ignored-extensions nil)
      (completion-ignore-case nil))
  (unwind-protect
      (progn
        (dolist (name '("f1.txt" "f2.el" "exact" "exact-more"
                        "solo" "日本一" "日本二" "αβ1" "αβ2"))
          (with-temp-file (expand-file-name name qa-fcp-dir) (insert "x")))
        (dolist (name '("dir-one" "dir-two" "unique-dir"))
          (make-directory (expand-file-name name qa-fcp-dir)))
        (qa (file-name-completion "f" qa-fcp-dir) "f")
        (qa (file-name-completion "" qa-fcp-dir) "")
        (qa (file-name-completion "f1" qa-fcp-dir) "f1.txt")
        (qa (file-name-completion "f1.txt" qa-fcp-dir) t)
        (qa (file-name-completion "exact" qa-fcp-dir) "exact")
        (qa (file-name-completion "exact-" qa-fcp-dir) "exact-more")
        (qa (file-name-completion "solo" qa-fcp-dir) t)
        (qa (file-name-completion "sol" qa-fcp-dir) "solo")
        (qa (file-name-completion "missing" qa-fcp-dir) nil)
        (qa (file-name-completion "日本" qa-fcp-dir) "日本")
        (qa (file-name-completion "日" qa-fcp-dir) "日本")
        (qa (file-name-completion "日本一" qa-fcp-dir) t)
        (qa (file-name-completion "αβ" qa-fcp-dir) "αβ")
        (qa (file-name-completion "α" qa-fcp-dir) "αβ")
        (qa (file-name-completion "dir" qa-fcp-dir) "dir-")
        (qa (file-name-completion "dir-" qa-fcp-dir) "dir-")
        (qa (file-name-completion "unique" qa-fcp-dir) "unique-dir/")
        (qa (file-name-completion "f" qa-fcp-dir #'file-directory-p) nil)
        (qa (file-name-completion "dir-" qa-fcp-dir #'file-directory-p) "dir-")
        (qa (file-name-completion "exact" qa-fcp-dir
                                  (lambda (name) (equal name "exact"))) t)
        (qa (file-name-completion "f" qa-fcp-dir
                                  (lambda (name) (equal name "f1.txt"))) "f1.txt")
        (qa (file-name-completion "f" qa-fcp-dir (lambda (name) nil)) nil)
        (qa (file-name-completion (propertize "f" 'face 'bold) qa-fcp-dir) "f"))
    (delete-directory qa-fcp-dir t)))
(princ (qa--done))
