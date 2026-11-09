;;; -*- lexical-binding: nil; -*-
;; Both games expose Pause and Resume in their menus.
(require 'cl-lib)
(load-file (expand-file-name "../lisp/tetris.el"
                             (file-name-directory load-file-name)))
(load-file (expand-file-name "../lisp/snake.el"
                             (file-name-directory load-file-name)))
(defun qa-game-menu-labels (menu)
  (let (labels)
    (dolist (item (cdr menu) (nreverse labels))
      (when (vectorp item)
        (push (aref item 0) labels)))))
(qa (qa-game-menu-labels tetris--menu-def)
    '("Start new game" "End game" "Pause" "Resume"))
(qa (qa-game-menu-labels snake--menu-def)
    '("Start new game" "End game" "Pause" "Resume"))

;; The menu filter pauses the game before the timer callback can advance it.
(let (tetris-ticked)
  (cl-letf (((symbol-function 'tetris-erase-shape)
             (lambda () (setq tetris-ticked t))))
    (with-temp-buffer
      (use-local-map tetris-mode-map)
      (setq-local tetris-paused nil)
      (tetris--menu-filter nil)
      (tetris-update-game (current-buffer))
      (qa tetris-paused t)
      (qa tetris-ticked nil))))
(let (snake-ticked)
  (cl-letf (((symbol-function 'snake-update-velocity)
             (lambda () (setq snake-ticked t))))
    (with-temp-buffer
      (use-local-map snake-mode-map)
      (setq-local snake-paused nil)
      (snake--menu-filter nil)
      (snake-update-game (current-buffer))
      (qa snake-paused t)
      (qa snake-ticked nil))))
(princ (qa--done))
