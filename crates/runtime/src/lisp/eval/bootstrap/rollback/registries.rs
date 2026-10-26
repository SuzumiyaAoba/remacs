//! Restore GNU minor-mode registry ordering and deferred rectangle definitions.

use crate::lisp::eval::Interp;

pub(super) fn restore(interp: &mut Interp) {
    // The rolled-back loads also pushed `minor-mode-map-alist'
    // entries (rect.el's `define-minor-mode' has a :keymap);
    // GNU's -Q alist doesn't have them.
    let _ = interp.eval_str(
        "(progn \
                   (set-default 'minor-mode-map-alist \
                       (delq (assq 'rectangle-mark-mode \
                                   (default-value 'minor-mode-map-alist)) \
                             (default-value 'minor-mode-map-alist))) \
                   (set-default 'minor-mode-alist \
                       (delq (assq 'rectangle-mark-mode \
                                   (default-value 'minor-mode-alist)) \
                             (default-value 'minor-mode-alist))) \
                   (setq minor-mode-map-alist \
                         (delq (assq 'rectangle-mark-mode \
                                     minor-mode-map-alist) \
                               minor-mode-map-alist) \
                         minor-mode-alist \
                         (delq (assq 'rectangle-mark-mode \
                                     minor-mode-alist) \
                               minor-mode-alist)))",
    );
    // GNU's -Q ordering of the minor-mode registries reflects
    // loadup evaluation order; our folded prelude registers
    // them in a different sequence, and prelude folds of
    // non-dumped files (face-remap, elec-pair, delsel,
    // battery, time) left extra `minor-mode-list' entries
    // while hand-written defuns (read-only-mode,
    // transient-mark-mode, use-hard-newlines) never pushed
    // theirs.  GNU's -Q list additionally carries
    // `cl-old-struct-compat-mode' at the head (pushed when
    // cl-lib's define-minor-mode ran during the dump; its
    // function cell is still the cl-lib autoload), so it
    // heads the literal list below.
    // Reproduce GNU's canonical dump state exactly: reorder
    // the alists (entry order = keymap priority) and set
    // `minor-mode-list' to GNU's literal -Q value.  The alists
    // are always-local, so fix both default and current
    // bindings.
    let _ = interp.eval_str(
        "(progn
                   (let ((order '(tab-bar-history-mode tab-bar-mode
                                  mouse-shift-adjust-mode context-menu-mode
                                  visual-line-mode read-extended-command-mode
                                  button-mode isearch-mode))
                         (reorder (lambda (list order)
                                    (let ((head (delq nil
                                                 (mapcar (lambda (s) (assq s list))
                                                         order)))
                                          (tail nil))
                                      (dolist (e list)
                                        (unless (memq (car e) order)
                                          (push e tail)))
                                      (nconc head (nreverse tail))))))
                     (set-default 'minor-mode-map-alist
                         (funcall reorder (default-value 'minor-mode-map-alist)
                                  order))
                     (setq minor-mode-map-alist
                           (funcall reorder minor-mode-map-alist order)))
                   (let ((order '(eldoc-mode center-line-mode
                                  isearch-fold-quotes-mode visible-mode
                                  visual-line-mode next-error-follow-minor-mode
                                  abbrev-mode overwrite-mode
                                  auto-fill-function defining-kbd-macro
                                  isearch-mode))
                         (reorder (lambda (list order)
                                    (let ((head (delq nil
                                                 (mapcar (lambda (s) (assq s list))
                                                         order)))
                                          (tail nil))
                                      (dolist (e list)
                                        (unless (memq (car e) order)
                                          (push e tail)))
                                      (nconc head (nreverse tail))))))
                     (set-default 'minor-mode-alist
                         (funcall reorder (default-value 'minor-mode-alist)
                                  order))
                     (setq minor-mode-alist
                           (funcall reorder minor-mode-alist order)))
                   (let ((order '(tooltip-mode global-eldoc-mode
                                  show-paren-mode electric-indent-mode
                                  file-name-shadow-mode
                                  minibuffer-regexp-mode
                                  minibuffer-nonselected-mode line-number-mode
                                  auto-encryption-mode auto-compression-mode
                                  windmove-mode gpm-mouse-mode
                                  mouse-wheel-mode))
                         (tail nil))
                     (setq global-minor-modes
                           (nconc (delq nil
                                   (mapcar (lambda (s)
                                             (and (memq s global-minor-modes) s))
                                           order))
                                  (progn
                                    (dolist (s global-minor-modes)
                                      (unless (memq s order) (push s tail)))
                                    (nreverse tail)))))
                   (setq minor-mode-list
                         '(cl-old-struct-compat-mode
                           tooltip-mode global-eldoc-mode eldoc-mode
                           show-paren-mode electric-quote-mode
                           electric-layout-mode electric-indent-mode
                           vc-auto-revert-mode mouse-wheel-mode
                           modifier-bar-mode tool-bar-mode center-line-mode
                           paragraph-indent-minor-mode
                           global-prettify-symbols-mode prettify-symbols-mode
                           use-hard-newlines tab-bar-history-mode tab-bar-mode
                           menu-bar-mode file-name-shadow-mode
                           isearch-fold-quotes-mode lost-selection-mode
                           horizontal-scroll-bar-mode mouse-shift-adjust-mode
                           context-menu-mode jit-lock-debug-mode
                           global-font-lock-mode font-lock-mode
                           blink-cursor-mode window-divider-mode
                           undelete-frame-mode minibuffer-nonselected-mode
                           minibuffer-regexp-mode completion-in-region-mode
                           visible-mode buffer-read-only
                           mode-line-invisible-mode size-indication-mode
                           column-number-mode line-number-mode
                           auto-fill-function global-visual-line-mode
                           visual-line-mode indent-tabs-mode
                           transient-mark-mode cursor-face-highlight-mode
                           read-extended-command-mode
                           delete-trailing-whitespace-mode
                           next-error-follow-minor-mode auto-composition-mode
                           auto-encryption-mode auto-compression-mode
                           temp-buffer-resize-mode button-mode lock-file-mode
                           auto-save-visited-mode auto-save-mode
                           auto-fill-mode abbrev-mode overwrite-mode
                           view-mode hs-minor-mode)))",
    );
    let _ = interp.eval_str(
                "(progn
  (fset 'clear-rectangle '(autoload \"rect\" \"Blank out the region-rectangle.
The text previously in the region is overwritten with blanks.

When called from a program the rectangle's corners are START and END.
With a prefix (or a FILL) argument, also fill with blanks the parts of the
rectangle which were empty.

(fn START END &optional FILL)\" t nil))
  (fset 'copy-rectangle-as-kill '(autoload \"rect\" \"Copy the region-rectangle and save it as the last killed one.

(fn START END)\" t nil))
  (fset 'delete-extract-rectangle '(autoload \"rect\" \"Delete the contents of the rectangle with corners at START and END.
Return it as a list of strings, one for each line of the rectangle.

When called from a program the rectangle's corners are START and END.
With an optional FILL argument, also fill lines where nothing has to be
deleted.

(fn START END &optional FILL)\" nil nil))
  (fset 'delete-rectangle '(autoload \"rect\" \"Delete (don't save) text in the region-rectangle.
The same range of columns is deleted in each line starting with the
line where the region begins and ending with the line where the region
ends.

When called from a program the rectangle's corners are START and END.
With a prefix (or a FILL) argument, also fill lines where nothing has
to be deleted.

(fn START END &optional FILL)\" t nil))
  (fset 'delete-whitespace-rectangle '(autoload \"rect\" \"Delete all whitespace following a specified column in each line.
The left edge of the rectangle specifies the position in each line
at which whitespace deletion should begin.  On each line in the
rectangle, all contiguous whitespace starting at that column is deleted.

When called from a program the rectangle's corners are START and END.
With a prefix (or a FILL) argument, also fill too short lines.

(fn START END &optional FILL)\" t nil))
  (fset 'extract-rectangle '(autoload \"rect\" \"Return the contents of the rectangle with corners at START and END.
Return it as a list of strings, one for each line of the rectangle.

(fn START END)\" nil nil))
  (fset 'insert-rectangle '(autoload \"rect\" \"Insert text of RECTANGLE with upper left corner at point.
RECTANGLE's first line is inserted at point, its second
line is inserted at a point vertically under point, etc.
RECTANGLE should be a list of strings.
After this command, the mark is at the upper left corner
and point is at the lower right corner.

(fn RECTANGLE)\" nil nil))
  (fset 'kill-rectangle '(autoload \"rect\" \"Delete the region-rectangle and save it as the last killed one.

When called from a program the rectangle's corners are START and END.
You might prefer to use `delete-extract-rectangle' from a program.

With a prefix (or a FILL) argument, also fill lines where nothing has to be
deleted.

If the buffer is read-only, Emacs will beep and refrain from deleting
the rectangle, but put it in `killed-rectangle' anyway.  This means that
you can use this command to copy text from a read-only buffer.
(If the variable `kill-read-only-ok' is non-nil, then this won't
even beep.)

(fn START END &optional FILL)\" t nil))
  (fset 'open-rectangle '(autoload \"rect\" \"Blank out the region-rectangle, shifting text right.

The text previously in the region is not overwritten by the blanks,
but instead winds up to the right of the rectangle.

When called from a program the rectangle's corners are START and END.
With a prefix (or a FILL) argument, fill with blanks even if there is
no text on the right side of the rectangle.

(fn START END &optional FILL)\" t nil))
  (fset 'rectangle-mark-mode '(autoload \"rect\" \"Toggle the region as rectangular.

Activates the region if it's inactive and Transient Mark mode is
on.  Only lasts until the region is next deactivated.

This is a minor mode.  If called interactively, toggle the
`Rectangle-Mark mode' mode.  If the prefix argument is positive, enable
the mode, and if it is zero or negative, disable the mode.

If called from Lisp, toggle the mode if ARG is `toggle'.  Enable the
mode if ARG is nil, omitted, or is a positive number.  Disable the mode
if ARG is a negative number.

To check whether the minor mode is enabled in the current buffer,
evaluate the variable `rectangle-mark-mode'.

The mode's hook is called both when the mode is enabled and when it is
disabled.

\\\\{rectangle-mark-mode-map}

(fn &optional ARG)\" t nil))
  (fset 'rectangle-number-lines '(autoload \"rect\" \"Insert numbers in front of the region-rectangle.

START-AT, if non-nil, should be a number from which to begin
counting.  FORMAT, if non-nil, should be a format string to pass
to `format' along with the line count.  When called interactively
with a prefix argument, prompt for START-AT and FORMAT.

(fn START END START-AT &optional FORMAT)\" t nil))
  (fset 'string-insert-rectangle '(autoload \"rect\" \"Insert STRING on each line of region-rectangle, shifting text right.

When called from a program, the rectangle's corners are START and END.
The left edge of the rectangle specifies the column for insertion.
This command does not delete or overwrite any existing text.

(fn START END STRING)\" t nil))
  (fset 'string-rectangle '(autoload \"rect\" \"Replace rectangle contents with STRING on each line.
The length of STRING need not be the same as the rectangle width.

When called interactively and option `rectangle-preview' is
non-nil, display the result as the user enters the string into
the minibuffer.

Called from a program, takes three args; START, END and STRING.

(fn START END STRING)\" t nil))
  (fset 'yank-rectangle '(autoload \"rect\" \"Yank the last killed rectangle with upper left corner at point.\" t nil))
  (fset 'send-to--resolve-handler '(autoload \"send-to\" nil nil nil))
  (fset 'send-to-supported-p '(autoload \"send-to\" \"Return non-nil for platforms where `send-to' is supported.\" nil nil))
  (fset 'send-to '(autoload \"send-to\" \"Send file(s) or region text to (non-Emacs) applications or services.

Sending is handled by the first supported handler from `send-to-handlers'.

ITEMS list is also populated by the resolved handler, but can be
explicitly overridden.

(fn &optional ITEMS)\" t nil)) \
  (fset 'close-rectangle 'delete-whitespace-rectangle) \
  (fset 'replace-rectangle 'string-rectangle))",
            );
    // rect.el's `rectangle-preview' defface must stay
    // unloaded too.
    let rectangle = interp.intern_soft("rectangle-preview");
    interp.face_table.retain(|(face, _)| Some(*face) != rectangle);
}
