;;; -*- lexical-binding: t; -*-
;; Issue #244: retain canonical buffer identity after kill and through GC,
;; independently of live-buffer lookup and mutable contact plists.
(defun qa244-create (kind buffer)
  (cond ((eq kind 'pipe) (make-pipe-process :name "qa244-pipe" :buffer buffer :sentinel #'ignore :noquery t))
        ((eq kind 'child) (make-process :name "qa244-child" :buffer buffer :command '("cat") :sentinel #'ignore :noquery t))
        ((eq kind 'network) (make-network-process :name "qa244-server" :buffer buffer :host "127.0.0.1" :service t :server t :sentinel #'ignore :noquery t))))
(defun qa244-dead-buffer (kind by-name)
  (let* ((buffer (generate-new-buffer " *qa244-buffer*"))
         (name (buffer-name buffer))
         (process (qa244-create kind (if by-name name buffer))))
    (unwind-protect
        (progn
          (kill-buffer buffer) (garbage-collect)
          (let* ((first (process-buffer process))
                 (replacement (generate-new-buffer name))
                 (before (list (bufferp first) (buffer-live-p first) (eq first buffer)
                               (eq first (process-buffer process)) (buffer-name first)
                               (marker-buffer (process-mark process)))))
            (unwind-protect
                (progn (delete-process process) (garbage-collect)
                       (list before (eq (process-buffer process) buffer)
                             (eq (process-buffer process) replacement) (buffer-live-p replacement)))
              (kill-buffer replacement))))
      (ignore-errors (delete-process process))
      (when (buffer-live-p buffer) (kill-buffer buffer)))))

(qa (qa244-dead-buffer 'pipe nil) '((t nil t t nil nil) t nil t))

(qa (qa244-dead-buffer 'pipe t) '((t nil t t nil nil) t nil t))

(qa (qa244-dead-buffer 'child nil) '((t nil t t nil nil) t nil t))

(qa (qa244-dead-buffer 'child t) '((t nil t t nil nil) t nil t))

(qa (qa244-dead-buffer 'network nil) '((t nil t t nil nil) t nil t))

(qa (qa244-dead-buffer 'network t) '((t nil t t nil nil) t nil t))

(qa
 (let*
     ((b (generate-new-buffer " *qa244-original*"))
      (p
       (make-pipe-process :name "qa244-original" :buffer b :noquery t)))
   (unwind-protect
       (progn
	 (kill-buffer b)
	 (list (buffer-live-p b) (eq (process-buffer p) b)))
     (ignore-errors (delete-process p))
     (when (buffer-live-p b) (kill-buffer b))))
 '(nil t))

(qa
 (let*
     ((b (generate-new-buffer " *qa244-late*"))
      (p (qa244-create 'pipe nil)) (m (process-mark p)))
   (unwind-protect
       (progn
	 (kill-buffer b) (set-process-buffer p b) (garbage-collect)
	 (let
	     ((before
	       (list (eq (process-buffer p) b) (marker-buffer m)
		     (marker-position m)
		     (eq (process-contact p :buffer) b))))
	   (set-process-buffer p nil)
	   (list before (process-buffer p) (process-contact p :buffer)
		 (marker-buffer m) (eq m (process-mark p)))))
     (ignore-errors (delete-process p))))
 '((t nil nil t) nil nil nil t))

(qa
 (let*
     ((b (generate-new-buffer " *qa244-switch*"))
      (next (generate-new-buffer " *qa244-next*"))
      (p (qa244-create 'pipe b)))
   (unwind-protect
       (progn
	 (kill-buffer b) (set-process-buffer p next)
	 (list (eq (process-buffer p) next)
	       (eq (marker-buffer (process-mark p)) next)
	       (eq (process-contact p :buffer) next)))
     (ignore-errors (delete-process p)) (kill-buffer next)))
 '(t t t))

(qa
 (let*
     ((b (generate-new-buffer " *qa244-release*"))
      (table (make-hash-table :test 'eq :weakness 'key))
      (p (qa244-create 'pipe nil)))
   (unwind-protect
       (progn
	 (puthash b 'held table) (set-process-buffer p b)
	 (kill-buffer b) (setq b nil) (garbage-collect)
	 (let
	     ((held
	       (list (hash-table-count table)
		     (bufferp (process-buffer p)))))
	   (set-process-buffer p nil) (garbage-collect)
	   (list held (process-buffer p) (process-contact p :buffer))))
     (ignore-errors (delete-process p))))
 '((1 t) nil nil))

(qa
 (let*
     ((b (generate-new-buffer " *qa244-contact-release*"))
      (table (make-hash-table :test 'eq :weakness 'key))
      (p (qa244-create 'pipe b)))
   (unwind-protect
       (progn
	 (puthash b 'held table) (kill-buffer b) (setq b nil)
	 (garbage-collect)
	 (let ((held (hash-table-count table)))
	   (set-process-buffer p nil) (garbage-collect)
	   (list held (process-buffer p) (process-contact p :buffer))))
     (ignore-errors (delete-process p))))
 '(1 nil nil))

(qa
 (let*
     ((b (generate-new-buffer " *qa244-child-root*"))
      (table (make-hash-table :test 'eq :weakness 'key))
      (p (qa244-create 'child nil)))
   (unwind-protect
       (progn
	 (set-process-buffer p b) (puthash b 'held table)
	 (kill-buffer b) (setq b nil) (garbage-collect)
	 (list (hash-table-count table) (bufferp (process-buffer p))
	       (buffer-live-p (process-buffer p))))
     (ignore-errors (delete-process p))))
 '(1 t nil))

(qa
 (let*
     ((b (generate-new-buffer " *qa244-canonical*"))
      (other (generate-new-buffer " *qa244-contact-only*"))
      (p (qa244-create 'pipe b)))
   (unwind-protect
       (let ((contact (process-contact p t)))
	 (plist-put contact :buffer other)
	 (let
	     ((separate
	       (list (eq (process-buffer p) b)
		     (eq (process-contact p :buffer) other))))
	   (set-process-buffer p b) (kill-buffer b) (garbage-collect)
	   (list separate (eq (process-buffer p) b)
		 (eq (process-contact p :buffer) b))))
     (ignore-errors (delete-process p))
     (when (buffer-live-p b) (kill-buffer b)) (kill-buffer other)))
 '((t t) t t))


(princ (qa--done)) (terpri)
