;; Backup/auto-save naming — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - find-backup-file-name returns the plain `~' name (list of 1).
;;   - backup-file-name-p returns the START index of the `~' (number)
;;     or nil — a number, NOT t.
;;   - auto-save-file-name-p returns the index of the closing `#'
;;     (0 for "#x#"), NOT t.
;;   - buffer-auto-save-file-name is a VARIABLE (not a fn): nil until
;;     set by auto-save-mode.
;;   - make-auto-save-file-name prepends `#NAME#' inside the SAME dir
;;     as buffer-file-name — "/tmp/#name#" (dir gets the # too).
;;   - auto-save-list-file-name is nil until set; auto-save-list-file-prefix
;;     is "~/.emacs.d/auto-save-list/.saves-" (default).
;;   - backup-directory-alist default is nil.
;;   - backup-enable-predicate is a SYMBOL (function name).

;; == find-backup-file-name ==
(qa (find-backup-file-name "/tmp/qa_backup.txt") '("/tmp/qa_backup.txt~"))
(qa (find-backup-file-name "/tmp/no_such_xyz.txt") '("/tmp/no_such_xyz.txt~"))
;; returns a list of 1 for a file with no prior backups
(qa (length (find-backup-file-name "/tmp/qa_backup.txt")) 1)

;; == backup-file-name-p: returns the index of the `~' OR version ==
;; For plain `~' names → index of `~'. For `.~N~' → index of N.
(qa (backup-file-name-p "/tmp/x~") 6)
(qa (backup-file-name-p "/tmp/x.~1~") 9)
(qa (backup-file-name-p "/tmp/x.~99~") 10)
(qa (backup-file-name-p "/tmp/x") nil)
;; t-style check is `numberp' not `eq t'
(qa (numberp (backup-file-name-p "/tmp/a~")) t)
(qa (auto-save-file-name-p "#only-leading") nil)
;; auto-save-file-name-p matches only the bare "#name#" form
(qa (auto-save-file-name-p "/dir/#file#") nil)

;; == make-auto-save-file-name in a visiting buffer ==
(with-temp-file "/tmp/qab_v.txt" (insert "x"))
(let ((b (find-file-noselect "/tmp/qab_v.txt")))
  (with-current-buffer b
    ;; buffer-auto-save-file-name is a VARIABLE, nil until set
    (qa buffer-auto-save-file-name nil)
    ;; make-auto-save-file-name prepends #name# INSIDE the dir
    (qa (make-auto-save-file-name) "/tmp/#qab_v.txt#"))
  (kill-buffer b))
;; non-file-visiting buffer: "#qa-nonfile#rand#" in default-dir; the
;; auto-save-file-name-p check applies to the basename
(with-temp-buffer
  (rename-buffer "qa-nonfile" t)
  (qa (auto-save-file-name-p
       (file-name-nondirectory (make-auto-save-file-name)))
      0))
;; == backup-directory-alist default ==
(qa backup-directory-alist nil)

;; == backup-enable-predicate: symbol name, not lambda ==
(qa (symbolp backup-enable-predicate) t)

;; == file-exists-p/# helpers stay nil-safe ==
(qa (boundp 'delete-auto-save-files) t)

;; == kill-buffer cleanup already done above ==

(princ (qa--done))
