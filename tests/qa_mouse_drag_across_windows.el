;;; qa_mouse_drag_across_windows.el --- Mouse drag cross-window regression -*- lexical-binding:t -*-

(require 'mouse-drag)

(defvar qa-mouse-drag-events nil)
(defvar qa-mouse-drag-scroll-calls nil)

(defun qa-mouse-drag-read-event ()
  (pop qa-mouse-drag-events))

(defun qa-mouse-drag-record-scroll (rows &optional columns)
  (push (list (selected-window) rows columns) qa-mouse-drag-scroll-calls))

(defun qa-mouse-drag-posn (window column row)
  (list window nil
        (cons (* column (frame-char-width))
              (* row (frame-char-height)))
        nil nil nil))

(qa
 (let ((old-window (selected-window)))
   (delete-other-windows)
   (let* ((start-window (selected-window))
          (other-window (split-window-right))
          (start-posn (qa-mouse-drag-posn start-window 2 5))
          (events (list
                   (list 'mouse-movement
                         (qa-mouse-drag-posn start-window 2 5))
                   (list 'mouse-movement
                         (qa-mouse-drag-posn other-window 2 5))
                   (list 'mouse-movement
                         (qa-mouse-drag-posn other-window 2 4))
                   (list 'mouse-2
                         (qa-mouse-drag-posn other-window 2 4))))
          (start-event (list 'down-mouse-2 start-posn))
          (original-reader (symbol-function 'read--potential-mouse-event))
          (original-scroll (symbol-function 'mouse-drag-safe-scroll))
          (original-repeated-scroll
           (symbol-function 'mouse-drag-repeatedly-safe-scroll))
          (qa-mouse-drag-events events)
          (qa-mouse-drag-scroll-calls nil))
     (unwind-protect
         (progn
           (fset 'read--potential-mouse-event #'qa-mouse-drag-read-event)
           (fset 'mouse-drag-safe-scroll #'qa-mouse-drag-record-scroll)
           (fset 'mouse-drag-repeatedly-safe-scroll #'ignore)
           (select-window start-window)
           (mouse-drag-drag start-event)
           (list (length qa-mouse-drag-scroll-calls)
                 (eq (caar qa-mouse-drag-scroll-calls) other-window)
                 (cadar qa-mouse-drag-scroll-calls)
                 (caddar qa-mouse-drag-scroll-calls)))
       (fset 'read--potential-mouse-event original-reader)
       (fset 'mouse-drag-safe-scroll original-scroll)
       (fset 'mouse-drag-repeatedly-safe-scroll original-repeated-scroll)
       (select-window old-window)
       (delete-other-windows))))
 '(1 t -1 0))

(princ (qa--done))
