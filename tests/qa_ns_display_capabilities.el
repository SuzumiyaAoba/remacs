;;; -*- lexical-binding: t; -*-
;; NS GUI display capability queries should return values for a live frame.

(qa
 (progn
   (set-frame-parameter nil 'window-system 'ns)
   (let* ((frame (selected-frame))
          (wrapped (list (display-screens)
                         (display-backing-store)
                         (display-save-under)
                         (display-visual-class)
                         (display-grayscale-p)
                         (display-planes)
                         (display-color-cells)))
          (primitive (list (x-display-screens frame)
                           (x-display-backing-store frame)
                           (x-display-save-under frame)
                           (x-display-visual-class frame)
                           (x-display-grayscale-p frame)
                           (x-display-planes frame)
                           (x-display-color-cells frame))))
     (list (equal wrapped primitive)
           (= (nth 0 wrapped) 1)
           (eq (nth 1 wrapped) 'buffered)
           (nth 2 wrapped)
           (eq (nth 3 wrapped) 'direct-color)
           (nth 4 wrapped)
           (> (nth 5 wrapped) 1)
           (= (nth 6 wrapped)
              (expt 2 (min (nth 5 wrapped) 24))))))
 '(t t t t t t t t))

(princ (qa--done))
