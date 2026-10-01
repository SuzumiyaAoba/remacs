;; file-attributes / file-modes / file-newer-than-file-p — GNU 31.1 pins.
;;
;; GNU truths pinned:
;;   - file-attributes returns a 12-slot list.
;;   - slot 0 = nil for regular file, t for dir, target-string for symlink.
;;   - slot 8 = mode string like "-rw-------".
;;   - file-modes returns the octal mode integer.
;;   - file-newer-than-file-p compares mtimes.
;;   - file-name-absolute-p returns t for "~/foo" (shell-style tilde).
;;
;; `qa' evals its form in DYNAMIC scope — locals referenced inside a
;; `qa' form must be defvar'd (special) to be visible to `eval'.
(defvar attrs)
(defvar a)
(defvar f)
(defvar sl)
(defvar f1)
(defvar f2)
(defvar h)
(defvar tn)
(defvar realf)
(defvar an)
(defvar d)

;; == surface ==
(qa (fboundp 'file-attributes) t)
(qa (fboundp 'file-modes) t)
(qa (fboundp 'file-newer-than-file-p) t)
(qa (fboundp 'file-exists-p) t)
(qa (fboundp 'file-directory-p) t)
(qa (fboundp 'file-regular-p) t)
(qa (fboundp 'file-readable-p) t)
(qa (fboundp 'file-writable-p) t)
(qa (fboundp 'file-executable-p) t)
(qa (fboundp 'file-symlink-p) t)
(qa (fboundp 'file-accessible-directory-p) t)
(qa (fboundp 'file-name-absolute-p) t)

;; == file-attributes slot layout ==
(let* ((f (make-temp-file "qatest"))
       (attrs (file-attributes f)))
  (qa (listp attrs) t)
  (qa (length attrs) 12)
  ;; slot 0: t for dir, nil for regular file, string for symlink
  (qa (car attrs) nil)
  ;; slot 1: number of links
  (qa (integerp (nth 1 attrs)) t)
  ;; slot 2: uid
  (qa (integerp (nth 2 attrs)) t)
  ;; slot 3: gid
  (qa (integerp (nth 3 attrs)) t)
  ;; slot 4-6: atime/mtime/ctime (Lisp timestamp)
  (qa (consp (nth 5 attrs)) t)
  ;; slot 7: size
  (qa (integerp (nth 7 attrs)) t)
  ;; slot 8: modes string like "-rw-------"
  (qa (stringp (nth 8 attrs)) t)
  ;; slot 10: inode
  (qa (integerp (nth 10 attrs)) t)
  ;; slot 11: device
  (qa (integerp (nth 11 attrs)) t)
  (delete-file f))

;; == file-attributes on a directory ==
(let ((attrs (file-attributes "/tmp")))
  ;; slot 0 is the SYMLINK TARGET string "/private/tmp" on macOS
  ;; (since /tmp is a symlink); elsewhere t for a regular dir.
  (qa (or (stringp (car attrs)) (eq (car attrs) t)) t))

;; == file-attributes on nonexistent file → nil ==
(qa (file-attributes "/nonexistent-file-xyz-12345") nil)

;; == file-attributes with ID-FORMAT 'integer ==
(let ((f (make-temp-file "qatest")))
  (let ((a (file-attributes f 'integer)))
    (qa (integerp (nth 2 a)) t))
  (delete-file f))

;; == file-modes returns the octal integer ==
(let ((f (make-temp-file "qatest")))
  (qa (integerp (file-modes f)) t)
  (delete-file f))

;; == file-exists-p ==
(let ((f (make-temp-file "qatest")))
  (qa (file-exists-p f) t)
  (delete-file f)
  (qa (file-exists-p f) nil))

;; == file-directory-p ==
(qa (file-directory-p "/tmp") t)
(let ((f (make-temp-file "qatest")))
  (qa (file-directory-p f) nil)
  (delete-file f))
(qa (file-directory-p "/nonexistent-xyz") nil)

;; == file-regular-p ==
(let ((f (make-temp-file "qatest")))
  (qa (file-regular-p f) t)
  (delete-file f))
(qa (file-regular-p "/tmp") nil)
(qa (file-regular-p "/nonexistent-xyz") nil)

;; == file-readable-p / file-writable-p ==
(let ((f (make-temp-file "qatest")))
  (qa (file-readable-p f) t)
  (qa (file-writable-p f) t)
  (delete-file f))

;; == file-symlink-p returns the target ==
(let* ((f (make-temp-file "qatest"))
       (sl (concat f ".lnk")))
  (make-symbolic-link f sl)
  (qa (file-symlink-p sl) f)
  (qa (file-symlink-p f) nil)
  (delete-file sl)
  (delete-file f))

;; == file-name-absolute-p ==
(qa (file-name-absolute-p "/tmp/foo") t)
(qa (file-name-absolute-p "tmp/foo") nil)
(qa (file-name-absolute-p "./foo") nil)
;; ~/foo IS absolute in GNU (shell-style tilde counts)
(qa (file-name-absolute-p "~/foo") t)

;; == file-newer-than-file-p ==
(let ((f1 (make-temp-file "qa1"))
      (f2 (make-temp-file "qa2")))
  (write-region "x" nil f1 nil 'quiet)
  (sleep-for 0.05)
  (write-region "x" nil f2 nil 'quiet)
  (qa (file-newer-than-file-p f2 f1) t)
  (qa (file-newer-than-file-p f1 f2) nil)
  (delete-file f1)
  (delete-file f2))

;; == set-file-modes ==
(let ((f (make-temp-file "qatest")))
  (set-file-modes f #o644)
  (qa (logand (file-modes f) #o777) #o644)
  (set-file-modes f #o600)
  (qa (logand (file-modes f) #o777) #o600)
  (delete-file f))

;; == file-name-directory / file-name-nondirectory ==
(qa (file-name-directory "/tmp/foo/bar.el") "/tmp/foo/")
(qa (file-name-directory "/tmp/foo/") "/tmp/foo/")
(qa (file-name-directory "/tmp/foo") "/tmp/")
(qa (file-name-directory "foo") nil)
(qa (file-name-nondirectory "/tmp/foo/bar.el") "bar.el")
(qa (file-name-nondirectory "/tmp/foo/") "")
(qa (file-name-nondirectory "foo.el") "foo.el")

;; == file-name-extension ==
(qa (file-name-extension "foo.el") "el")
(qa (file-name-extension "foo.tar.gz") "gz")
(qa (file-name-extension "foo") nil)
(qa (file-name-extension "foo.") "")
(qa (file-name-extension ".hidden") nil)
(qa (file-name-extension "foo.el" t) ".el")

;; == file-name-sans-extension ==
(qa (file-name-sans-extension "foo.el") "foo")
(qa (file-name-sans-extension "foo.tar.gz") "foo.tar")
(qa (file-name-sans-extension "foo") "foo")
(qa (file-name-sans-extension "/tmp/foo.el") "/tmp/foo")

;; == file-name-base ==
(qa (file-name-base "/tmp/foo/bar.el") "bar")
(qa (file-name-base "/tmp/foo/") "")
(qa (file-name-base "foo.el") "foo")

;; == expand-file-name ==
(qa (expand-file-name "foo" "/tmp") "/tmp/foo")
(qa (expand-file-name "/abs/path") "/abs/path")
(let ((h (expand-file-name "~/")))
  (qa (file-name-absolute-p h) t))
(qa (expand-file-name "./x" "/tmp") "/tmp/x")
(qa (expand-file-name "../x" "/tmp/a") "/tmp/x")

;; == file-truename — resolves symlinks ==
(let* ((f (make-temp-file "qatest"))
       (sl (concat f ".lnk")))
  (make-symbolic-link f sl)
  (let ((tn (file-truename sl))
        (realf (file-truename f)))
    (qa tn realf))
  (delete-file sl)
  (delete-file f))

;; == substitute-in-file-name ==
(qa (substitute-in-file-name "$HOME/foo") (concat (getenv "HOME") "/foo"))
(qa (substitute-in-file-name "/tmp/foo") "/tmp/foo")
;; undefined var keeps the name without "$"
(qa (string-match-p "UNDEFINED_XYZ_999"
        (substitute-in-file-name "$UNDEFINED_XYZ_999/x")) 1)

;; == abbreviate-file-name returns a string ==
(let ((an (abbreviate-file-name (expand-file-name "~/foo"))))
  (qa (stringp an) t))

;; == directory-file-name / file-name-as-directory ==
(qa (directory-file-name "/tmp/") "/tmp")
(qa (directory-file-name "/tmp") "/tmp")
(qa (file-name-as-directory "/tmp") "/tmp/")
(qa (file-name-as-directory "/tmp/") "/tmp/")

;; == make-temp-file / DIR-FLAG ==
(let ((f (make-temp-file "qa-test")))
  (qa (file-exists-p f) t)
  (qa (file-regular-p f) t)
  (delete-file f))
(let ((d (make-temp-file "qa-dir" t)))
  (qa (file-directory-p d) t)
  (delete-directory d))

(princ (qa--done))
