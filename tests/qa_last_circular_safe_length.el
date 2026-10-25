;;; -*- lexical-binding: nil; -*-
;; last uses GNU safe-length's Brent count for circular lists (#495).
(qa (let ((list (list 'a 'b 'c 'd)))
      (setcdr (last list) list)
      (list (safe-length list)
            (car (last list))
            (car (last list 1))
            (car (last list 6))))
    '(10 b b a))
;; `last' observes the current safe-length function cell.
(qa (let ((original (symbol-function 'safe-length)) (calls 0))
      (unwind-protect
          (progn
            (fset 'safe-length (lambda (_list) (setq calls (1+ calls)) 2))
            (list (last '(a b c) 1) calls))
        (fset 'safe-length original)))
    '((b c) 1))
(qa-err-data (last '(a b) 0.5) 'wrong-type-argument '(integerp 1.5))
(qa (last '(a b) (expt 2 200)) '(a b))
(qa (last '(a b) (- (expt 2 200))) nil)
(princ (qa--done))
