;;; editorconfig-fnmatch.el --- Glob pattern matching  -*- lexical-binding: t -*-

;; Copyright (C) 2011-2026 Free Software Foundation, Inc.

;; Author: EditorConfig Team <editorconfig@googlegroups.com>
;; Package: editorconfig

;; See
;; https://github.com/editorconfig/editorconfig-emacs/graphs/contributors or
;; https://github.com/editorconfig/editorconfig-emacs/blob/master/CONTRIBUTORS
;; for the list of contributors.

;; This file is part of GNU Emacs.

;; GNU Emacs is free software: you can redistribute it and/or modify
;; it under the terms of the GNU General Public License as published by
;; the Free Software Foundation, either version 3 of the License, or
;; (at your option) any later version.

;; GNU Emacs is distributed in the hope that it will be useful,
;; but WITHOUT ANY WARRANTY; without even the implied warranty of
;; MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
;; See the GNU General Public License for more details.

;; You should have received a copy of the GNU General Public License
;; along with GNU Emacs.  If not, see <https://www.gnu.org/licenses/>.

;;; Commentary:

;; editorconfig-fnmatch.el provides a fnmatch implementation with a few
;; extensions.
;; The main usage of this library is glob pattern matching for EditorConfig, but
;; it can also act solely.

;; editorconfig-fnmatch-p (name pattern)

;; Test whether NAME match PATTERN.

;; PATTERN should be a shell glob pattern, and some zsh-like wildcard matching
;; can be used:

;; *           Matches any string of characters, except path separators (/)
;; **          Matches any string of characters
;; ?           Matches any single character
;; [name]      Matches any single character in name
;; [^name]     Matches any single character not in name
;; {s1,s2,s3}  Matches any of the strings given (separated by commas)
;; {min..max}  Matches any number between min and max


;; This library is a port from editorconfig-core-py library.
;; https://github.com/editorconfig/editorconfig-core-py/blob/master/editorconfig/fnmatch.py

;;; Code:

(require 'cl-lib)

(defconst editorconfig-fnmatch--cache-hashtable ;; Clear cache on file reload.
  (make-hash-table :test 'equal)
  "Cache of shell pattern and its translation.")

(defconst editorconfig-fnmatch--left-brace-regexp
  "\\(^\\|[^\\]\\){"
  "Regular expression for left brace ({).")

(defconst editorconfig-fnmatch--right-brace-regexp
  "\\(^\\|[^\\]\\)}"
  "Regular expression for right brace (}).")


(defconst editorconfig-fnmatch--numeric-range-regexp
  "\\([+-]?[0-9]+\\)\\.\\.\\([+-]?[0-9]+\\)"
  "Regular expression for numeric range (like {-3..+3}).")

(defun editorconfig-fnmatch--match-num (regexp string)
  "Return how many times REGEXP is found in STRING."
  (let ((num 0))
    ;; START arg does not work as expected in this case
    (while (string-match regexp string)
      (setq num (1+ num)
            string (substring string (match-end 0))))
    num))

(defun editorconfig-fnmatch--range-endpoint-p (member)
  "Return non-nil when MEMBER contains a character range endpoint."
  (integerp (car member)))

(defconst editorconfig-fnmatch--regexp-bracket-specials
  '(?- ?\[ ?\\ ?\] ?^)
  "Regexp bracket characters that need splitting out of ranges.")

(defun editorconfig-fnmatch--range-components (first last)
  "Split the character interval FIRST through LAST around regexp specials.
Return `char' and `range' components without expanding ordinary intervals."
  (let ((next first)
        (components nil))
    (while (<= next last)
      (if (memq next editorconfig-fnmatch--regexp-bracket-specials)
          (progn
            (push (list 'char next) components)
            (setq next (1+ next)))
        (let* ((special
                (cl-find-if (lambda (char) (> char next))
                            editorconfig-fnmatch--regexp-bracket-specials))
               (end (if (and special (<= special last))
                        (1- special)
                      last)))
          (push (if (= next end)
                    (list 'char next)
                  (list 'range next end))
                components)
          (setq next (1+ end)))))
    (nreverse components)))

(defun editorconfig-fnmatch--range-component-regexp (component)
  "Translate one range COMPONENT to an Emacs regexp."
  (if (eq (car component) 'char)
      (regexp-quote (char-to-string (cadr component)))
    (concat "[" (char-to-string (cadr component)) "-"
            (char-to-string (caddr component)) "]")))

(defun editorconfig-fnmatch--positive-bracket-regexp (members)
  "Translate positive bracket MEMBERS to a regexp."
  (let ((remaining members)
        (parts nil))
    (while remaining
      (let ((first (car remaining))
            (middle (cadr remaining))
            (last (caddr remaining)))
        (if (and middle last
                 (= (car middle) ?-)
                 (not (cdr middle))
                 (editorconfig-fnmatch--range-endpoint-p first)
                 (editorconfig-fnmatch--range-endpoint-p last))
            (let ((components
                   (editorconfig-fnmatch--range-components
                    (car first) (car last))))
              (if components
                  (dolist (component components)
                    (push (editorconfig-fnmatch--range-component-regexp
                           component)
                          parts))
                ;; Emacs word boundary followed by non-boundary cannot match
                ;; at the same position, representing a descending range.
                (push "\\b\\B" parts))
              (setq remaining (cdddr remaining)))
          (push (regexp-quote (char-to-string (car first))) parts)
          (setq remaining (cdr remaining)))))
    (setq parts (nreverse parts))
    (cond ((null parts) nil)
          ((null (cdr parts)) (car parts))
          (t (concat "\\(?:" (mapconcat #'identity parts "\\|") "\\)")))))

(defun editorconfig-fnmatch--negative-bracket-regexp (members)
  "Translate negative bracket MEMBERS to an Emacs regexp."
  (let ((literal-close nil)
        (literal-open nil)
        (literal-backslash nil)
        (literal-hyphen nil)
        (literal-caret nil)
        (ordinary-parts nil))
    (cl-labels ((record-character
                 (char)
                 (cond ((= char ?\]) (setq literal-close t))
                       ((= char ?\[) (setq literal-open t))
                       ((= char ?\\) (setq literal-backslash t))
                       ((= char ?-) (setq literal-hyphen t))
                       ((= char ?^) (setq literal-caret t))
                       (t (push (char-to-string char) ordinary-parts))))
                (record-component
                 (component)
                 (if (eq (car component) 'char)
                     (record-character (cadr component))
                   (push (concat (char-to-string (cadr component)) "-"
                                 (char-to-string (caddr component)))
                         ordinary-parts))))
      (let ((remaining members))
        (while remaining
          (let ((first (car remaining))
                (middle (cadr remaining))
                (last (caddr remaining)))
            (if (and middle last
                     (= (car middle) ?-)
                     (not (cdr middle))
                     (editorconfig-fnmatch--range-endpoint-p first)
                     (editorconfig-fnmatch--range-endpoint-p last))
                (progn
                  (dolist (component
                           (editorconfig-fnmatch--range-components
                            (car first) (car last)))
                    (record-component component))
                  (setq remaining (cdddr remaining)))
              (record-character (caar remaining))
              (setq remaining (cdr remaining))))))
    (setq ordinary-parts (nreverse ordinary-parts))
    (if (and (null ordinary-parts)
             (not literal-close) (not literal-open)
             (not literal-backslash) (not literal-caret)
             (not literal-hyphen))
        ;; A complemented empty set matches any one character, including LF.
        "\\(?:.\\|\n\\)"
      (concat "[^"
              (if literal-close "]" "")
              (mapconcat #'identity ordinary-parts "")
              (if literal-open "[" "")
              (if literal-backslash (char-to-string ?\\) "")
              (if literal-caret "^" "")
              (if literal-hyphen "-" "")
              "]")))))

(defun editorconfig-fnmatch--bracket-regexp (members negated)
  "Translate bracket MEMBERS, applying NEGATED when non-nil."
  (if negated
      (editorconfig-fnmatch--negative-bracket-regexp members)
    (editorconfig-fnmatch--positive-bracket-regexp members)))

(defun editorconfig-fnmatch--parse-bracket-expression
    (pattern opening literalize-slash)
  "Parse a bracket expression in PATTERN beginning at OPENING.
If LITERALIZE-SLASH is non-nil, quote a class containing an unescaped slash.
Return [REGEXP END SPECIAL SLASH], or nil for an incomplete or empty class."
  (let ((length (length pattern))
        (index (1+ opening))
        (negated nil)
        (first-member t)
        (members nil)
        (sensitive nil)
        (has-slash nil)
        closed)
    (when (and (< index length)
               (memq (aref pattern index) '(?! ?^)))
      (setq negated t
            index (1+ index)))
    (while (and (< index length) (not closed))
      (let ((char (aref pattern index)))
        (cond
         ((= char ?\\)
          (setq sensitive t)
          (if (< (1+ index) length)
              (let ((escaped-char (aref pattern (1+ index))))
                (when (memq escaped-char '(?\[ ?\]))
                  (setq sensitive t))
                (push (cons escaped-char t) members)
                (setq index (+ index 2)
                      first-member nil))
            (setq index length)))
         ((= char ?\])
          (if first-member
              (progn
                (setq sensitive t)
                (push (cons char t) members)
                (setq index (1+ index)
                      first-member nil))
            (setq index (1+ index)
                  closed t)))
         (t
          (when (= char ?\[)
            (setq sensitive t))
          (when (= char ?/)
            (setq has-slash t))
          (push (cons char nil) members)
          (setq index (1+ index)
                first-member nil)))))
    (when (and closed members)
      (setq members (nreverse members))
      (vector (if (and literalize-slash has-slash)
                  (regexp-quote (substring pattern opening index))
                (editorconfig-fnmatch--bracket-regexp members negated))
              index sensitive has-slash))))

(defun editorconfig-fnmatch--count-braces-outside-classes (pattern brace)
  "Count unescaped BRACE characters outside bracket expressions in PATTERN."
  (let ((index 0)
        (length (length pattern))
        (in-brackets nil)
        (first-in-brackets nil)
        (negation-allowed nil)
        (escaped nil)
        (count 0))
    (while (< index length)
      (let ((char (aref pattern index)))
        (setq index (1+ index))
        (cond
         (escaped
          (setq escaped nil)
          (when in-brackets
            (setq first-in-brackets nil
                  negation-allowed nil)))
         ((= char ?\\)
          (setq escaped t))
         ((not in-brackets)
          (cond ((= char ?\[)
                 (setq in-brackets t
                       first-in-brackets t
                       negation-allowed t))
                ((= char brace)
                 (setq count (1+ count)))))
         ((and first-in-brackets negation-allowed
               (memq char '(?! ?^)))
          ;; A single leading `!' or `^' is a negation marker.
          (setq negation-allowed nil))
         ((and in-brackets first-in-brackets (= char ?\]))
          ;; A leading `]' is a literal class member.
          (setq first-in-brackets nil
                negation-allowed nil))
         ((and in-brackets (= char ?\]))
          (setq in-brackets nil
                first-in-brackets nil
                negation-allowed nil))
         (in-brackets
          (setq first-in-brackets nil
                negation-allowed nil)))))
    count))

(defun editorconfig-fnmatch-p (string pattern)
  "Test whether STRING match PATTERN.

Matching ignores case if `case-fold-search' is non-nil.

PATTERN should be a shell glob pattern, and some zsh-like wildcard matching can
be used:

*           Matches any string of characters, except path separators (/)
**          Matches any string of characters
?           Matches any single character
[name]      Matches any single character in name
[^name]     Matches any single character not in name
{s1,s2,s3}  Matches any of the strings given (separated by commas)
{min..max}  Matches any number between min and max"
  (string-match (editorconfig-fnmatch-translate pattern)
                string))

;;(editorconfig-fnmatch-translate "{a,{-3..3}}.js")
;;(editorconfig-fnmatch-p "1.js" "{a,{-3..3}}.js")

(defun editorconfig-fnmatch-translate (pattern)
  "Translate a shell PATTERN to a regular expression.

Translation result will be cached, so same translation will not be done twice."
  (let ((cached (gethash pattern
                         editorconfig-fnmatch--cache-hashtable)))
    (or cached
        (puthash pattern
                 (editorconfig-fnmatch--do-translate pattern)
                 editorconfig-fnmatch--cache-hashtable))))


(defun editorconfig-fnmatch--do-translate (pattern &optional nested)
  "Translate a shell PATTERN to a regular expression.

Set NESTED to t when this function is called from itself.

This function is called from `editorconfig-fnmatch-translate', when no cached
translation is found for PATTERN."
  (let ((index 0)
        (length (length pattern))
        (brace-level 0)
        (in-brackets nil)
        ;; List of strings of resulting regexp, in reverse order.
        (result ())
        (is-escaped nil)
        (matching-braces
         (= (editorconfig-fnmatch--count-braces-outside-classes pattern ?{)
            (editorconfig-fnmatch--count-braces-outside-classes pattern ?})))

        current-char
        pos
        has-slash
        has-comma
        num-range)

    (while (< index length)
      (if (and (not is-escaped)
               (string-match "[^]\\*?[{},/]+"
                             ;;(string-match "[^]\\*?[{},/\\-]+" "?.a")
                             pattern
                             index)
               (eq index (match-beginning 0)))
          (progn
            (push (regexp-quote (match-string 0 pattern)) result)
            (setq index (match-end 0)
                  is-escaped nil))

        (setq current-char (aref pattern index)
              index (1+ index))

        (push
         (cl-case current-char
           (?*
            (if (or is-escaped in-brackets)
                (regexp-quote "*")
              (setq pos index)
              (if (and (< pos length)
                       (= (aref pattern pos) ?*))
                  (progn
                    (setq index (1+ index))
                    ".*")
                "[^/]*")))

           (?? (if (or is-escaped in-brackets)
                   (regexp-quote "?")
                 "[^/]"))

           (?\[
            (cond
             (is-escaped (regexp-quote "["))
             (in-brackets "\\[")
             (t
              (let ((parsed
                     (editorconfig-fnmatch--parse-bracket-expression
                      pattern (1- index) nil)))
                (if parsed
                    (progn
                      (setq index (aref parsed 1))
                      (aref parsed 0))
                  (if (= (aref pattern index) ?/)
                      ;; Slash after an half-open bracket.
                      (progn
                        (setq index (1+ index))
                        "\\[/")
                    (setq pos index
                          has-slash nil)
                    (while (and (< pos length)
                                (not (= (aref pattern pos) ?\]))
                                (not has-slash))
                      (if (and (= (aref pattern pos) ?/)
                               (not (= (aref pattern (1- pos)) ?\\)))
                          (setq has-slash t)
                        (setq pos (1+ pos))))
                    (if has-slash
                        (let ((content (substring pattern index (1+ pos))))
                          (setq index (+ pos 2))
                          (concat "\\[" content "\\]"))
                      (setq in-brackets t)
                      (if (and (< index length)
                               (memq (aref pattern index) '(?! ?^)))
                          (progn
                            (setq index (1+ index))
                            "[^")
                        "["))))))))

           (?\]
            (if is-escaped
                (regexp-quote "]")
              (setq in-brackets nil)
              "]"))

           (?\{
            (if is-escaped
                (regexp-quote "{")
              (if in-brackets
                  "{"
                (progn
            (setq pos index
                  has-comma nil)
            (let ((scan-in-brackets nil)
                  (scan-first-in-brackets nil)
                  (scan-negation-allowed nil)
                  (scan-escaped nil)
                  (scan-brace-level 0)
                  scan-closed)
              (while (and (< pos length) (not has-comma) (not scan-closed))
                (let ((scan-char (aref pattern pos)))
                  (cond
                   (scan-escaped
                    (setq scan-escaped nil)
                    (when scan-in-brackets
                      (setq scan-first-in-brackets nil
                            scan-negation-allowed nil)))
                   ((= scan-char ?\\)
                    (setq scan-escaped t))
                   (scan-in-brackets
                    (cond ((and scan-first-in-brackets scan-negation-allowed
                                (memq scan-char '(?! ?^)))
                           (setq scan-negation-allowed nil))
                          ((and scan-first-in-brackets (= scan-char ?\]))
                           (setq scan-first-in-brackets nil
                                 scan-negation-allowed nil))
                          ((= scan-char ?\])
                           (setq scan-in-brackets nil
                                 scan-first-in-brackets nil
                                 scan-negation-allowed nil))
                          (t (setq scan-first-in-brackets nil
                                   scan-negation-allowed nil))))
                   ((= scan-char ?\[)
                    (setq scan-in-brackets t
                          scan-first-in-brackets t
                          scan-negation-allowed t))
                   ((= scan-char ?{)
                    (setq scan-brace-level (1+ scan-brace-level)))
                   ((= scan-char ?})
                    (if (= scan-brace-level 0)
                        (setq scan-closed t)
                      (setq scan-brace-level (1- scan-brace-level))))
                   ((and (= scan-char ?,) (= scan-brace-level 0))
                    (setq has-comma t)))
                  (unless (or has-comma scan-closed)
                    (setq pos (1+ pos))))))
            (if (and (not has-comma)
                     (< pos length))
                (let ((pattern-sub (substring pattern index pos)))
                  (setq num-range
                        (string-match
                         (concat "\\`" editorconfig-fnmatch--numeric-range-regexp
                                 "\\'")
                         pattern-sub))
                  (setq index (1+ pos))
                  (if num-range
                      (let ((number-start (string-to-number
                                           (match-string 1 pattern-sub)))
                            (number-end (string-to-number
                                         (match-string 2 pattern-sub))))
                        (let (numeric-strings nonnegative-strings)
                          ;; Visit each numeric value once.  Keep the existing
                          ;; canonical spelling and also track nonnegative
                          ;; values for their explicit-plus form.
                          (cl-loop for i from number-start to number-end
                                   for number = (number-to-string i)
                                   do (progn
                                        (push number numeric-strings)
                                        (when (>= i 0)
                                          (push number nonnegative-strings))))
                          (setq numeric-strings (nreverse numeric-strings)
                                nonnegative-strings
                                (nreverse nonnegative-strings))
                          (let* ((regexp (regexp-opt numeric-strings))
                                 (positive-regexp
                                  (and nonnegative-strings
                                       (concat (regexp-quote "+") "0*"
                                               (if (equal numeric-strings
                                                          nonnegative-strings)
                                                   regexp
                                                 (regexp-opt nonnegative-strings)))))
                                 (parts (delq nil (list regexp positive-regexp))))
                            (if (cdr parts)
                                (concat "\\(?:" (mapconcat #'identity parts "\\|") "\\)")
                              (car parts)))))
                    (let ((inner (editorconfig-fnmatch--do-translate
                                  pattern-sub t)))
                      (format "{%s}" inner))))
              (if matching-braces
                  (progn
                    (setq brace-level (1+ brace-level))
                    "\\(?:")
                "{"))))))

           (?,
            (if (and (not in-brackets)
                     (> brace-level 0)
                     (not is-escaped))
                "\\|" ","))

           (?\}
            (if is-escaped
                (regexp-quote "}")
              (if in-brackets
                  "}"
                (if (> brace-level 0)
                    (progn
                      (setq brace-level (- brace-level 1))
                      "\\)")
                  "}"))))

           (?/
            (if is-escaped
                "/"
              (if (and (<= (+ index 3) (length pattern))
                       (string= (substring pattern index (+ index 3)) "**/"))
                  (progn
                    (setq index (+ index 3))
                    "\\(?:/\\|/.*/\\)")
                "/")))

           (?\\ (when is-escaped "\\\\"))
           (t (regexp-quote (char-to-string current-char))))
         result)

        (setq is-escaped (and (= current-char ?\\) (not is-escaped)))))
    (unless nested
      (setq result `("\\'" ,@result "\\`")))
    (apply #'concat (reverse result))))

(provide 'editorconfig-fnmatch)
;;; editorconfig-fnmatch.el ends here
