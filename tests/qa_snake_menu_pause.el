;;; qa_snake_menu_pause.el --- Snake menu pause regression -*- lexical-binding:t -*-

(require 'snake)

(defvar qa-snake-popup-paused nil)
(defvar qa-snake-popup-menu nil)

(defun qa-snake-popup-menu-stub (_position menu)
  (setq qa-snake-popup-paused snake-paused
        qa-snake-popup-menu menu)
  nil)

(qa
 (list (aref (nth 5 snake--menu-def) 0)
       (aref (nth 6 snake--menu-def) 0))
 '("Pause" "Resume"))

(qa
 (with-temp-buffer
   (snake-mode)
   (snake-start-game)
   (let ((original-x-popup-menu (symbol-function 'x-popup-menu))
         positions)
     (unwind-protect
         (progn
           (setq qa-snake-popup-paused nil
                 qa-snake-popup-menu nil)
           (fset 'x-popup-menu #'qa-snake-popup-menu-stub)
           (snake-mode-menu nil)
           (setq positions (copy-tree snake-positions))
           (snake-update-game (current-buffer))
           (list qa-snake-popup-paused
                 snake-paused
                 (equal positions snake-positions)
                 (eq (current-local-map) snake-mode-map)))
       (fset 'x-popup-menu original-x-popup-menu)
       (gamegrid-kill-timer))))
 '(t t t t))

(princ (qa--done))
