;;; shr-color.el --- Simple HTML Renderer color management  -*- lexical-binding:t -*-

;; Copyright (C) 2010-2026 Free Software Foundation, Inc.

;; Author: Julien Danjou <julien@danjou.info>
;; Keywords: html

;; This file is part of GNU Emacs.

;; GNU Emacs is free software: you can redistribute it and/or modify
;; it under the terms of the GNU General Public License as published by
;; the Free Software Foundation, either version 3 of the License, or
;; (at your option) any later version.

;; GNU Emacs is distributed in the hope that it will be useful,
;; but WITHOUT ANY WARRANTY; without even the implied warranty of
;; MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
;; GNU General Public License for more details.

;; You should have received a copy of the GNU General Public License
;; along with GNU Emacs.  If not, see <https://www.gnu.org/licenses/>.

;;; Commentary:

;; This package handles colors display for shr.

;;; Code:

(require 'color)

(defgroup shr-color nil
  "Simple HTML Renderer colors."
  :group 'shr)

(defcustom shr-color-visible-luminance-min 40
  "Minimum luminance distance between two colors to be considered visible.
Must be between 0 and 100."
  :type 'number)

(defcustom shr-color-visible-distance-min 5
  "Minimum color distance between two colors to be considered visible.
This value is used to compare result for `ciede2000'.  It's an
absolute value without any unit."
  :type 'integer)

(defconst shr-color-html-colors-alist
  '(("AliceBlue" . "#F0F8FF")
    ("AntiqueWhite" . "#FAEBD7")
    ("Aqua" . "#00FFFF")
    ("Aquamarine" . "#7FFFD4")
    ("Azure" . "#F0FFFF")
    ("Beige" . "#F5F5DC")
    ("Bisque" . "#FFE4C4")
    ("Black" . "#000000")
    ("BlanchedAlmond" . "#FFEBCD")
    ("Blue" . "#0000FF")
    ("BlueViolet" . "#8A2BE2")
    ("Brown" . "#A52A2A")
    ("BurlyWood" . "#DEB887")
    ("CadetBlue" . "#5F9EA0")
    ("Chartreuse" . "#7FFF00")
    ("Chocolate" . "#D2691E")
    ("Coral" . "#FF7F50")
    ("CornflowerBlue" . "#6495ED")
    ("Cornsilk" . "#FFF8DC")
    ("Crimson" . "#DC143C")
    ("Cyan" . "#00FFFF")
    ("DarkBlue" . "#00008B")
    ("DarkCyan" . "#008B8B")
    ("DarkGoldenRod" . "#B8860B")
    ("DarkGray" . "#A9A9A9")
    ("DarkGrey" . "#A9A9A9")
    ("DarkGreen" . "#006400")
    ("DarkKhaki" . "#BDB76B")
    ("DarkMagenta" . "#8B008B")
    ("DarkOliveGreen" . "#556B2F")
    ("Darkorange" . "#FF8C00")
    ("DarkOrchid" . "#9932CC")
    ("DarkRed" . "#8B0000")
    ("DarkSalmon" . "#E9967A")
    ("DarkSeaGreen" . "#8FBC8F")
    ("DarkSlateBlue" . "#483D8B")
    ("DarkSlateGray" . "#2F4F4F")
    ("DarkSlateGrey" . "#2F4F4F")
    ("DarkTurquoise" . "#00CED1")
    ("DarkViolet" . "#9400D3")
    ("DeepPink" . "#FF1493")
    ("DeepSkyBlue" . "#00BFFF")
    ("DimGray" . "#696969")
    ("DimGrey" . "#696969")
    ("DodgerBlue" . "#1E90FF")
    ("FireBrick" . "#B22222")
    ("FloralWhite" . "#FFFAF0")
    ("ForestGreen" . "#228B22")
    ("Fuchsia" . "#FF00FF")
    ("Gainsboro" . "#DCDCDC")
    ("GhostWhite" . "#F8F8FF")
    ("Gold" . "#FFD700")
    ("GoldenRod" . "#DAA520")
    ("Gray" . "#808080")
    ("Grey" . "#808080")
    ("Green" . "#008000")
    ("GreenYellow" . "#ADFF2F")
    ("HoneyDew" . "#F0FFF0")
    ("HotPink" . "#FF69B4")
    ("IndianRed" . "#CD5C5C")
    ("Indigo" . "#4B0082")
    ("Ivory" . "#FFFFF0")
    ("Khaki" . "#F0E68C")
    ("Lavender" . "#E6E6FA")
    ("LavenderBlush" . "#FFF0F5")
    ("LawnGreen" . "#7CFC00")
    ("LemonChiffon" . "#FFFACD")
    ("LightBlue" . "#ADD8E6")
    ("LightCoral" . "#F08080")
    ("LightCyan" . "#E0FFFF")
    ("LightGoldenRodYellow" . "#FAFAD2")
    ("LightGray" . "#D3D3D3")
    ("LightGrey" . "#D3D3D3")
    ("LightGreen" . "#90EE90")
    ("LightPink" . "#FFB6C1")
    ("LightSalmon" . "#FFA07A")
    ("LightSeaGreen" . "#20B2AA")
    ("LightSkyBlue" . "#87CEFA")
    ("LightSlateGray" . "#778899")
    ("LightSlateGrey" . "#778899")
    ("LightSteelBlue" . "#B0C4DE")
    ("LightYellow" . "#FFFFE0")
    ("Lime" . "#00FF00")
    ("LimeGreen" . "#32CD32")
    ("Linen" . "#FAF0E6")
    ("Magenta" . "#FF00FF")
    ("Maroon" . "#800000")
    ("MediumAquaMarine" . "#66CDAA")
    ("MediumBlue" . "#0000CD")
    ("MediumOrchid" . "#BA55D3")
    ("MediumPurple" . "#9370DB")
    ("MediumSeaGreen" . "#3CB371")
    ("MediumSlateBlue" . "#7B68EE")
    ("MediumSpringGreen" . "#00FA9A")
    ("MediumTurquoise" . "#48D1CC")
    ("MediumVioletRed" . "#C71585")
    ("MidnightBlue" . "#191970")
    ("MintCream" . "#F5FFFA")
    ("MistyRose" . "#FFE4E1")
    ("Moccasin" . "#FFE4B5")
    ("NavajoWhite" . "#FFDEAD")
    ("Navy" . "#000080")
    ("OldLace" . "#FDF5E6")
    ("Olive" . "#808000")
    ("OliveDrab" . "#6B8E23")
    ("Orange" . "#FFA500")
    ("OrangeRed" . "#FF4500")
    ("Orchid" . "#DA70D6")
    ("PaleGoldenRod" . "#EEE8AA")
    ("PaleGreen" . "#98FB98")
    ("PaleTurquoise" . "#AFEEEE")
    ("PaleVioletRed" . "#DB7093")
    ("PapayaWhip" . "#FFEFD5")
    ("PeachPuff" . "#FFDAB9")
    ("Peru" . "#CD853F")
    ("Pink" . "#FFC0CB")
    ("Plum" . "#DDA0DD")
    ("PowderBlue" . "#B0E0E6")
    ("Purple" . "#800080")
    ("RebeccaPurple" . "#663399")
    ("Red" . "#FF0000")
    ("RosyBrown" . "#BC8F8F")
    ("RoyalBlue" . "#4169E1")
    ("SaddleBrown" . "#8B4513")
    ("Salmon" . "#FA8072")
    ("SandyBrown" . "#F4A460")
    ("SeaGreen" . "#2E8B57")
    ("SeaShell" . "#FFF5EE")
    ("Sienna" . "#A0522D")
    ("Silver" . "#C0C0C0")
    ("SkyBlue" . "#87CEEB")
    ("SlateBlue" . "#6A5ACD")
    ("SlateGray" . "#708090")
    ("SlateGrey" . "#708090")
    ("Snow" . "#FFFAFA")
    ("SpringGreen" . "#00FF7F")
    ("SteelBlue" . "#4682B4")
    ("Tan" . "#D2B48C")
    ("Teal" . "#008080")
    ("Thistle" . "#D8BFD8")
    ("Tomato" . "#FF6347")
    ("Turquoise" . "#40E0D0")
    ("Violet" . "#EE82EE")
    ("Wheat" . "#F5DEB3")
    ("White" . "#FFFFFF")
    ("WhiteSmoke" . "#F5F5F5")
    ("Yellow" . "#FFFF00")
    ("YellowGreen" . "#9ACD32"))
  "Alist of HTML colors.
Each entry should have the form (COLOR-NAME . HEXADECIMAL-COLOR).")

(defconst shr-color--css-number-regexp
  "[+-]?\\(?:[0-9]+\\(?:\\.[0-9]+\\)?\\|\\.[0-9]+\\)\\(?:[eE][+-]?[0-9]+\\)?"
  "Regexp matching a signed CSS number, including fractional and exponent forms.")

(defconst shr-color--component-regexp
  (concat "\\(" shr-color--css-number-regexp "\\)")
  "Regexp matching a captured CSS number.")

(defconst shr-color--rgb-component-regexp
  (concat "\\(" shr-color--css-number-regexp
          "\\(?:[[:space:]]*%\\)?\\)")
  "Regexp matching a captured CSS RGB number or percentage.")

(defconst shr-color--modern-rgb-regexp
  (concat "\\(?:rgb\\|rgba\\)([[:space:]]*"
          shr-color--rgb-component-regexp "[[:space:]]+"
          shr-color--rgb-component-regexp "[[:space:]]+"
          shr-color--rgb-component-regexp
          "\\(?:[[:space:]]*/[[:space:]]*\\("
          shr-color--css-number-regexp
          "\\(?:[[:space:]]*%\\)?\\)\\)?[[:space:]]*)")
  "Regexp matching modern space-separated rgb() or rgba() syntax.")

(defconst shr-color--modern-hsl-regexp
  (concat "\\(?:hsl\\|hsla\\)([[:space:]]*"
          shr-color--component-regexp "[[:space:]]+"
          shr-color--rgb-component-regexp "[[:space:]]+"
          shr-color--rgb-component-regexp
          "\\(?:[[:space:]]*/[[:space:]]*\\("
          shr-color--css-number-regexp
          "\\(?:[[:space:]]*%\\)?\\)\\)?[[:space:]]*)")
  "Regexp matching modern space-separated hsl() or hsla() syntax.")

(defconst shr-color--legacy-alpha-regexp
  (concat "\\(?:rgba\\|hsla\\)([[:space:]]*.*,[[:space:]]*\\("
          shr-color--css-number-regexp
          "\\(?:[[:space:]]*%\\)?\\)[[:space:]]*)")
  "Regexp capturing alpha in legacy rgba() and hsla() syntax.")

(defun shr-color--alpha-from-color (color)
  "Return COLOR's opacity, defaulting to 1.0."
  (let ((case-fold-search t))
    (cond
     ((string-equal (downcase color) "transparent") 0.0)
     ((and (= (length color) 5)
           (string-match-p "\\`#[[:xdigit:]]\\{4\\}\\'" color))
      (/ (* 17 (string-to-number (substring color 4 5) 16)) 255.0))
     ((and (= (length color) 9)
           (string-match-p "\\`#[[:xdigit:]]\\{8\\}\\'" color))
      (/ (string-to-number (substring color 7 9) 16) 255.0))
     ((string-match shr-color--legacy-alpha-regexp color)
      (shr-color--alpha-number (match-string-no-properties 1 color)))
     ((string-match shr-color--modern-rgb-regexp color)
      (let ((alpha (match-string-no-properties 4 color)))
        (if alpha (shr-color--alpha-number alpha) 1.0)))
     ((string-match shr-color--modern-hsl-regexp color)
      (let ((alpha (match-string-no-properties 4 color)))
        (if alpha (shr-color--alpha-number alpha) 1.0)))
     (t 1.0))))

(defun shr-color-relative-to-absolute (number)
  "Convert CSS RGB component NUMBER to a clamped 8-bit channel.
Percentages are scaled to 255 and rounded to the nearest integer,
with halfway values rounded upward."
  (let* ((string-length (length number))
         (percentage (and (> string-length 0)
                          (eq (aref number (1- string-length)) ?%)))
         (value (string-to-number
                 (if percentage (substring number 0 (1- string-length))
                   number)))
         (absolute (if percentage (/ (* value 255.0) 100.0) value))
         (clamped (max 0 (min 255 absolute))))
    (floor (+ clamped 0.5))))

(defun shr-color-hue-to-rgb (x y h)
  "Convert X Y H to RGB value."
  (setq h (mod h 1.0))
  (cond ((< h (/ 6.0)) (+ x (* (- y x) h 6)))
        ((< h 0.5) y)
        ((< h (/ 2.0 3.0)) (+ x (* (- y x) (- (/ 2.0 3.0) h) 6)))
        (t x)))

(defun shr-color-hsl-to-rgb-fractions (h s l)
  "Convert H S L to fractional RGB values."
  (let (m1 m2)
    (if (<= l 0.5)
        (setq m2 (* l (+ s 1)))
        (setq m2 (- (+ l s) (* l s))))
    (setq m1 (- (* l 2) m2))
    (list (shr-color-hue-to-rgb m1 m2 (+ h (/ 3.0)))
	  (shr-color-hue-to-rgb m1 m2 h)
	  (shr-color-hue-to-rgb m1 m2 (- h (/ 3.0))))))

(defun shr-color--alpha-number (number)
  "Return NUMBER as an opacity in the range 0 to 1."
  (let* ((string-length (length number))
         (percentage (and (> string-length 0)
                          (eq (aref number (1- string-length)) ?%)))
         (value (string-to-number
                 (if percentage (substring number 0 (1- string-length))
                   number)))
         (alpha (if percentage (/ value 100.0) value)))
    (max 0 (min 1 alpha))))

(defun shr-color--alpha-byte (number)
  "Convert CSS opacity NUMBER to an 8-bit alpha channel."
  (floor (+ (* (shr-color--alpha-number number) 255) 0.5)))

(defun shr-color--append-alpha (rgb alpha)
  "Append CSS opacity ALPHA to the RGB hexadecimal color RGB."
  (if alpha
      (concat (upcase rgb) (format "%02X" (shr-color--alpha-byte alpha)))
    rgb))

(defun shr-color--normalize-alpha-hex (color)
  "Return COLOR as #RRGGBBAA, expanding its short hexadecimal form."
  (let ((digits (substring color 1)))
    (if (= (length digits) 4)
        (format "#%02X%02X%02X%02X"
                (* 17 (string-to-number (substring digits 0 1) 16))
                (* 17 (string-to-number (substring digits 1 2) 16))
                (* 17 (string-to-number (substring digits 2 3) 16))
                (* 17 (string-to-number (substring digits 3 4) 16)))
      (upcase color))))

(defun shr-color--alpha-from-hex (color)
  "Return COLOR's 8-bit hexadecimal opacity, or nil when it is opaque."
  (when (and (stringp color)
             (= (length color) 9)
             (string-match-p "\\`#[[:xdigit:]]\\{8\\}\\'" color))
    (/ (string-to-number (substring color 7 9) 16) 255.0)))

(defun shr-color--hex-to-rgb (color)
  "Return COLOR's normalized RGB components."
  (when (stringp color)
    (let ((length (length color)))
      (cond
       ((and (= length 7)
             (string-match-p "\\`#[[:xdigit:]]\\{6\\}\\'" color))
        (list (/ (string-to-number (substring color 1 3) 16) 255.0)
              (/ (string-to-number (substring color 3 5) 16) 255.0)
              (/ (string-to-number (substring color 5 7) 16) 255.0)))
       ((and (= length 4)
             (string-match-p "\\`#[[:xdigit:]]\\{3\\}\\'" color))
        (list (/ (* 17 (string-to-number (substring color 1 2) 16)) 255.0)
              (/ (* 17 (string-to-number (substring color 2 3) 16)) 255.0)
              (/ (* 17 (string-to-number (substring color 3 4) 16)) 255.0)))
       (t (color-name-to-rgb color))))))

(defun shr-color--composite-alpha (color background &optional alpha)
  "Composite RGBA COLOR over opaque BACKGROUND.
ALPHA, when non-nil, is the unquantized opacity of COLOR."
  (if (not (and (stringp color)
                (= (length color) 9)
                (string-match-p "\\`#[[:xdigit:]]\\{8\\}\\'" color)))
      color
    (let ((alpha (or alpha (shr-color--alpha-from-hex color)))
          (rgb (substring color 0 7)))
      (cond
       ((or (null alpha) (>= alpha 1.0)) rgb)
       ((= alpha 0.0) background)
       (t
        (let ((foreground-values (shr-color--hex-to-rgb rgb))
              (background-values (shr-color--hex-to-rgb background)))
          (if (and foreground-values background-values)
              (color-rgb-to-hex
               (+ (* alpha (nth 0 foreground-values))
                  (* (- 1 alpha) (nth 0 background-values)))
               (+ (* alpha (nth 1 foreground-values))
                  (* (- 1 alpha) (nth 1 background-values)))
               (+ (* alpha (nth 2 foreground-values))
                  (* (- 1 alpha) (nth 2 background-values)))
               2)
            rgb)))))))

(defun shr-color->hexadecimal (color)
  "Convert COLOR to hexadecimal, including alpha as a final byte when present."
  (when color
    (cond
     ;; CSS hex colors have exactly 3, 4, 6, or 8 digits.
     ((string-equal (downcase color) "transparent")
      "#00000000")
     ((and (string-match-p "\\`#[[:xdigit:]]+\\'" color)
           (memq (length (substring color 1)) '(3 4 6 8)))
      (if (memq (length (substring color 1)) '(4 8))
          (shr-color--normalize-alpha-hex color)
        color))
     ;; rgb() or rgba() colors
     ((or (string-match
           (concat "rgb([[:space:]]*" shr-color--rgb-component-regexp
                   "[[:space:]]*,[[:space:]]*" shr-color--rgb-component-regexp
                   "[[:space:]]*,[[:space:]]*" shr-color--rgb-component-regexp
                   "[[:space:]]*)")
           color)
          (string-match
           (concat "rgba([[:space:]]*" shr-color--rgb-component-regexp
                   "[[:space:]]*,[[:space:]]*" shr-color--rgb-component-regexp
                   "[[:space:]]*,[[:space:]]*" shr-color--rgb-component-regexp
                   "[[:space:]]*,[[:space:]]*\\("
                   shr-color--css-number-regexp
                   "\\(?:[[:space:]]*%\\)?\\)[[:space:]]*)")
           color)
          (string-match shr-color--modern-rgb-regexp color))
      (shr-color--append-alpha
       (format "#%02X%02X%02X"
               (shr-color-relative-to-absolute (match-string-no-properties 1 color))
               (shr-color-relative-to-absolute (match-string-no-properties 2 color))
               (shr-color-relative-to-absolute (match-string-no-properties 3 color)))
       (match-string-no-properties 4 color)))
     ;; hsl() or hsla() colors
     ((or (string-match
           (concat "hsl([[:space:]]*" shr-color--component-regexp
                   "[[:space:]]*,[[:space:]]*" shr-color--component-regexp
                   "[[:space:]]*%[[:space:]]*,[[:space:]]*"
                   shr-color--component-regexp "[[:space:]]*%[[:space:]]*)")
           color)
          (string-match
           (concat "hsla([[:space:]]*" shr-color--component-regexp
                   "[[:space:]]*,[[:space:]]*" shr-color--component-regexp
                   "[[:space:]]*%[[:space:]]*,[[:space:]]*"
                   shr-color--component-regexp
                   "[[:space:]]*%[[:space:]]*,[[:space:]]*\\("
                   shr-color--css-number-regexp
                   "\\(?:[[:space:]]*%\\)?\\)[[:space:]]*)")
           color)
          (string-match shr-color--modern-hsl-regexp color))
      (let* ((alpha (match-string-no-properties 4 color))
             (h (/ (string-to-number (match-string-no-properties 1 color)) 360.0))
             (s (/ (max 0 (string-to-number (match-string-no-properties 2 color)))
                   100.0))
             (l (/ (string-to-number (match-string-no-properties 3 color)) 100.0)))
        (pcase-let ((`(,r ,g ,b) (shr-color-hsl-to-rgb-fractions h s l)))
          (shr-color--append-alpha (color-rgb-to-hex r g b 2) alpha))))
     ;; Color names
     ((cdr (assoc-string color shr-color-html-colors-alist t)))
     ;; Unrecognized color :(
     (t
      nil))))

(defun shr-color-set-minimum-interval (val1 val2 min max interval
					    &optional fixed)
  "Set minimum interval between VAL1 and VAL2 to INTERVAL.
The values are bound by MIN and MAX.
If FIXED is t, then VAL1 will not be touched."
  (let ((diff (abs (- val1 val2))))
    (unless (>= diff interval)
      (if fixed
          (let* ((missing (- interval diff))
                 ;; If val2 > val1, try to increase val2
                 ;; That's the "good direction"
                 (val2-good-direction
                  (if (> val2 val1)
                      (min max (+ val2 missing))
                    (max min (- val2 missing))))
                 (diff-val2-good-direction-val1 (abs (- val2-good-direction val1))))
            (if (>= diff-val2-good-direction-val1 interval)
                (setq val2 val2-good-direction)
              ;; Good-direction is not so good, compute bad-direction
              (let* ((val2-bad-direction
                      (if (> val2 val1)
                          (max min (- val1 interval))
                        (min max (+ val1 interval))))
                     (diff-val2-bad-direction-val1 (abs (- val2-bad-direction val1))))
                (if (>= diff-val2-bad-direction-val1 interval)
                    (setq val2 val2-bad-direction)
                  ;; Still not good, pick the best and prefer good direction
                  (setq val2
                        (if (>= diff-val2-good-direction-val1 diff-val2-bad-direction-val1)
                            val2-good-direction
                          val2-bad-direction))))))
        ;; No fixed, move val1 and val2
        (let ((missing (/ (- interval diff) 2.0)))
          (if (< val1 val2)
              (setq val1 (max min (- val1 missing))
                    val2 (min max (+ val2 missing)))
            (setq val2 (max min (- val2 missing))
                  val1 (min max (+ val1 missing))))
          (setq diff (abs (- val1 val2)))   ; Recompute diff
          (unless (>= diff interval)
            ;; Not ok, we hit a boundary
            (let ((missing (- interval diff)))
              (cond ((= val1 min)
                     (setq val2 (+ val2 missing)))
                    ((= val2 min)
                     (setq val1 (+ val1 missing)))
                    ((= val1 max)
                     (setq val2 (- val2 missing)))
                    ((= val2 max)
                     (setq val1 (- val1 missing)))))))))
    (list val1 val2)))

(defun shr-color-visible (bg fg &optional fixed-background)
  "Check that BG and FG colors are visible if they are drawn on each other.
Return (bg fg) if they are.  If they are too similar, two new
colors are returned instead.
If FIXED-BACKGROUND is set, and if the color are not visible, a
new background color will not be computed.  Only the foreground
color will be adapted to be visible on BG."
  ;; Convert fg and bg to CIE Lab
  (let ((fg-norm (color-name-to-rgb fg))
	(bg-norm (color-name-to-rgb bg)))
    (if (or (null fg-norm)
	    (null bg-norm))
	(list bg fg)
      (let* ((fg-lab (apply #'color-srgb-to-lab fg-norm))
	     (bg-lab (apply #'color-srgb-to-lab bg-norm))
	     ;; Compute color distance using CIE DE 2000
	     (fg-bg-distance (color-cie-de2000 fg-lab bg-lab))
	     ;; Compute luminance distance (subtract L component)
	     (luminance-distance (abs (- (car fg-lab) (car bg-lab)))))
	(if (and (>= fg-bg-distance shr-color-visible-distance-min)
		 (>= luminance-distance shr-color-visible-luminance-min))
	    (list bg fg)
	  ;; Not visible, try to change luminance to make them visible
	  (let ((Ls (shr-color-set-minimum-interval
		     (car bg-lab) (car fg-lab) 0 100
		     shr-color-visible-luminance-min fixed-background)))
	    (unless fixed-background
	      (setcar bg-lab (car Ls)))
	    (setcar fg-lab (cadr Ls))
	    (list
	     (if fixed-background
		 bg
	       (apply #'format "#%02x%02x%02x"
		      (mapcar (lambda (x) (* (max (min 1 x) 0) 255))
			      (apply #'color-lab-to-srgb bg-lab))))
	     (apply #'format "#%02x%02x%02x"
		    (mapcar (lambda (x) (* (max (min 1 x) 0) 255))
			    (apply #'color-lab-to-srgb fg-lab))))))))))

(provide 'shr-color)

;;; shr-color.el ends here
