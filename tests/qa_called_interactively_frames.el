;;; qa_called_interactively_frames.el --- Inspect the containing call and KIND -*- lexical-binding: t; -*-

(defun qa336-probe (&optional kind) (called-interactively-p kind))

(defun qa336-middle () (qa336-probe 'any))

;; Explicit batch context; individual checks can override it dynamically.
(let ((noninteractive t))
  ;; direct kind 'any
  (qa (funcall-interactively #'qa336-probe 'any)
      't)
  
  ;; direct kind 'interactive
  (qa (funcall-interactively #'qa336-probe 'interactive)
      'nil)
  
  ;; direct kind nil
  (qa (funcall-interactively #'qa336-probe nil)
      't)
  
  ;; direct kind 'other
  (qa (funcall-interactively #'qa336-probe 'other)
      't)
  
  ;; direct kind 42
  (qa (funcall-interactively #'qa336-probe 42)
      't)
  
  ;; interactive kind gating noninteractive=nil macro=nil
  (qa (let ((noninteractive nil) (executing-kbd-macro nil)) (funcall-interactively #'qa336-probe 'interactive))
      't)
  
  ;; interactive kind gating noninteractive=t macro=nil
  (qa (let ((noninteractive t) (executing-kbd-macro nil)) (funcall-interactively #'qa336-probe 'interactive))
      'nil)
  
  ;; interactive kind gating noninteractive=nil macro=t
  (qa (let ((noninteractive nil) (executing-kbd-macro t)) (funcall-interactively #'qa336-probe 'interactive))
      'nil)
  
  ;; funcall helper
  (qa (funcall-interactively (lambda () (funcall #'qa336-probe 'any)))
      'nil)
  
  ;; apply helper
  (qa (funcall-interactively (lambda () (apply #'qa336-probe '(any))))
      'nil)
  
  ;; named helper
  (qa (funcall-interactively (lambda () (qa336-probe 'any)))
      'nil)
  
  ;; bytecompiled helper
  (qa (let ((fn (byte-compile (lambda () (called-interactively-p 'any))))) (funcall-interactively (lambda () (funcall fn))))
      'nil)
  
  ;; direct bytecompiled target
  (qa (funcall-interactively (byte-compile (lambda () (called-interactively-p 'any))))
      't)
  
  ;; ordinary funcall is not interactive
  (qa (funcall #'qa336-probe 'any)
      'nil)
  
  ;; checker itself called as target
  (qa (funcall-interactively #'called-interactively-p 'any)
      'nil)
  
  ;; checker called through funcall in target
  (qa (funcall-interactively (lambda () (funcall #'called-interactively-p 'any)))
      'nil)
  
  ;; checker called through apply in target
  (qa (funcall-interactively (lambda () (apply #'called-interactively-p '(any))))
      'nil)
  
  ;; primitive eval target is not a containing command
  (qa (funcall-interactively #'eval '(called-interactively-p 'any))
      'nil)
  
  ;; special forms do not hide the containing function
  (qa (funcall-interactively (lambda () (let ((flag t)) (if flag (called-interactively-p 'any)))))
      't)
  
  ;; nested interactive calls restore outer classification
  (qa (funcall-interactively (lambda () (list (funcall-interactively #'qa336-probe 'any) (called-interactively-p 'any))))
      '(t t))
  
  ;; call-interactively batch
  (qa (call-interactively (lambda () (interactive) (list (called-interactively-p 'any) (called-interactively-p 'interactive))))
      '(t nil))
  
  ;; command-execute batch
  (qa (command-execute (lambda () (interactive) (list (called-interactively-p 'any) (called-interactively-p 'interactive))))
      '(t nil))
  
  ;; obsolete interactive-p batch
  (qa (funcall-interactively (lambda () (interactive-p)))
      'nil)
  
  ;; obsolete interactive-p with interactive input
  (qa (let ((noninteractive nil) (executing-kbd-macro nil)) (funcall-interactively (lambda () (interactive-p))))
      't)
  
  ;; primitive dispatcher object
  (qa (let ((dispatcher (symbol-function 'funcall-interactively))) (funcall dispatcher (lambda () (called-interactively-p 'any))))
      't)
  
  ;; call-interactively honors dispatcher rebinding
  (qa (let ((old (symbol-function 'funcall-interactively))) (unwind-protect (progn (fset 'funcall-interactively (lambda (&rest _) 'wrong)) (call-interactively (lambda () (interactive) (called-interactively-p 'any)))) (fset 'funcall-interactively old)))
      'wrong)
  
  ;; error unwinds interactive state
  (qa (progn (condition-case nil (funcall-interactively (lambda () (error "stop"))) (error nil)) (called-interactively-p 'any))
      'nil)
  
  ;; throw unwinds interactive state
  (qa (progn (catch 'qa336-exit (funcall-interactively (lambda () (throw 'qa336-exit t)))) (called-interactively-p 'any))
      'nil)
  
  ;; internal dispatcher identity matters
  (qa (let ((internal--funcall-interactively #'ignore)) (funcall-interactively #'qa336-probe 'any))
      'nil)
  
  
  ;; Package hooks can identify wrapper frames to skip.
  (qa (let ((called-interactively-p-functions
             (list (lambda (_index frame _next)
                     (and (eq (nth 1 frame) 'qa336-probe) 0)))))
        (funcall-interactively (lambda () (qa336-probe 'any))))
      t)
  (qa (let ((called-interactively-p-functions
             (list (lambda (_index frame _next)
                     (and (eq (nth 1 frame) 'qa336-probe) 1)))))
        (funcall-interactively (lambda () (qa336-middle))))
      t)
  
  (princ (qa--done))
)
