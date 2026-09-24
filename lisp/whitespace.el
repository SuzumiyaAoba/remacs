;;; whitespace.el --- minor mode to visualize TAB, (HARD) SPC, NEWLINE  -*- lexical-binding: t -*-
;; Ported from GNU Emacs whitespace.el for remacs.

(defvar whitespace-style
  '(face
    tabs spaces trailing lines space-before-tab newline
    indentation empty space-after-tab
    space-mark tab-mark newline-mark
    missing-newline-at-eof)
  "Determine the kinds of whitespace are visualized.")

(defvar whitespace-action nil
  "Specify the whitespace cleanup action to be taken.")

(defvar whitespace-trailing-regexp
  "\\([\t \u00A0]+\\)$"
  "Regexp to match trailing characters that should be visualized.")

(defvar whitespace-space-before-tab-regexp "\\( +\\)\\(\t+\\)"
  "Regexp to match SPACEs before TAB that should be visualized.")

(defvar whitespace-indentation-regexp
  '("^\t*\\(\\( \\{%d\\}\\)+\\)[^\n\t]"
    . "^ *\\(\t+\\).")
  "Regexps to match indentation whitespace that should be visualized.")

(defvar whitespace-empty-at-bob-regexp "\\`\\([ \t\n]*\\(?:\n\\|$\\)\\)"
  "Regexp to match empty lines at beginning of buffer.")

(defvar whitespace-empty-at-eob-regexp "^\\([ \t\n]+\\)\\'"
  "Regexp to match empty lines at end of buffer.")

(defvar whitespace-space-after-tab-regexp
  '("\t+\\(\\( \\{%d,\\}\\)+\\)"
    . "\\(\t+\\) \\{%d,\\}")
  "Regexps to match multiple SPACEs after TAB that should be visualized.")

(defun whitespace-warn-read-only (msg)
  "Warn if buffer is read-only."
  (when (memq 'warn-if-read-only whitespace-action)
    (message "Can't %s: %s is read-only" msg (buffer-name))))

(defun whitespace-cleanup ()
  "Cleanup some blank problems in all buffer or at region.

It usually applies to the whole buffer, but in transient mark
mode when the mark is active, it applies to the region.  It also
applies to the region when it is not in transient mark mode, the
mark is active and \\[universal-argument] was pressed just before
calling `whitespace-cleanup' interactively.

See also `whitespace-cleanup-region'.

The problems cleaned up are:

1. empty lines at beginning of buffer.
2. empty lines at end of buffer.
   If `whitespace-style' includes the value `empty', remove all
   empty lines at beginning and/or end of buffer.

3. `tab-width' or more SPACEs at beginning of line.
   If `whitespace-style' includes the value `indentation':
   replace `tab-width' or more SPACEs at beginning of line by
   TABs, if `indent-tabs-mode' is non-nil; otherwise, replace TABs by
   SPACEs.
   If `whitespace-style' includes the value `indentation::tab',
   replace `tab-width' or more SPACEs at beginning of line by TABs.
   If `whitespace-style' includes the value `indentation::space',
   replace TABs by SPACEs.

4. SPACEs before TAB.
   If `whitespace-style' includes the value `space-before-tab':
   replace SPACEs by TABs, if `indent-tabs-mode' is non-nil;
   otherwise, replace TABs by SPACEs.
   If `whitespace-style' includes the value
   `space-before-tab::tab', replace SPACEs by TABs.
   If `whitespace-style' includes the value
   `space-before-tab::space', replace TABs by SPACEs.

5. SPACEs or TABs at end of line.
   If `whitespace-style' includes the value `trailing', remove
   all SPACEs or TABs at end of line.

6. `tab-width' or more SPACEs after TAB.
   If `whitespace-style' includes the value `space-after-tab':
   replace SPACEs by TABs, if `indent-tabs-mode' is non-nil;
   otherwise, replace TABs by SPACEs.
   If `whitespace-style' includes the value
   `space-after-tab::tab', replace SPACEs by TABs.
   If `whitespace-style' includes the value
   `space-after-tab::space', replace TABs by SPACEs.

See `whitespace-style', `indent-tabs-mode' and `tab-width' for
documentation."
  (interactive "@")
  (cond
   ;; read-only buffer
   (buffer-read-only
    (whitespace-warn-read-only "cleanup"))
   ;; region active
   ((and (or transient-mark-mode
	     current-prefix-arg)
	 mark-active)
    ;; PROBLEMs 1 and 2 are not handled in region
    ;; PROBLEM 3: `tab-width' or more SPACEs at bol
    ;; PROBLEM 4: SPACEs before TAB
    ;; PROBLEM 5: SPACEs or TABs at eol
    ;; PROBLEM 6: `tab-width' or more SPACEs after TAB
    (whitespace-cleanup-region (region-beginning) (region-end)))
   ;; whole buffer
   (t
    (save-excursion
      ;; PROBLEM 1: empty lines at bob
      ;; PROBLEM 2: empty lines at eob
      ;; ACTION: remove all empty lines at bob and/or eob
      (when (memq 'empty whitespace-style)
        (let (overwrite-mode)		; enforce no overwrite
          (goto-char (point-min))
          (when (looking-at whitespace-empty-at-bob-regexp)
            (delete-region (match-beginning 1) (match-end 1)))
          (when (re-search-forward
                 whitespace-empty-at-eob-regexp nil t)
            (delete-region (match-beginning 1) (match-end 1))))))
    ;; PROBLEM 3: `tab-width' or more SPACEs at bol
    ;; PROBLEM 4: SPACEs before TAB
    ;; PROBLEM 5: SPACEs or TABs at eol
    ;; PROBLEM 6: `tab-width' or more SPACEs after TAB
    (whitespace-cleanup-region (point-min) (point-max)))))

(defun whitespace-cleanup-region (start end)
  "Cleanup some blank problems at region."
  (interactive "@r")
  (if buffer-read-only
      ;; read-only buffer
      (whitespace-warn-read-only "cleanup region")
    ;; non-read-only buffer
    (let ((rstart           (min start end))
	  (rend             (copy-marker (max start end)))
	  overwrite-mode		; enforce no overwrite
	  tmp)
      (save-excursion
        ;; PROBLEM 1: `tab-width' or more SPACEs at bol
        (cond
         ;; ACTION: replace `tab-width' or more SPACEs at bol by TABs, if
         ;; `indent-tabs-mode' is non-nil; otherwise, replace TABs
         ;; by SPACEs.
         ((memq 'indentation whitespace-style)
          (let ((regexp (whitespace-indentation-regexp)))
            (goto-char rstart)
            (while (re-search-forward regexp rend t)
              (setq tmp (current-indentation))
              (goto-char (match-beginning 0))
              (delete-horizontal-space)
              (unless (eolp)
                (indent-to tmp)))))
         ;; ACTION: replace `tab-width' or more SPACEs at bol by TABs.
         ((memq 'indentation::tab whitespace-style)
          (whitespace-replace-action
           'tabify rstart rend
           (whitespace-indentation-regexp 'tab) 0))
         ;; ACTION: replace TABs by SPACEs.
         ((memq 'indentation::space whitespace-style)
          (whitespace-replace-action
           'untabify rstart rend
           (whitespace-indentation-regexp 'space) 0)))
        ;; PROBLEM 3: SPACEs or TABs at eol
        ;; ACTION: remove all SPACEs or TABs at eol
        (when (memq 'trailing whitespace-style)
          (whitespace-replace-action
           'delete-region rstart rend
           whitespace-trailing-regexp 1))
        ;; PROBLEM 4: `tab-width' or more SPACEs after TAB
        (cond
         ;; ACTION: replace `tab-width' or more SPACEs by TABs, if
         ;; `indent-tabs-mode' is non-nil; otherwise, replace TABs
         ;; by SPACEs.
         ((memq 'space-after-tab whitespace-style)
          (whitespace-replace-action
           (if indent-tabs-mode 'tabify 'untabify)
           rstart rend (whitespace-space-after-tab-regexp) 1))
         ;; ACTION: replace `tab-width' or more SPACEs by TABs.
         ((memq 'space-after-tab::tab whitespace-style)
          (whitespace-replace-action
           'tabify rstart rend
           (whitespace-space-after-tab-regexp 'tab) 1))
         ;; ACTION: replace TABs by SPACEs.
         ((memq 'space-after-tab::space whitespace-style)
          (whitespace-replace-action
           'untabify rstart rend
           (whitespace-space-after-tab-regexp 'space) 1)))
        ;; PROBLEM 2: SPACEs before TAB
        (cond
         ;; ACTION: replace SPACEs before TAB by TABs, if
         ;; `indent-tabs-mode' is non-nil; otherwise, replace TABs
         ;; by SPACEs.
         ((memq 'space-before-tab whitespace-style)
          (whitespace-replace-action
           (if indent-tabs-mode 'tabify 'untabify)
           rstart rend whitespace-space-before-tab-regexp
           (if indent-tabs-mode 0 2)))
         ;; ACTION: replace SPACEs before TAB by TABs.
         ((memq 'space-before-tab::tab whitespace-style)
          (whitespace-replace-action
           'tabify rstart rend
           whitespace-space-before-tab-regexp 0))
         ;; ACTION: replace TABs by SPACEs.
         ((memq 'space-before-tab::space whitespace-style)
          (whitespace-replace-action
           'untabify rstart rend
           whitespace-space-before-tab-regexp 2)))
        ;; PROBLEM 5: missing newline at end of file
        (and (memq 'missing-newline-at-eof whitespace-style)
             (> (point-max) (point-min))
             (= (point-max) (without-restriction (point-max)))
             (/= (char-before (point-max)) ?\n)
             (not (and (eq selective-display t)
                       (= (char-before (point-max)) ?\r)))
             (goto-char (point-max))
             (ignore-errors (insert "\n"))))
      (set-marker rend nil))))		; point marker to nowhere

(defun whitespace-replace-action (action rstart rend regexp index)
  "Do ACTION in the string matched by REGEXP between RSTART and REND.

INDEX is the level group matched by REGEXP and used by ACTION."
  (goto-char rstart)
  (while (re-search-forward regexp rend t)
    (goto-char (match-end index))
    (funcall action (match-beginning index) (match-end index))))

(defun whitespace-regexp (regexp &optional kind)
  "Return REGEXP depending on `indent-tabs-mode'."
  (format
   (cond
    ((or (eq kind 'tab)
         indent-tabs-mode)
     (car regexp))
    ((or (eq kind 'space)
         (not indent-tabs-mode))
     (cdr regexp)))
   tab-width))

(defun whitespace-indentation-regexp (&optional kind)
  "Return the indentation regexp depending on `indent-tabs-mode'."
  (whitespace-regexp whitespace-indentation-regexp kind))

(defun whitespace-space-after-tab-regexp (&optional kind)
  "Return the space-after-tab regexp depending on `indent-tabs-mode'."
  (whitespace-regexp whitespace-space-after-tab-regexp kind))

;; ---------- whitespace.el display/mode half (GNU port) ----------

(defvar whitespace-hspace-regexp "\\( +\\)"
  "Regexp to match HARD SPACE characters that should be visualized.")

(defvar whitespace-space-regexp "\\( +\\)"
  "Regexp to match SPACE characters that should be visualized.")

(defvar whitespace-tab-regexp "\\(\t+\\)"
  "Regexp to match TAB characters that should be visualized.")

(defvar whitespace-big-indent-regexp
  "^\\(\\(?:\t\\{4,\\}\\| \\{32,\\}\\)[\t ]*\\)"
  "Regexp to match big indentation at BOL that should be visualized.")

(defvar whitespace-line-column 80
  "Column beyond which the line is highlighted.
If nil, use the value of `fill-column'.")

(defvar whitespace-display-mappings
  '((space-mark   ?\     [?·]     [?.])
    (space-mark   ?\xA0  [?¤]     [?_])
    (newline-mark ?\n    [?$ ?\n])
    (tab-mark     ?\t    [?» ?\t] [?\\ ?\t]))
  "Alist of mappings for displaying characters: (KIND CHAR VECTOR...).")

(defvar whitespace-global-modes t
  "Modes for which global `whitespace-mode' is automatically turned on.")

(defvar whitespace-global-mode-buffers '("\\`\\*scratch\\*\\'")
  "Buffer name regexps where global `whitespace-mode' can be auto-enabled.")

(defvar whitespace-report-buffer-name "*Whitespace Report*"
  "The buffer name for whitespace bogus report.")

(defvar whitespace-report-list
  (list
   (cons 'empty                   whitespace-empty-at-bob-regexp)
   (cons 'empty                   whitespace-empty-at-eob-regexp)
   (cons 'trailing                whitespace-trailing-regexp)
   (cons 'indentation             nil)
   (cons 'indentation::tab        nil)
   (cons 'indentation::space      nil)
   (cons 'space-before-tab        whitespace-space-before-tab-regexp)
   (cons 'space-before-tab::tab   whitespace-space-before-tab-regexp)
   (cons 'space-before-tab::space whitespace-space-before-tab-regexp)
   (cons 'space-after-tab         nil)
   (cons 'space-after-tab::tab    nil)
   (cons 'space-after-tab::space  nil))
  "List of whitespace bogus symbol and corresponding regexp.")

(defvar whitespace-report-text
  '( ;; `indent-tabs-mode' has non-nil value
    "\
 Whitespace Report

 Current Setting                       Whitespace Problem

 empty                    []     []  empty lines at beginning of buffer
 empty                    []     []  empty lines at end of buffer
 trailing                 []     []  SPACEs or TABs at end of line
 indentation              []     []  >= `tab-width' SPACEs at beginning of line
 indentation::tab         []     []  >= `tab-width' SPACEs at beginning of line
 indentation::space       []     []  TABs at beginning of line
 space-before-tab         []     []  SPACEs before TAB
 space-before-tab::tab    []     []  SPACEs before TAB: SPACEs
 space-before-tab::space  []     []  SPACEs before TAB: TABs
 space-after-tab          []     []  >= `tab-width' SPACEs after TAB
 space-after-tab::tab     []     []  >= `tab-width' SPACEs after TAB: SPACEs
 space-after-tab::space   []     []  >= `tab-width' SPACEs after TAB: TABs

 indent-tabs-mode =
 tab-width        = \n\n"
    . ;; `indent-tabs-mode' has nil value
    "\
 Whitespace Report

 Current Setting                       Whitespace Problem

 empty                    []     []  empty lines at beginning of buffer
 empty                    []     []  empty lines at end of buffer
 trailing                 []     []  SPACEs or TABs at end of line
 indentation              []     []  TABs at beginning of line
 indentation::tab         []     []  >= `tab-width' SPACEs at beginning of line
 indentation::space       []     []  TABs at beginning of line
 space-before-tab         []     []  SPACEs before TAB
 space-before-tab::tab    []     []  SPACEs before TAB: SPACEs
 space-before-tab::space  []     []  SPACEs before TAB: TABs
 space-after-tab          []     []  >= `tab-width' SPACEs after TAB
 space-after-tab::tab     []     []  >= `tab-width' SPACEs after TAB: SPACEs
 space-after-tab::space   []     []  >= `tab-width' SPACEs after TAB: TABs

 indent-tabs-mode =
 tab-width        = \n\n")
  "Text for whitespace bogus report: (INDENT-TABS . NO-INDENT-TABS).")

(defvar whitespace-style-value-list
  '(face
    tabs
    spaces
    trailing
    page-delimiters
    lines
    lines-tail
    lines-char
    newline
    empty
    indentation
    indentation::tab
    indentation::space
    big-indent
    space-after-tab
    space-after-tab::tab
    space-after-tab::space
    space-before-tab
    space-before-tab::tab
    space-before-tab::space
    help-newline
    tab-mark
    space-mark
    newline-mark)
  "List of valid `whitespace-style' values.")

(defvar whitespace-toggle-option-alist
  '((?f    . face)
    (?t    . tabs)
    (?s    . spaces)
    (?p    . page-delimiters)
    (?r    . trailing)
    (?l    . lines)
    (?L    . lines-tail)
    (?\C-l . lines-char)
    (?n    . newline)
    (?e    . empty)
    (?\C-i . indentation)
    (?I    . indentation::tab)
    (?i    . indentation::space)
    (?\C-t . big-indent)
    (?\C-a . space-after-tab)
    (?A    . space-after-tab::tab)
    (?a    . space-after-tab::space)
    (?\C-b . space-before-tab)
    (?B    . space-before-tab::tab)
    (?b    . space-before-tab::space)
    (?T    . tab-mark)
    (?S    . space-mark)
    (?N    . newline-mark)
    (?x    . whitespace-style))
  "Alist of toggle options: (CHAR . SYMBOL).")

(defvar-local whitespace-active-style nil
  "Used to save locally `whitespace-style' value.")

(defvar-local whitespace-point (point)
  "Used to save locally current point value.")

(defvar-local whitespace-point--used nil
  "Region whose highlighting depends on `whitespace-point'.")

(defvar-local whitespace-bob-marker nil
  "Position of the buffer's first non-empty line.")

(defvar-local whitespace-eob-marker nil
  "Position after the buffer's last non-empty line.")

(defvar-local whitespace-buffer-changed nil
  "Used to indicate locally if buffer changed.")

(defvar-local whitespace-display-table nil
  "Used to save a local display table.")

(defvar-local whitespace-display-table-was-local nil
  "Used to remember whether a buffer initially had a local display table.")

(defvar-local buffer-display-table nil
  "The buffer's display table (a char-table) or nil.")

(defvar standard-display-table nil
  "The standard display table, nil by default.")

(defvar whitespace-toggle-style nil
  "Used to toggle the global `whitespace-style' value.")

(defvar whitespace--page-delimiters-keyword
  `((,(lambda (bound)
        (re-search-forward (concat page-delimiter "\n") bound t))
     0
     (prog1 nil
       (put-text-property (match-beginning 0) (1- (match-end 0)) 'display " ")
       (add-text-properties (match-beginning 0) (match-end 0)
                            '(face whitespace-page-delimiter
                                   display-line-numbers-disable t)))))
  "Used to add page delimiters keywords to `whitespace-font-lock-keywords'.")

(defvar whitespace-enable-predicate
  (lambda ()
    (and (cond
          ((eq whitespace-global-modes t))
          ((listp whitespace-global-modes)
           (if (eq (car-safe whitespace-global-modes) 'not)
               (not (derived-mode-p (cdr whitespace-global-modes)))
             (derived-mode-p whitespace-global-modes)))
          (t nil))
         ;; ...we have a display (not running a batch job)
         (not noninteractive)
         ;; ...the buffer is not internal (name starts with a space)
         (not (eq (aref (buffer-name) 0) ?\s))
         ;; ...the buffer is not special (name starts with *)
         (or (not (eq (aref (buffer-name) 0) ?*))
             ;; except, e.g., the scratch buffer.
             (seq-some (lambda (re)
                         (string-match-p re (buffer-name)))
                       whitespace-global-mode-buffers))))
  "Predicate to decide which buffers obey `global-whitespace-mode'.")

(defface whitespace-hspace
  '((((class color) (background dark))
     :background "grey24"        :foreground "darkgray")
    (((class color) (background light))
     :background "LemonChiffon3" :foreground "lightgray")
    (t :inverse-video t))
  "Face used to visualize HARD SPACE.")

(defface whitespace-tab
  '((((class color) (background dark))
     :background "grey22" :foreground "darkgray")
    (((class color) (background light))
     :background "beige"  :foreground "lightgray")
    (t :inverse-video t))
  "Face used to visualize TAB.")

(defface whitespace-space
  '((((class color) (background dark))
     :background "grey20"      :foreground "darkgray")
    (((class color) (background light))
     :background "lightyellow" :foreground "lightgray")
    (t :inverse-video t))
  "Face used to visualize SPACE.")

(defface whitespace-newline
  '((default :weight normal)
    (((class color) (background dark)) :foreground "darkgray")
    (((class color) (min-colors 88) (background light)) :foreground "lightgray")
    (((class color) (background light)) :foreground "brown")
    (t :underline t))
  "Face used to visualize NEWLINE char mapping.")

(defface whitespace-trailing
  '((default :weight bold)
    (((class mono)) :inverse-video t :underline t)
    (t :background "red1" :foreground "yellow"))
  "Face used to visualize trailing blanks.")

(defface whitespace-line
  '((((class mono)) :inverse-video t :weight bold :underline t)
    (t :background "gray20" :foreground "violet"))
  "Face used to visualize \"long\" lines.")

(defface whitespace-space-before-tab
  '((((class mono)) :inverse-video t :weight bold :underline t)
    (t :background "DarkOrange" :foreground "firebrick"))
  "Face used to visualize SPACEs before TAB.")

(defface whitespace-indentation
  '((((class mono)) :inverse-video t :weight bold :underline t)
    (t :background "yellow" :foreground "firebrick"))
  "Face used to visualize indentation.")

(defface whitespace-big-indent
  '((((class mono)) :inverse-video t :weight bold :underline t)
    (t :background "red" :foreground "firebrick"))
  "Face used to visualize big indentation.")

(defface whitespace-missing-newline-at-eof
  '((((class mono)) :inverse-video t :weight bold :underline t)
    (t :background "#d0d040" :foreground "black"))
  "Face used to visualize missing newline at end of file.")

(defface whitespace-empty
  '((((class mono)) :inverse-video t :weight bold :underline t)
    (t :background "yellow" :foreground "firebrick" :extend t))
  "Face used to visualize empty lines at bob and/or eob.")

(defface whitespace-space-after-tab
  '((((class mono)) :inverse-video t :weight bold :underline t)
    (t :background "yellow" :foreground "firebrick"))
  "Face used to visualize SPACEs after TAB.")

(defface whitespace-page-delimiter
  '((t :inherit shadow :strike-through t :extend t))
  "Face used to visualize page delimiters.")

(define-minor-mode whitespace-mode
  "Toggle whitespace visualization (Whitespace mode)."
  :lighter    " ws"
  (cond
   (noninteractive			; running a batch job
    (setq whitespace-mode nil))
   (whitespace-mode			; whitespace-mode on
    (whitespace-turn-on)
    (whitespace-action-when-on))
   (t					; whitespace-mode off
    (whitespace-turn-off))))

(define-minor-mode whitespace-newline-mode
  "Toggle newline visualization (Whitespace Newline mode)."
  :lighter    " nl"
  (let ((whitespace-style '(face newline-mark newline)))
    (whitespace-mode (if whitespace-newline-mode
			 1 -1)))
  ;; sync states (running a batch job)
  (setq whitespace-newline-mode whitespace-mode))

(define-minor-mode whitespace-page-delimiters-mode
  "Display page-break delimiter characters as horizontal lines."
  :lighter " pd"
  (let ((whitespace-style '(face page-delimiters)))
    (whitespace-mode (if whitespace-page-delimiters-mode
                         1 -1))))

(define-globalized-minor-mode global-whitespace-mode
  whitespace-mode
  whitespace-turn-on-if-enabled
  :init-value nil)

(defun whitespace-turn-on-if-enabled ()
  (when (funcall whitespace-enable-predicate)
    (whitespace-mode)))

(define-minor-mode global-whitespace-newline-mode
  "Toggle global newline visualization (Global Whitespace Newline mode)."
  :lighter    " NL"
  :global     t
  (let ((whitespace-style '(newline-mark newline)))
    (global-whitespace-mode (if global-whitespace-newline-mode
                                1 -1))
    ;; sync states (running a batch job)
    (setq global-whitespace-newline-mode global-whitespace-mode)))
(make-obsolete 'global-whitespace-newline-mode
               "use `global-whitespace-mode' with `whitespace-style' set to `(newline-mark newline)' instead."
               "28.1")

(defun whitespace-toggle-options (arg)
  "Toggle local `whitespace-mode' options."
  (interactive (whitespace-interactive-char t))
  (let ((whitespace-style
	 (whitespace-toggle-list t arg whitespace-active-style)))
    (whitespace-mode 0)
    (whitespace-mode 1)))

(defun global-whitespace-toggle-options (arg)
  "Toggle global `whitespace-mode' options."
  (interactive (whitespace-interactive-char nil))
  (let ((whitespace-toggle-style
         (whitespace-toggle-list nil arg whitespace-toggle-style)))
    (global-whitespace-mode 0)
    (global-whitespace-mode 1)))

(defun whitespace-toggle-list (local-p arg the-list)
  "Toggle options in THE-LIST based on list ARG."
  (unless (if local-p whitespace-mode global-whitespace-mode)
    (setq the-list whitespace-style))
  (setq the-list (copy-sequence the-list)) ; keep original list
  (dolist (sym (if (listp arg) arg (list arg)))
    (cond
     ;; ignore help value
     ((eq sym 'help-newline))
     ;; restore default values
     ((eq sym 'whitespace-style)
      (setq the-list whitespace-style))
     ;; toggle valid values
     ((memq sym whitespace-style-value-list)
      (setq the-list (if (memq sym the-list)
			 (delq sym the-list)
		       (cons sym the-list))))))
  the-list)

(defun whitespace-insert-value (value)
  "Insert VALUE at column 20 of next line."
  (forward-line 1)
  (move-to-column 20 t)
  (insert (format "%s" value)))

(defun whitespace-mark-x (nchars condition)
  "Insert the mark (`X' or ` ') after NCHARS depending on CONDITION."
  (forward-char nchars)
  (insert (if condition "X" " ")))

(defun whitespace-insert-option-mark (the-list the-value)
  "Insert the option mark (`X' or ` ') in toggle options buffer."
  (goto-char (point-min))
  (forward-line 2)
  (dolist (sym  the-list)
    (if (eq sym 'help-newline)
	(forward-line 2)
      (forward-line 1)
      (whitespace-mark-x 2 (memq sym the-value)))))

(defvar whitespace-help-buffer-name "*Whitespace Toggle Options*"
  "The buffer name for whitespace toggle options.")

(defconst whitespace-help-text
  "\
 Whitespace Toggle Options                  | scroll up  :  SPC   or > |
                                            | scroll down:  M-SPC or < |
 FACES                                      \\__________________________/
 []  f   - toggle face visualization
 []  t   - toggle TAB visualization
 []  s   - toggle SPACE and HARD SPACE visualization
 []  r   - toggle trailing blanks visualization
 []  l   - toggle \"long lines\" visualization
 []  L   - toggle \"long lines\" tail visualization
 []  C-l - toggle \"long lines\" one character visualization
 []  n   - toggle NEWLINE visualization
 []  e   - toggle empty line at bob and/or eob visualization
 []  C-i - toggle indentation SPACEs visualization (via `indent-tabs-mode')
 []  I   - toggle indentation SPACEs visualization
 []  i   - toggle indentation TABs visualization
 []  C-t - toggle big indentation visualization
 []  C-a - toggle SPACEs after TAB visualization (via `indent-tabs-mode')
 []  A   - toggle SPACEs after TAB: SPACEs visualization
 []  a   - toggle SPACEs after TAB: TABs visualization
 []  C-b - toggle SPACEs before TAB visualization (via `indent-tabs-mode')
 []  B   - toggle SPACEs before TAB: SPACEs visualization
 []  b   - toggle SPACEs before TAB: TABs visualization

 DISPLAY TABLE
 []  T - toggle TAB visualization
 []  S - toggle SPACE and HARD SPACE visualization
 []  N - toggle NEWLINE visualization

      x - restore `whitespace-style' value

      ? - display this text\n\n"
  "Text for whitespace toggle options.")

(defun whitespace-help-on (style)
  "Display the whitespace toggle options."
  (unless (get-buffer whitespace-help-buffer-name)
    (delete-other-windows)
    (let ((buffer (get-buffer-create whitespace-help-buffer-name)))
      (with-current-buffer buffer
	(erase-buffer)
	(insert whitespace-help-text)
	(whitespace-insert-option-mark
	 whitespace-style-value-list style)
	(whitespace-display-window buffer)))))

(defun whitespace-display-window (buffer)
  "Display BUFFER, preferably below the selected window."
  (goto-char (point-min))
  (set-buffer-modified-p nil)
  (let ((window (display-buffer
	         buffer
	         `((display-buffer-reuse-window
		    display-buffer-below-selected)))))
    (shrink-window-if-larger-than-buffer window)))

(defun whitespace-kill-buffer (buffer-name)
  "Kill buffer BUFFER-NAME and windows related with it."
  (let ((buffer (get-buffer buffer-name)))
    (when buffer
      (delete-windows-on buffer)
      (kill-buffer buffer))))

(defun whitespace-help-off ()
  "Remove the buffer and window of the whitespace toggle options."
  (whitespace-kill-buffer whitespace-help-buffer-name))

(defun whitespace-help-scroll (&optional up)
  "Scroll help window, if it exists."
  (condition-case nil
      (let ((buffer (get-buffer whitespace-help-buffer-name)))
	(if buffer
	    (with-selected-window (get-buffer-window buffer)
	      (if up
		  (scroll-up 3)
		(scroll-down 3)))
	  (ding)))
    ;; handler
    ((error)
     ;; just ignore error
     )))

(defun whitespace-interactive-char (local-p)
  "Interactive function to read a char and return a symbol."
  (let* ((is-off (not (if local-p
			  whitespace-mode
			global-whitespace-mode)))
	 (style  (cond (is-off  whitespace-style) ; use default value
		       (local-p whitespace-active-style)
		       (t       whitespace-toggle-style)))
	 (prompt
	  (format "Whitespace Toggle %s (type ? for further options)-"
		  (if local-p "Local" "Global")))
	 ch sym)
    ;; read a valid option and get the corresponding symbol
    (save-window-excursion
      (condition-case data
	  (progn
	    (while
		;; while condition
		(progn
		  (setq ch (read-char prompt))
		  (not
		   (setq sym
			 (cdr
			  (assq ch whitespace-toggle-option-alist)))))
	      ;; while body
	      (cond
	       ((eq ch ?\?)   (whitespace-help-on style))
	       ((eq ch ?\ )   (whitespace-help-scroll t))
	       ((eq ch ?\M- ) (whitespace-help-scroll))
	       ((eq ch ?>)    (whitespace-help-scroll t))
	       ((eq ch ?<)    (whitespace-help-scroll))
	       (t             (ding))))
	    (whitespace-help-off)
	    (message " "))		; clean echo area
	;; handler
	((quit error)
	 (whitespace-help-off)
	 (error (error-message-string data)))))
    (list sym)))			; return the appropriate symbol

(defun whitespace-report (&optional force report-if-bogus)
  "Report some whitespace problems in buffer."
  (interactive (list current-prefix-arg))
  (whitespace-report-region (point-min) (point-max)
			    force report-if-bogus))

(defun whitespace-report-region (start end &optional force report-if-bogus)
  "Report some whitespace problems in a region."
  (interactive "r")
  (setq force (or current-prefix-arg force))
  (save-excursion
    (let* ((has-bogus nil)
           (rstart    (min start end))
           (rend      (max start end))
           ;; Fall back to whitespace-style so we can run before
           ;; the mode is active.
           (style     (copy-sequence
                       (or whitespace-active-style whitespace-style)))
           (bogus-list
            (mapcar
             (lambda (option)
               (when force
                 (push (car option) style))
               (goto-char rstart)
               (let ((regexp
                      (cond
                       ((eq (car option) 'indentation)
                        (whitespace-indentation-regexp))
                       ((eq (car option) 'indentation::tab)
                        (whitespace-indentation-regexp 'tab))
                       ((eq (car option) 'indentation::space)
                        (whitespace-indentation-regexp 'space))
                       ((eq (car option) 'space-after-tab)
                        (whitespace-space-after-tab-regexp))
                       ((eq (car option) 'space-after-tab::tab)
                        (whitespace-space-after-tab-regexp 'tab))
                       ((eq (car option) 'space-after-tab::space)
                        (whitespace-space-after-tab-regexp 'space))
                       ((eq (car option) 'missing-newline-at-eof)
                        ".\\'")
                       (t
                        (cdr option)))))
                 (when (re-search-forward regexp rend t)
                   (unless has-bogus
                     (setq has-bogus (memq (car option) style)))
                   t)))
             whitespace-report-list)))
      (when (pcase report-if-bogus ('nil t) ('never nil) (_ has-bogus))
        (whitespace-kill-buffer whitespace-report-buffer-name)
        ;; `indent-tabs-mode' may be local to current buffer
        ;; `tab-width' may be local to current buffer
        (let ((ws-indent-tabs-mode indent-tabs-mode)
              (ws-tab-width tab-width))
          (with-current-buffer (get-buffer-create
                                whitespace-report-buffer-name)
            (let ((inhibit-read-only t))
              (special-mode)
              (erase-buffer)
              (insert (if ws-indent-tabs-mode
                          (car whitespace-report-text)
                        (cdr whitespace-report-text)))
              (goto-char (point-min))
              (forward-line 3)
              (dolist (option whitespace-report-list)
                (forward-line 1)
                (whitespace-mark-x
                 27 (memq (car option) style))
                (whitespace-mark-x 7 (car bogus-list))
                (setq bogus-list (cdr bogus-list)))
              (forward-line 1)
              (whitespace-insert-value ws-indent-tabs-mode)
              (whitespace-insert-value ws-tab-width)
              (when has-bogus
                (goto-char (point-max))
                (insert (substitute-command-keys
                         " Type \\[whitespace-cleanup]")
                        " to cleanup the buffer.\n\n"
                        (substitute-command-keys
                         " Type \\[whitespace-cleanup-region]")
                        " to cleanup a region.\n\n"))
              (whitespace-display-window (current-buffer))))))
      has-bogus)))

(defvar-local whitespace-font-lock-keywords nil
  "Used to save the value `whitespace-color-on' adds to `font-lock-keywords'.")

(defun whitespace-turn-on ()
  "Turn on whitespace visualization."
  ;; prepare local hooks
  (add-hook 'write-file-functions #'whitespace-write-file-hook nil t)
  ;; create whitespace local buffer environment
  (setq-local whitespace-font-lock-keywords nil)
  (setq-local whitespace-display-table nil)
  (setq-local whitespace-display-table-was-local nil)
  (setq-local whitespace-active-style
              (if (listp whitespace-style)
	          whitespace-style
	        (list whitespace-style)))
  ;; turn on whitespace
  (when whitespace-active-style
    (whitespace-color-on)
    (whitespace-display-char-on)))

(defun whitespace-turn-off ()
  "Turn off whitespace visualization."
  (remove-hook 'write-file-functions #'whitespace-write-file-hook t)
  (when whitespace-active-style
    (whitespace-color-off)
    (whitespace-display-char-off)))

(defun whitespace-style-face-p ()
  "Return t if there is some visualization via face."
  (and (memq 'face whitespace-active-style)
       (or (memq 'tabs                    whitespace-active-style)
	   (memq 'spaces                  whitespace-active-style)
	   (memq 'trailing                whitespace-active-style)
	   (memq 'lines                   whitespace-active-style)
	   (memq 'lines-tail              whitespace-active-style)
	   (memq 'lines-char              whitespace-active-style)
	   (memq 'newline                 whitespace-active-style)
           (memq 'page-delimiters         whitespace-active-style)
	   (memq 'empty                   whitespace-active-style)
	   (memq 'indentation             whitespace-active-style)
	   (memq 'indentation::tab        whitespace-active-style)
	   (memq 'indentation::space      whitespace-active-style)
	   (memq 'big-indent              whitespace-active-style)
	   (memq 'space-after-tab         whitespace-active-style)
	   (memq 'space-after-tab::tab    whitespace-active-style)
	   (memq 'space-after-tab::space  whitespace-active-style)
	   (memq 'space-before-tab        whitespace-active-style)
	   (memq 'space-before-tab::tab   whitespace-active-style)
	   (memq 'space-before-tab::space whitespace-active-style))
       t))

(defun whitespace--clone ()
  "Hook function run after `make-indirect-buffer' and `clone-buffer'."
  (when (whitespace-style-face-p)
    (setq-local whitespace-bob-marker
                (copy-marker (marker-position whitespace-bob-marker)
                             (marker-insertion-type whitespace-bob-marker)))
    (setq-local whitespace-eob-marker
                (copy-marker (marker-position whitespace-eob-marker)
                             (marker-insertion-type whitespace-eob-marker)))))

(defun whitespace-color-on ()
  "Turn on color visualization."
  (when (whitespace-style-face-p)
    ;; save current point and refontify when necessary
    (setq-local whitespace-point (point))
    (setq whitespace-point--used
          (let ((ol (make-overlay (point) (point) nil nil t)))
            (delete-overlay ol) ol))
    (setq-local whitespace-bob-marker (point-min-marker))
    (setq-local whitespace-eob-marker (point-max-marker))
    (whitespace--update-bob-eob)
    (setq-local whitespace-buffer-changed nil)
    (add-hook 'post-command-hook #'whitespace-post-command-hook nil t)
    (add-hook 'after-change-functions #'whitespace--update-bob-eob
              ;; The -1 ensures that it runs before any
              ;; `font-lock-mode' hook functions.
              -1 t)
    (add-hook 'clone-buffer-hook #'whitespace--clone nil t)
    (add-hook 'clone-indirect-buffer-hook #'whitespace--clone nil t)
    ;; Add whitespace-mode color into font lock.
    (setq
     whitespace-font-lock-keywords
     `(
       (whitespace-point--flush-used)
       ,@(when (memq 'spaces whitespace-active-style)
           ;; Show SPACEs.
           `((,whitespace-space-regexp 1 whitespace-space t)
             ;; Show HARD SPACEs.
             (,whitespace-hspace-regexp 1 whitespace-hspace t)))
       ,@(when (memq 'tabs whitespace-active-style)
           ;; Show TABs.
           `((,whitespace-tab-regexp 1 whitespace-tab t)))
       ,@(when (memq 'trailing whitespace-active-style)
           ;; Show trailing blanks.
           `((,#'whitespace-trailing-regexp 1 whitespace-trailing t)))
       ,@(when (or (memq 'lines      whitespace-active-style)
                   (memq 'lines-tail whitespace-active-style)
                   (memq 'lines-char whitespace-active-style))
           ;; Show "long" lines.
           `((,#'whitespace-lines-regexp
              ,(cond
                ;; whole line
                ((memq 'lines whitespace-active-style) 0)
                ;; line tail
                ((memq 'lines-tail whitespace-active-style) 2)
                ;; first overflowing character
                ((memq 'lines-char whitespace-active-style) 3))
              whitespace-line prepend)))
       ,@(when (memq 'page-delimiters whitespace-active-style)
           (unless (and (memq 'display font-lock-extra-managed-props)
                        (memq 'display-line-numbers-disable font-lock-extra-managed-props))
             (setq-local font-lock-extra-managed-props
                         `(,@font-lock-extra-managed-props display display-line-numbers-disable)))
           ;; Show page delimiters characters
           whitespace--page-delimiters-keyword)
       ,@(when (or (memq 'space-before-tab whitespace-active-style)
                   (memq 'space-before-tab::tab whitespace-active-style)
                   (memq 'space-before-tab::space whitespace-active-style))
           `((,whitespace-space-before-tab-regexp
              ,(cond
                ((memq 'space-before-tab whitespace-active-style)
                 ;; Show SPACEs before TAB (indent-tabs-mode).
                 (if indent-tabs-mode 1 2))
                ((memq 'space-before-tab::tab whitespace-active-style)
                 1)
                ((memq 'space-before-tab::space whitespace-active-style)
                 2))
              whitespace-space-before-tab t)))
       ,@(when (or (memq 'indentation whitespace-active-style)
                   (memq 'indentation::tab whitespace-active-style)
                   (memq 'indentation::space whitespace-active-style))
           `((,#'whitespace--indentation-matcher
              1 whitespace-indentation t)))
       ,@(when (memq 'big-indent whitespace-active-style)
           ;; Show big indentation.
           `((,whitespace-big-indent-regexp 1 'whitespace-big-indent t)))
       ,@(when (memq 'empty whitespace-active-style)
           ;; Show empty lines at beginning of buffer.
           `((,#'whitespace--empty-at-bob-matcher
              0 whitespace-empty t)
             ;; Show empty lines at end of buffer.
             (,#'whitespace--empty-at-eob-matcher
              0 whitespace-empty t)))
       ,@(when (or (memq 'space-after-tab whitespace-active-style)
                   (memq 'space-after-tab::tab whitespace-active-style)
                   (memq 'space-after-tab::space whitespace-active-style))
           `((,(cond
                ((memq 'space-after-tab whitespace-active-style)
                 ;; Show SPACEs after TAB (indent-tabs-mode).
                 (whitespace-space-after-tab-regexp))
                ((memq 'space-after-tab::tab whitespace-active-style)
                 ;; Show SPACEs after TAB (SPACEs).
                 (whitespace-space-after-tab-regexp 'tab))
                ((memq 'space-after-tab::space whitespace-active-style)
                 ;; Show SPACEs after TAB (TABs).
                 (whitespace-space-after-tab-regexp 'space)))
              1 whitespace-space-after-tab t)))
       ,@(when (memq 'missing-newline-at-eof whitespace-active-style)
           ;; Show missing newline.
           `((".\\'" 0
              ;; Don't mark the end of the buffer if point is there --
              ;; it probably means that the user is typing something
              ;; at the end of the buffer.
              (and (/= whitespace-point (point-max))
                   'whitespace-missing-newline-at-eof)
              prepend)))))
    (font-lock-add-keywords nil whitespace-font-lock-keywords 'append)
    (font-lock-flush)))

(defun whitespace-color-off ()
  "Turn off color visualization."
  ;; turn off font lock
  (kill-local-variable 'whitespace-point--used)
  (when (whitespace-style-face-p)
    (remove-hook 'post-command-hook #'whitespace-post-command-hook t)
    (remove-hook 'after-change-functions #'whitespace--update-bob-eob
                 t)
    (remove-hook 'clone-buffer-hook #'whitespace--clone t)
    (remove-hook 'clone-indirect-buffer-hook #'whitespace--clone t)
    (font-lock-remove-keywords nil whitespace-font-lock-keywords)
    (font-lock-flush)))

(defun whitespace-point--used (start end)
  (let ((ostart (overlay-start whitespace-point--used)))
    (if ostart
        (move-overlay whitespace-point--used
                      (min start ostart)
                      (max end (overlay-end whitespace-point--used)))
      (move-overlay whitespace-point--used start end))))

(defun whitespace-point--flush-used (limit)
  (let ((ostart (overlay-start whitespace-point--used)))
    ;; Strip parts of whitespace-point--used we're about to refresh.
    (when ostart
      (let ((oend (overlay-end whitespace-point--used)))
        (if (<= (point) ostart)
            (if (<= oend limit)
                (delete-overlay whitespace-point--used)
              (move-overlay whitespace-point--used limit oend)))
        (if (<= oend limit)
            (move-overlay whitespace-point--used ostart (point))))))
  nil)

(defun whitespace-trailing-regexp (limit)
  "Match trailing spaces which do not contain the point at end of line."
  (let ((status t))
    (while (if (re-search-forward whitespace-trailing-regexp limit t)
	       (when (= whitespace-point (match-end 1)) ; Loop if point at eol.
                 (whitespace-point--used (match-beginning 0) (match-end 0))
                 t)
	     (setq status nil)))		  ;; end of buffer
    status))

(defun whitespace-lines-regexp (limit)
  (re-search-forward
   (let ((line-column (or whitespace-line-column fill-column)))
     (format
      "^\\([^\t\n]\\{%s\\}\\|[^\t\n]\\{0,%s\\}\t\\)\\{%d\\}%s\\(?2:\\(?3:.\\).*\\)$"
      tab-width
      (1- tab-width)
      (/ line-column tab-width)
      (let ((rem (% line-column tab-width)))
        (if (zerop rem)
            ""
          (format ".\\{%d\\}" rem)))))
   limit t))

(defun whitespace--empty-at-bob-matcher (limit)
  "Match empty/space-only lines at beginning of buffer (BoB)."
  (let ((p (point))
        (e (min whitespace-bob-marker limit
                ;; EoB marker will be before BoB marker if the buffer
                ;; has nothing but empty lines.
                whitespace-eob-marker
                (save-excursion (goto-char whitespace-point)
                                (line-beginning-position)))))
    (when (= p (point-min))
      (with-silent-modifications
        ;; See the comment in `whitespace--update-bob-eob' for why
        ;; this text property is added here.
        (put-text-property (point-min) whitespace-bob-marker
                           'font-lock-multiline t)))
    (when (< p e)
      (set-match-data (list p e))
      (goto-char e))))

(defsubst whitespace--looking-back (regexp)
  (save-excursion
    (when (/= 0 (skip-chars-backward " \t\n"))
      (unless (bolp)
	(forward-line 1))
      (looking-at regexp))))

(defun whitespace--empty-at-eob-matcher (limit)
  "Match empty/space-only lines at end of buffer (EoB)."
  (when (= limit (point-max))
    (with-silent-modifications
      ;; See the comment in `whitespace--update-bob-eob' for why this
      ;; text property is added here.
      (put-text-property whitespace-eob-marker limit
                         'font-lock-multiline t)))
  (let ((b (max (point) whitespace-eob-marker
                whitespace-bob-marker ; See comment in the bob func.
                (save-excursion (goto-char whitespace-point)
                                (forward-line 1)
                                (point)))))
    (when (< b limit)
      (set-match-data (list b limit))
      (goto-char limit))))

(defun whitespace-post-command-hook ()
  "Save current point into `whitespace-point' variable.
Also refontify when necessary."
  (when (or (not (eq whitespace-point (point)))
            whitespace-buffer-changed)
    (when (and (not whitespace-buffer-changed)
               (memq 'empty whitespace-active-style))
      ;; No need to handle the `whitespace-buffer-changed' case here
      ;; because that is taken care of by the `font-lock-multiline'
      ;; text property.
      (when (<= (min (point) whitespace-point) whitespace-bob-marker)
        (font-lock-flush (point-min) whitespace-bob-marker))
      (when (>= (max (point) whitespace-point) whitespace-eob-marker)
        (font-lock-flush whitespace-eob-marker (point-max))))
    (setq-local whitespace-buffer-changed nil)
    (setq whitespace-point (point))	; current point position
    (let ((refontify (or (and (eolp) ; It is at end of line ...
                              ;; ... with trailing SPACE or TAB
                              (or (memq (preceding-char) '(?\s ?\t)))
                              (line-beginning-position))
                         (and (memq 'missing-newline-at-eof
                                    ;; If user requested to highlight
                                    ;; EOB without a newline...
                                    whitespace-active-style)
                              ;; ...and the buffer is not empty...
                              (not (= (point-min) (point-max)))
                              (= (point-max) (without-restriction (point-max)))
                              ;; ...and no newline at EOB...
                              (not (eq (char-before (point-max)) ?\n))
                              ;; ...then refontify the last character in
                              ;; the buffer
                              (max (1- (point-max)) (point-min)))))
          (ostart (overlay-start whitespace-point--used)))
      (cond
       ((not refontify)
        ;; New point does not affect highlighting: just refresh the
        ;; highlighting of old point, if needed.
        (when ostart
          (font-lock-flush ostart
                           (overlay-end whitespace-point--used))
          (delete-overlay whitespace-point--used)))
       ((not ostart)
        ;; Old point did not affect highlighting, but new one does: refresh the
        ;; highlighting of new point.
        (font-lock-flush (min refontify (point)) (max refontify (point))))
       ((save-excursion
          (goto-char ostart)
          (setq ostart (line-beginning-position))
          (and (<= ostart (max refontify (point)))
               (progn
                 (goto-char (overlay-end whitespace-point--used))
                 (let ((oend (line-beginning-position 2)))
                   (<= (min refontify (point)) oend)))))
        ;; The old point highlighting and the new point highlighting
        ;; cover a contiguous region: do a single refresh.
        (font-lock-flush (min refontify (point) ostart)
                         (max refontify (point)
                              (overlay-end whitespace-point--used)))
        (delete-overlay whitespace-point--used))
       (t
        (font-lock-flush (min refontify (point))
                         (max refontify (point)))
        (font-lock-flush ostart (overlay-end whitespace-point--used))
        (delete-overlay whitespace-point--used))))))

(defun whitespace--indentation-matcher (limit)
  "Indentation matcher for `font-lock-keywords'."
  (re-search-forward
   (whitespace-indentation-regexp
    (cond
     ((memq 'indentation whitespace-active-style) nil)
     ((memq 'indentation::tab whitespace-active-style) 'tab)
     ((memq 'indentation::space whitespace-active-style) 'space)))
   limit t))

(defun whitespace--variable-watcher (_symbol _newval _op buffer)
  "Variable watcher that calls `font-lock-flush' for BUFFER."
  (when buffer
    (with-current-buffer buffer
      (when whitespace-mode
        (font-lock-flush)))))

(defun whitespace--update-bob-eob (&optional beg end &rest _)
  "Update `whitespace-bob-marker' and `whitespace-eob-marker'.
Also apply `font-lock-multiline' text property."
  (setq whitespace-buffer-changed t)
  (when (memq 'empty whitespace-active-style)
    (save-excursion
      (save-restriction
        (widen)
        (let ((inhibit-read-only t))
          (when (or (null beg)
                    (<= beg (save-excursion
                              (goto-char whitespace-bob-marker)
                              ;; Any change in the first non-`empty'
                              ;; line, even if it's not the first
                              ;; character in the line, can potentially
                              ;; cause subsequent lines to become
                              ;; classified as `empty'.
                              (forward-line 1)
                              (point))))
            (goto-char (point-min))
            (set-marker whitespace-bob-marker (point))
            (save-match-data
              (when (looking-at whitespace-empty-at-bob-regexp)
                (set-marker whitespace-bob-marker (match-end 1))
                (with-silent-modifications
                  (put-text-property (match-beginning 1) (match-end 1)
                                     'font-lock-multiline t)))))
          (when (or (null end)
                    (>= end (save-excursion
                              (goto-char whitespace-eob-marker)
                              ;; See above comment for the BoB case.
                              (forward-line -1)
                              (point))))
            (goto-char (point-max))
            (set-marker whitespace-eob-marker (point))
            (save-match-data
              (when (whitespace--looking-back
                     whitespace-empty-at-eob-regexp)
                (set-marker whitespace-eob-marker (match-beginning 1))
                (with-silent-modifications
                  (put-text-property (match-beginning 1) (match-end 1)
                                     'font-lock-multiline t))))))))))

(defun whitespace-style-mark-p ()
  "Return t if there is some visualization via display table."
  (and (or (memq 'tab-mark     whitespace-active-style)
           (memq 'space-mark   whitespace-active-style)
           (memq 'newline-mark whitespace-active-style))
       t))

(defsubst whitespace-char-valid-p (char)
  (or (< char 256)
      (characterp char)))

(defun whitespace-display-vector-p (vec)
  "Return non-nil if every character in vector VEC can be displayed."
  (let ((i (length vec)))
    (when (> i 0)
      (while (and (>= (setq i (1- i)) 0)
		  (whitespace-char-valid-p (glyph-char (aref vec i)))))
      (< i 0))))

(defun whitespace-display-char-on ()
  "Turn on character display mapping."
  (when (and whitespace-display-mappings
	     (whitespace-style-mark-p))
    (let (vecs vec)
      ;; Remember whether a buffer has a local display table.
      (unless whitespace-display-table-was-local
	(setq whitespace-display-table-was-local t)
        ;; Save the old table so we can restore it when
        ;; `whitespace-mode' is switched off again.
        (when whitespace-mode
	  (setq whitespace-display-table
	        (copy-sequence buffer-display-table)))
	;; Assure `buffer-display-table' is unique
	;; when two or more windows are visible.
	(setq buffer-display-table
	      (copy-sequence (or buffer-display-table
                                 standard-display-table))))
      (unless buffer-display-table
	(setq buffer-display-table (make-display-table)))
      (dolist (entry whitespace-display-mappings)
	;; check if it is to display this mark
	(when (memq (car entry) whitespace-style)
	  ;; Get a displayable mapping.
	  (setq vecs (cddr entry))
	  (while (and vecs
		      (not (whitespace-display-vector-p (car vecs))))
	    (setq vecs (cdr vecs)))
	  ;; Display a valid mapping.
	  (when vecs
	    (setq vec (copy-sequence (car vecs)))
	    ;; NEWLINE char
	    (when (and (eq (cadr entry) ?\n)
		       (memq 'newline whitespace-active-style))
	      ;; Only insert face bits on NEWLINE char mapping to avoid
	      ;; obstruction of other faces like TABs and (HARD) SPACEs
	      ;; faces, font-lock faces, etc.
	      (dotimes (i (length vec))
		(or (eq (aref vec i) ?\n)
		    (aset vec i
			  (make-glyph-code (aref vec i)
					   'whitespace-newline)))))
	    ;; Display mapping
	    (aset buffer-display-table (cadr entry) vec)))))))

(defun whitespace-display-char-off ()
  "Turn off character display mapping."
  (and whitespace-display-mappings
       (whitespace-style-mark-p)
       whitespace-display-table-was-local
       (setq whitespace-display-table-was-local nil
	     buffer-display-table whitespace-display-table)))

(defun whitespace-action-when-on ()
  "Action to be taken always when local whitespace is turned on."
  (cond ((memq 'cleanup whitespace-action)
	 (whitespace-cleanup))
	((memq 'report-on-bogus whitespace-action)
	 (whitespace-report nil t))))

(defun whitespace-write-file-hook ()
  "Action to be taken when buffer is written."
  (cond ((memq 'auto-cleanup whitespace-action)
	 (whitespace-cleanup))
	((memq 'abort-on-bogus whitespace-action)
	 (when (whitespace-report nil t)
	   (error "Abort write due to whitespace problems in %s"
		  (buffer-name)))))
  nil)					; continue hook processing

(defvar whitespace--watched-vars
  '(fill-column indent-tabs-mode tab-width whitespace-line-column))

(dolist (var whitespace--watched-vars)
  (add-variable-watcher var #'whitespace--variable-watcher))

(provide 'whitespace)
