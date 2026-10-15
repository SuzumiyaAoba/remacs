;;; -*- lexical-binding: nil -*-
;; Issue #54. Only processes created here are deleted; no fixed PID signals.
(defun qa54-create (kind buffer explicit-nil)
 (cond ((eq kind 'start) (start-process "qa54-child" buffer "cat"))
       ((eq kind 'pipe) (make-pipe-process :name "qa54-pipe" :buffer buffer :sentinel nil :noquery t))
       (explicit-nil (make-process :name "qa54-child" :buffer buffer :command '("cat") :sentinel nil :noquery t))
       (t (make-process :name "qa54-child" :buffer buffer :command '("cat") :noquery t))))
(dolist (kind '(make start pipe))
 (dolist (explicit-nil '(nil t))
  (with-temp-buffer
   (insert "current")
   (let ((p (qa54-create kind nil explicit-nil)))
    (unwind-protect
     (let ((m (process-mark p)))
      (qa (process-filter p) 'internal-default-process-filter)
      (qa (process-sentinel p) 'internal-default-process-sentinel)
      (qa (markerp m) t) (qa (marker-buffer m) nil)
      (qa (marker-position m) nil) (qa (marker-insertion-type m) nil)
      (qa-same (process-mark p) m) (qa (process-buffer p) nil)
      (qa (buffer-string) "current")
      (qa (set-process-sentinel p 'ignore) 'ignore)
      (qa (process-sentinel p) 'ignore)
      (qa (set-process-sentinel p nil) 'internal-default-process-sentinel)
      (qa (process-sentinel p) 'internal-default-process-sentinel)
      (internal-default-process-sentinel p "ignored\n")
      (qa (buffer-string) "current")
      (garbage-collect) (qa-same (process-mark p) m))
     (set-process-sentinel p 'ignore) (delete-process p))))))
;; Stable mark through attachment, ordinary edits, detachment, and buffer death.
(dolist (kind '(make start pipe))
 (let ((a (generate-new-buffer "qa54-a")) (b (generate-new-buffer "qa54-b"))
       (p (qa54-create kind nil nil)))
  (unwind-protect
   (let ((m (process-mark p)))
    (with-current-buffer a (insert "abcdef") (narrow-to-region 2 5))
    (with-current-buffer b (insert "xyz"))
    (qa-same (set-process-buffer p a) a)
    (qa-same (marker-buffer m) a) (qa (marker-position m) 5)
    (qa (marker-insertion-type m) nil)
    (with-current-buffer a (goto-char 2) (insert "Q"))
    (qa (marker-position m) 6)
    (set-marker m 3 a)
    (qa-same (set-process-buffer p a) a) (qa (marker-position m) 3)
    (qa (set-process-buffer p nil) nil)
    (qa (process-buffer p) nil) (qa-same (marker-buffer m) a)
    (qa (marker-position m) 3)
    (qa-same (set-process-buffer p a) a) (qa (marker-position m) 3)
    (set-marker-insertion-type m t)
    (qa-same (set-process-buffer p b) b)
    (qa-same (marker-buffer m) b) (qa (marker-position m) 4)
    (qa (marker-insertion-type m) t)
    (qa-same (process-mark p) m)
    (set-process-buffer p nil)
    (kill-buffer b)
    (qa (marker-buffer m) nil) (qa (marker-position m) nil)
    (qa (marker-insertion-type m) t)
    (qa-same (set-process-buffer p b) b)
    (qa-xf (eq (process-buffer p) b) t "preexisting killed process-buffer identity")
    (qa (marker-buffer (process-mark p)) nil)
    (qa-err (set-process-buffer p "qa54-a") 'wrong-type-argument))
   (set-process-sentinel p 'ignore) (delete-process p)
   (when (buffer-live-p a) (kill-buffer a))
   (when (buffer-live-p b) (kill-buffer b)))))
;; Constructor attachment uses accessible point-max and registers the mark.
(dolist (kind '(make start pipe))
 (with-temp-buffer
  (insert "abcdef") (narrow-to-region 2 5)
  (let ((p (qa54-create kind (current-buffer) nil)))
   (unwind-protect
    (let ((m (process-mark p)))
     (qa-same (marker-buffer m) (current-buffer))
     (qa (marker-position m) 5) (qa (marker-insertion-type m) nil)
     (goto-char 2) (insert "XY") (qa (marker-position m) 7)
     (delete-region 2 4) (qa (marker-position m) 5))
    (set-process-sentinel p 'ignore) (delete-process p)))))
;; Exercise actual default sentinel insertion, restoring point/read-only/current.
(dolist (detached '(nil t))
 (dolist (point '(1 3 5))
  (with-temp-buffer
   (insert "abcd") (goto-char point)
   (let* ((origin (current-buffer))
          (p (make-pipe-process :name "qa54-event" :buffer origin :sentinel 'ignore :noquery t))
          (m (process-mark p)))
    (delete-process p)
    (set-marker m (if detached nil 3) origin)
    (setq buffer-read-only t)
    (qa (internal-default-process-sentinel p "done\n") nil)
    (qa (buffer-string) (if detached "abcd\nProcess qa54-event done\n" "ab\nProcess qa54-event done\ncd"))
    (qa (point) (if (>= point (if detached 5 3)) (+ point 25) point))
    (qa buffer-read-only t)
    (qa-same (current-buffer) origin)
    (qa-same (marker-buffer m) origin)
    (qa (marker-position m) (+ (if detached 5 3) 25))
    (qa-same (process-mark p) m)))))
;; Running processes' default sentinel is silent even with a buffer.
(with-temp-buffer
 (let ((p (make-pipe-process :name "qa54-running" :buffer (current-buffer) :noquery t)))
  (unwind-protect
   (progn (qa (internal-default-process-sentinel p "anything\n") nil)
          (qa (buffer-string) ""))
   (set-process-sentinel p 'ignore) (delete-process p))))
;; A real child exit invokes the installed default exactly once.
(with-temp-buffer
 (let ((p (make-process :name "qa54-exit" :buffer (current-buffer)
                        :command '("sh" "-c" "printf hi") :noquery t)))
  (unwind-protect
   (progn
    (dotimes (_ 40)
     (when (process-live-p p) (accept-process-output p 0.05)))
    (qa (process-status p) 'exit)
    (qa (buffer-string) "hi\nProcess qa54-exit finished\n")
    (qa (marker-position (process-mark p)) (point-max))
    (let ((text (buffer-string)))
     (delete-process p)
     (qa (buffer-string) text)))
   (set-process-sentinel p 'ignore) (delete-process p))))
;; A sentinel restores the caller's current buffer after writing elsewhere.
(with-temp-buffer
 (let* ((caller (current-buffer)) (dest (generate-new-buffer "qa54-dest"))
        (p (make-pipe-process :name "qa54-other" :buffer dest :sentinel 'ignore :noquery t)))
  (unwind-protect
   (progn
    (delete-process p)
    (qa (internal-default-process-sentinel p "done\n") nil)
    (qa-same (current-buffer) caller)
    (qa (buffer-string) "")
    (qa (with-current-buffer dest (buffer-string)) "\nProcess qa54-other done\n"))
   (set-process-sentinel p 'ignore) (delete-process p) (kill-buffer dest))))
;; Killing an old registration owner must not detach a moved process marker.
(dolist (manual '(nil t))
 (let* ((a (generate-new-buffer "qa54-old")) (b (generate-new-buffer "qa54-new"))
        (p (make-pipe-process :name "qa54-moved" :buffer a :sentinel 'ignore :noquery t))
        (m (process-mark p)))
  (unwind-protect
   (progn
    (with-current-buffer b (insert "abc"))
    (if manual (set-marker m 2 b) (set-process-buffer p b))
    (kill-buffer a)
    (qa-same (marker-buffer m) b)
    (qa (marker-position m) (if manual 2 4))
    (with-current-buffer b (goto-char 1) (insert "X"))
    (qa (marker-position m) (if manual 3 5)))
   (delete-process p)
   (when (buffer-live-p a) (kill-buffer a))
   (when (buffer-live-p b) (kill-buffer b)))))
;; Default sentinel's temporary read-only bypass does not run watchers.
(with-temp-buffer
 (let ((p (make-pipe-process :name "qa54-watch" :buffer (current-buffer) :sentinel 'ignore :noquery t))
       (events nil)
       (watcher (lambda (&rest args) (push args events))))
  (delete-process p) (setq buffer-read-only t)
  (unwind-protect
   (progn
    (add-variable-watcher 'buffer-read-only watcher)
    (qa (internal-default-process-sentinel p "done\n") nil)
    (qa events nil) (qa buffer-read-only t)
    (qa (buffer-string) "\nProcess qa54-watch done\n"))
   (remove-variable-watcher 'buffer-read-only watcher))))
;; Listener status is not GNU's internal run status.
(with-temp-buffer
 (let ((p (make-network-process :name "qa54-listener" :server t :host "127.0.0.1"
                                :service 0 :buffer (current-buffer) :sentinel 'ignore :noquery t)))
  (unwind-protect
   (progn
    (qa (internal-default-process-sentinel p "listen-test\n") nil)
    (qa (buffer-string) "\nProcess qa54-listener listen-test\n"))
   (delete-process p))))
(princ (qa--done)) (terpri)
