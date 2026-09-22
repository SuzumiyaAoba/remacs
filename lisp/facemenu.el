;;; facemenu.el --- face text-menu keymaps -*- lexical-binding: t -*-

;; Subset of GNU's facemenu.el: just the menu keymaps the global-map
;; autoload needs.

(fset 'facemenu-menu
  '(keymap (fc "Face" . facemenu-face-menu) (fg "Foreground Color" . facemenu-foreground-menu) (bg "Background Color" . facemenu-background-menu) (sp "Special Properties" . facemenu-special-menu) (s2 "--") (ju "Justification" . facemenu-justification-menu) (in "Indentation" . facemenu-indentation-menu) (s1 "--") (rm menu-item "Remove Face Properties" facemenu-remove-face-props :enable mark-active) (ra menu-item "Remove Text Properties" facemenu-remove-all :enable mark-active) (dp "Describe Properties" . describe-text-properties) (df "Display Faces" . list-faces-display) (dc "Display Colors" . list-colors-display) "Text Properties"))

(fset 'facemenu-face-menu
  '(keymap (100 "default" . facemenu-set-default) (98 "bold" . facemenu-set-bold) (105 "italic" . facemenu-set-italic) (108 "bold-italic" . facemenu-set-bold-italic) (117 "underline" . facemenu-set-underline) (111 "Other..." . facemenu-set-face) "Face"))

(fset 'facemenu-foreground-menu
  '(keymap (111 "Other..." . facemenu-set-foreground) "Foreground Color"))

(fset 'facemenu-background-menu
  '(keymap (111 "Other..." . facemenu-set-background) "Background Color"))

(fset 'facemenu-special-menu
  '(keymap (114 "Read-Only" . facemenu-set-read-only) (118 "Invisible" . facemenu-set-invisible) (116 "Intangible" . facemenu-set-intangible) (99 "Charset" . facemenu-set-charset) (115 "Remove Special" . facemenu-remove-special) "Special"))

(fset 'facemenu-justification-menu
  '(keymap (117 "Unfilled" . set-justification-none) (108 "Left" . set-justification-left) (114 "Right" . set-justification-right) (98 "Full" . set-justification-full) (99 "Center" . set-justification-center) "Justification"))

(fset 'facemenu-indentation-menu
  '(keymap (increase-left-margin "Indent More" . increase-left-margin) (decrease-left-margin "Indent Less" . decrease-left-margin) (increase-right-margin "Indent Right More" . increase-right-margin) (decrease-right-margin "Indent Right Less" . decrease-right-margin) "Indentation"))

(provide 'facemenu)
