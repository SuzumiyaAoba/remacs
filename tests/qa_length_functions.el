;;; -*- lexical-binding: nil; -*-
;; Function sequences participate in length comparisons (#357).
(qa (length= #[257 "\207" [] 1] 4) t)
(qa (length< #[257 "\207" [] 1 "doc"] 6) t)
(qa (length> #[257 "\207" [] 1 "doc" nil] 5) t)
(dolist (fn (list #[257 "\207" [] 1]
                 #[257 "\207" [] 1 "doc"]
                 #[257 "\207" [] 1 "doc" nil]
                 (make-closure #[0 "\300\207" [nil] 1] 'captured)
                 (eval '(let ((x 42)) (lambda () x)) t)))
  (let ((n (length fn)))
    (qa (length= fn n) t)
    (qa (length= fn (1+ n)) nil)
    (qa (length< fn n) nil)
    (qa (length< fn (1+ n)) t)
    (qa (length> fn n) nil)
    (qa (length> fn (1- n)) t)
    (qa-err-data (length= fn 'bad) 'wrong-type-argument '(fixnump bad))))
(princ (qa--done))
