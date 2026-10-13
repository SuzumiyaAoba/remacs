;;; -*- lexical-binding: t -*-
;; Differential-sweep face/fontset/frame regressions — every check
;; below was verified against GNU Emacs 31.1 -Q --batch before
;; pinning (the sweep's DIVERG rows these fix are noted per block).
(require 'qa-framework nil t)

;; == set-face-attribute empty-ARGS early return ==
;; GNU Fset_face_attribute1 returns nil before validating FACE when
;; the ARGS tail is empty.
(qa-err (set-face-attribute 'a) 'wrong-number-of-arguments)
(qa-err (set-face-attribute "a") 'wrong-number-of-arguments)
(qa (set-face-attribute 'nosuch-face nil) nil)  ; face never checked
(qa-err (set-face-attribute 'nosuch-face nil :foreground "x") 'error)
(qa-err-data (set-face-attribute 'nosuch-face nil :foreground "x")
             'error '("Invalid face" nosuch-face))

;; == check_face_attr value validation (xfaces.c) ==
;; color attrs: CHECK_STRING
(qa-err-data (set-face-attribute 'default nil :foreground 'in)
             'wrong-type-argument '(stringp in))
(qa-err-data (set-face-attribute 'default nil :background 'in)
             'wrong-type-argument '(stringp in))
;; boolean attrs accept nil/t (incl. :inverse-video nil — this fired
;; the tab-bar boot error before the fix)
(qa (set-face-attribute 'default nil :inverse-video nil) nil)
(qa (set-face-attribute 'default nil :inverse-video t) nil)
(qa (set-face-attribute 'default nil :extend nil) nil)
(qa (set-face-attribute 'default nil :underline t) nil)
(qa (set-face-attribute 'default nil :underline '(:color "red")) nil)
(qa-err-data (set-face-attribute 'default nil :inverse-video 'in)
             'error '("Invalid inverse-video face attribute value in"))
(qa-err-data (set-face-attribute 'default nil :extend 'in)
             'error '("Invalid extend face attribute value in"))
(qa-err-data (set-face-attribute 'default nil :overline 'in)
             'error '("Invalid face overline in"))
(qa-err-data (set-face-attribute 'default nil :strike-through 'in)
             'error '("Invalid face strike-through in"))
(qa-err-data (set-face-attribute 'default nil :underline 'in)
             'error '("Invalid face underline in"))
;; symbolic metric attrs
(qa-err (set-face-attribute 'default nil :weight 100) 'wrong-type-argument)
(qa-err-data (set-face-attribute 'default nil :weight 'in)
             'error '("Invalid face weight in"))
(qa-err-data (set-face-attribute 'default nil :slant 'in)
             'error '("Invalid face slant in"))
(qa-err-data (set-face-attribute 'default nil :slant 'roman)
             'error '("Invalid face slant roman"))
(qa-err-data (set-face-attribute 'default nil :width 'in)
             'error '("Invalid face width in"))
(qa (set-face-attribute 'default nil :weight 'bold) nil)
(qa (set-face-attribute 'default nil :slant 'italic) nil)
(qa (set-face-attribute 'default nil :width 'condensed) nil)
;; height must be a positive number (default face: absolute)
(qa-err-data (set-face-attribute 'default nil :height 'in)
             'error '("Default face height not absolute and positive in"))
(qa-err-data (set-face-attribute 'default nil :height 0)
             'error '("Default face height not absolute and positive 0"))
(qa-err-data (set-face-attribute 'default nil :height "abc")
             'error '("Default face height not absolute and positive \"abc\""))
;; :font / :stipple / :box / :inherit shape checks
(qa-err-data (set-face-attribute 'default nil :font 'in)
             'error '("Invalid font or font-spec in"))
(qa-err-data (set-face-attribute 'default nil :stipple 'in)
             'error '("Invalid stipple attribute in"))
(qa-err-data (set-face-attribute 'default nil :box 'in)
             'error '("Invalid face box in"))
(qa-err-data (set-face-attribute 'default nil :inherit 5)
             'error '("Invalid face inheritance 5"))
;; unspecified/undefined pass for every attribute
(qa (set-face-attribute 'default nil :foreground 'unspecified) nil)
(qa (set-face-attribute 'default nil :slant 'unspecified) nil)
(qa (set-face-attribute 'default nil :height 'unspecified) nil)

;; == set-fontset-font decode order ==
;; FRAME (arg 3) decodes before the fontset-existence check and before
;; TARGET validation.
(qa-err-data (set-fontset-font "nosuch" 97 nil 5)
             'wrong-type-argument '(frame-live-p 5))
(qa-err (set-fontset-font "nosuch" 'BAD nil nil) 'error)
(qa-err-data (set-fontset-font "fontset-default" 'BAD nil 5)
             'wrong-type-argument '(frame-live-p 5))

;; == frame-text-lines / frame-text-height (batch tty: full 25) ==
(qa (frame-text-lines) 25)
(qa (frame-text-height) 25)
(qa (frame-height) 25)

(princ (qa--done))
