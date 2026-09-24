;; uniquify buffer-name uniquification, GNU-compatible.
;; Self-asserting: any mismatch signals `error'.

;; ---------- startup/dump availability (GNU: dumped feature) ----------
(cl-assert (featurep 'uniquify))
(cl-assert (fboundp 'uniquify-rationalize-file-buffer-names))
(cl-assert (fboundp 'uniquify--rename-buffer-advice))
(cl-assert (fboundp 'uniquify--create-file-buffer-advice))
(cl-assert (fboundp 'uniquify-maybe-rerationalize-w/o-cb))
(cl-assert (fboundp 'uniquify-get-unique-names))
(cl-assert (boundp 'uniquify-buffer-name-style))
(cl-assert (special-variable-p 'uniquify-buffer-name-style))
(cl-assert (eq uniquify-buffer-name-style 'post-forward-angle-brackets))
;; uniquify-managed is automatically buffer-local (defvar-local) and
;; survives kill-all-local-variables (permanent-local).
(cl-assert (local-variable-if-set-p 'uniquify-managed))
(cl-assert (get 'uniquify-managed 'permanent-local))

;; ---------- uniquify-item record ----------
(let ((it (uniquify-make-item "base" "/d" (current-buffer) nil)))
  (cl-assert (uniquify-item-p it))
  (cl-assert (equal (uniquify-item-base it) "base"))
  (cl-assert (equal (uniquify-item-dirname it) "/d"))
  (cl-assert (eq (uniquify-item-buffer it) (current-buffer)))
  (cl-assert (null (uniquify-item-proposed it)))
  (setf (uniquify-item-proposed it) "prop")
  (cl-assert (equal (uniquify-item-proposed it) "prop"))
  (setf (uniquify-item-dirname it) "/e")
  (cl-assert (equal (uniquify-item-dirname it) "/e")))

;; ---------- uniquify-get-proposed-name: all styles ----------
(let ((dir "/tmp/probe-uq-x"))
  (dolist (spec '((post-forward-angle-brackets . "base<probe-uq-x>")
                  (post-forward . "base|probe-uq-x")
                  (forward . "probe-uq-x/base")
                  (reverse . "base\\probe-uq-x")))
    (let ((uniquify-buffer-name-style (car spec)))
      (cl-assert (equal (uniquify-get-proposed-name "base" dir 1)
                        (cdr spec)))))
  ;; depth 0 → bare base; extra components when depth > 1.  When the
  ;; walk reaches the directory just below root, the leading "/" is
  ;; included (GNU-verified: "/tmp/probe-uq-x/base").
  (cl-assert (equal (uniquify-get-proposed-name "base" dir 0) "base"))
  (let ((uniquify-buffer-name-style 'forward))
    (cl-assert (equal (uniquify-get-proposed-name "base" dir 2)
                      "/tmp/probe-uq-x/base"))))

;; ---------- file-visiting buffer lifecycle ----------
(progn
  (dolist (d '("/tmp/probe-uq-a" "/tmp/probe-uq-b"))
    (make-directory d t)
    (write-region "x" nil (concat d "/name.txt") nil 'silent))
  (unwind-protect
      (let ((uniquify-buffer-name-style 'post-forward-angle-brackets))
        ;; First visit: plain basename.
        (find-file "/tmp/probe-uq-a/name.txt")
        (let ((b1 (current-buffer)))
          (cl-assert (equal (buffer-name b1) "name.txt"))
          ;; Second visit to same basename: both uniquified.
          (find-file "/tmp/probe-uq-b/name.txt")
          (let ((b2 (current-buffer)))
            (cl-assert (equal (buffer-name b1) "name.txt<probe-uq-a>"))
            (cl-assert (equal (buffer-name b2) "name.txt<probe-uq-b>"))
            ;; Managed fix-list on both buffers.
            (cl-assert (consp (buffer-local-value 'uniquify-managed b1)))
            (cl-assert (consp (buffer-local-value 'uniquify-managed b2)))
            (cl-assert (equal (buffer-local-value 'uniquify-managed b1)
                              (buffer-local-value 'uniquify-managed b2)))
            ;; kill-buffer rerationalizes: b1 returns to basename.
            (kill-buffer b2)
            (cl-assert (equal (buffer-name b1) "name.txt")))))
    ;; Cleanup: kill any probe buffers left over.
    (dolist (b (buffer-list))
      (let ((fn (buffer-file-name b)))
        (when (and fn (string-match-p "probe-uq-[ab]" fn))
          (kill-buffer b))))))

;; ---------- rename-buffer advice ----------
(progn
  (find-file "/tmp/probe-uq-a/name.txt")
  (let ((b1 (current-buffer)))
    (find-file "/tmp/probe-uq-b/name.txt")
    (let ((b2 (current-buffer)))
      (unwind-protect
          (progn
            (cl-assert (equal (buffer-name b1) "name.txt<probe-uq-a>"))
            ;; Rename b2 away from the conflict: b1 drops its suffix.
            (with-current-buffer b2 (rename-buffer "probe-uq-other"))
            (cl-assert (equal (buffer-name b1) "name.txt"))
            (cl-assert (equal (buffer-name b2) "probe-uq-other"))
            ;; b2's managed state was cleared by the advice.
            (cl-assert (null (buffer-local-value 'uniquify-managed b2))))
        (kill-buffer b2)
        (kill-buffer b1)))))

;; ---------- uniquify-get-unique-names (stateless) ----------
(progn
  (find-file "/tmp/probe-uq-a/name.txt")
  (let ((b1 (current-buffer)))
    (find-file "/tmp/probe-uq-b/name.txt")
    (let* ((b2 (current-buffer))
           (names (uniquify-get-unique-names (list b1 b2))))
      (unwind-protect
          (progn
            ;; GNU returns a list of propertized strings, one per buffer.
            (cl-assert (= (length names) 2))
            (cl-assert (equal (nth 0 names) "name.txt<probe-uq-a>"))
            (cl-assert (equal (nth 1 names) "name.txt<probe-uq-b>"))
            ;; uniquify-orig-buffer text property back-reference.
            (cl-assert (eq (get-text-property 0 'uniquify-orig-buffer
                                             (nth 0 names)) b1))
            (cl-assert (eq (get-text-property 0 'uniquify-orig-buffer
                                             (nth 1 names)) b2))
            (cl-assert (not (equal (nth 0 names) (nth 1 names)))))
        (kill-buffer b2)
        (kill-buffer b1)))))

;; ---------- style variants end-to-end ----------
(dolist (spec '((forward . ("probe-uq-a/name.txt" "probe-uq-b/name.txt"))
                (reverse . ("name.txt\\probe-uq-a" "name.txt\\probe-uq-b"))
                (post-forward . ("name.txt|probe-uq-a" "name.txt|probe-uq-b"))
                (post-forward-angle-brackets . ("name.txt<probe-uq-a>"
                                                "name.txt<probe-uq-b>"))))
  (let ((uniquify-buffer-name-style (car spec)))
    (find-file "/tmp/probe-uq-a/name.txt")
    (let ((b1 (current-buffer)))
      (find-file "/tmp/probe-uq-b/name.txt")
      (let ((b2 (current-buffer)))
        (unwind-protect
            (progn
              (cl-assert (equal (buffer-name b1) (nth 0 (cdr spec))))
              (cl-assert (equal (buffer-name b2) (nth 1 (cdr spec)))))
          (kill-buffer b2)
          (kill-buffer b1))))))

;; ---------- non-file buffers are unaffected ----------
(let ((b (generate-new-buffer "probe-uq-scratch")))
  (unwind-protect
      (progn
        (cl-assert (null (buffer-local-value 'uniquify-managed b)))
        (with-current-buffer b (rename-buffer "probe-uq-scratch2"))
        (cl-assert (equal (buffer-name b) "probe-uq-scratch2")))
    (kill-buffer b)))
