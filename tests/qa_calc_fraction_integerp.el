;;; qa_calc_fraction_integerp.el --- Integer predicates accept reducible fractions -*- lexical-binding:t; -*-

(require 'calc-macs)
(require 'cl-lib)

(defconst qa-calc-ext-source-file
  (expand-file-name "../lisp/calc-ext.el" (file-name-directory load-file-name)))

(defun qa-calc-eval-source-defun (function)
  "Evaluate FUNCTION from the checkout calc-ext.el source."
  (with-temp-buffer
    (insert-file-contents qa-calc-ext-source-file)
    (goto-char (point-min))
    (unless (re-search-forward
             (concat "^[(]defun " (regexp-quote (symbol-name function)) "\\_>")
             nil t)
      (error "No source definition for %s" function))
    (goto-char (match-beginning 0))
    (eval (read (current-buffer)))))

(defun qa-calc-normalize-fraction (value)
  "Normalize only the integer fraction cases needed by this predicate test."
  (if (eq (car-safe value) 'frac)
      (let ((numerator (nth 1 value))
            (denominator (nth 2 value)))
        (if (zerop (% numerator denominator))
            (/ numerator denominator)
          value))
    value))

(dolist (function '(math-messy-integerp math-num-integerp math-num-natnump))
  (qa-calc-eval-source-defun function))

(qa
 (cl-letf (((symbol-function 'math-normalize) #'qa-calc-normalize-fraction))
   (list (math-messy-integerp '(frac 4 2))
         (Math-messy-integerp '(frac 4 2))
         (math-num-integerp '(frac 4 2))
         (Math-num-integerp '(frac 4 2))
         (math-num-natnump '(frac 4 2))
         (math-num-integerp '(frac -4 2))
         (Math-num-integerp '(frac -4 2))
         (math-num-natnump '(frac -4 2))
         (math-num-integerp '(frac 3 2))
         (Math-num-integerp '(frac 3 2))
         (math-num-natnump '(frac 3 2))))
 '(t t t t t t t nil nil nil nil))

(princ (qa--done))
