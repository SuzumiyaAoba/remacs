;;; -*- lexical-binding: t; -*-
;; Issue #251: string-width uses the same sanitized buffer-local tab-width
;; as char-width and format, independent of the TAB's string column.
(defun qa251-widths (setting text)
  (let ((tab-width setting)
        (char-width-table (copy-sequence char-width-table)))
    ;; Keep the non-TAB widths explicit when comparing startup tables.
    (set-char-table-range char-width-table ?é 1)
    (list (char-width 9) (string-width text)
          (if (> (length text) 0) (string-width text 0 -1) (string-width text 0 0))
          (format "%6s" text)
          (truncate-string-to-width text 4)
          (truncate-string-to-width text 6 1)
          (truncate-string-to-width text 6 0 nil "."))))

(qa (qa251-widths nil "") '(8 0 0 "      " "" "" ""))

(qa (qa251-widths nil "	") '(8 8 0 "	" "" "" "."))

(qa (qa251-widths nil "a	") '(8 9 1 "a	" "a" "" "a."))

(qa (qa251-widths nil "	a") '(8 9 8 "	a" "" "" "."))

(qa (qa251-widths nil "a	b	")
    '(8 18 10 "a	b	" "a" "" "a."))

(qa (qa251-widths nil "	\n	") '(8 16 8 "	\n	" "" "" "."))

(qa (qa251-widths nil "日	é")
    '(8 11 10 "日	é" "日" "" "日."))

(qa (qa251-widths -1 "") '(8 0 0 "      " "" "" ""))

(qa (qa251-widths -1 "	") '(8 8 0 "	" "" "" "."))

(qa (qa251-widths -1 "a	") '(8 9 1 "a	" "a" "" "a."))

(qa (qa251-widths -1 "	a") '(8 9 8 "	a" "" "" "."))

(qa (qa251-widths -1 "a	b	") '(8 18 10 "a	b	" "a" "" "a."))

(qa (qa251-widths -1 "	\n	") '(8 16 8 "	\n	" "" "" "."))

(qa (qa251-widths -1 "日	é")
    '(8 11 10 "日	é" "日" "" "日."))

(qa (qa251-widths 0 "") '(8 0 0 "      " "" "" ""))

(qa (qa251-widths 0 "	") '(8 8 0 "	" "" "" "."))

(qa (qa251-widths 0 "a	") '(8 9 1 "a	" "a" "" "a."))

(qa (qa251-widths 0 "	a") '(8 9 8 "	a" "" "" "."))

(qa (qa251-widths 0 "a	b	") '(8 18 10 "a	b	" "a" "" "a."))

(qa (qa251-widths 0 "	\n	") '(8 16 8 "	\n	" "" "" "."))

(qa (qa251-widths 0 "日	é") '(8 11 10 "日	é" "日" "" "日."))

(qa (qa251-widths 1 "") '(1 0 0 "      " "" "" ""))

(qa (qa251-widths 1 "	") '(1 1 0 "     	" "	" "" "	"))

(qa (qa251-widths 1 "a	")
    '(1 2 1 "    a	" "a	" "	" "a	"))

(qa (qa251-widths 1 "	a")
    '(1 2 1 "    	a" "	a" "a" "	a"))

(qa (qa251-widths 1 "a	b	")
    '(1 4 3 "  a	b	" "a	b	" "	b	"
	"a	b	"))

(qa (qa251-widths 1 "	\n	")
    '(1 2 1 "    	\n	" "	\n	" "\n	"
	"	\n	"))

(qa (qa251-widths 1 "日	é")
    '(1 4 3 "  日	é" "日	é" "	é" "日	é"))

(qa (qa251-widths 2 "") '(2 0 0 "      " "" "" ""))

(qa (qa251-widths 2 "	") '(2 2 0 "    	" "	" "" "	"))

(qa (qa251-widths 2 "a	")
    '(2 3 1 "   a	" "a	" "	" "a	"))

(qa (qa251-widths 2 "	a")
    '(2 3 2 "   	a" "	a" "a" "	a"))

(qa (qa251-widths 2 "a	b	")
    '(2 6 4 "a	b	" "a	b" "	b	" "a	b	"))

(qa (qa251-widths 2 "	\n	")
    '(2 4 2 "  	\n	" "	\n	" "\n	" "	\n	"))

(qa (qa251-widths 2 "日	é")
    '(2 5 4 " 日	é" "日	" "	é" "日	é"))

(qa (qa251-widths 4 "") '(4 0 0 "      " "" "" ""))

(qa (qa251-widths 4 "	") '(4 4 0 "  	" "	" "" "	"))

(qa (qa251-widths 4 "a	") '(4 5 1 " a	" "a" "	" "a	"))

(qa (qa251-widths 4 "	a") '(4 5 4 " 	a" "	" "a" "	a"))

(qa (qa251-widths 4 "a	b	")
    '(4 10 6 "a	b	" "a" "	b" "a	."))

(qa (qa251-widths 4 "	\n	")
    '(4 8 4 "	\n	" "	" "\n" "	\n."))

(qa (qa251-widths 4 "日	é") '(4 7 6 "日	é" "日" "	" "日."))

(qa (qa251-widths 8 "") '(8 0 0 "      " "" "" ""))

(qa (qa251-widths 8 "	") '(8 8 0 "	" "" "" "."))

(qa (qa251-widths 8 "a	") '(8 9 1 "a	" "a" "" "a."))

(qa (qa251-widths 8 "	a") '(8 9 8 "	a" "" "" "."))

(qa (qa251-widths 8 "a	b	") '(8 18 10 "a	b	" "a" "" "a."))

(qa (qa251-widths 8 "	\n	") '(8 16 8 "	\n	" "" "" "."))

(qa (qa251-widths 8 "日	é") '(8 11 10 "日	é" "日" "" "日."))

(qa (qa251-widths 16 "") '(16 0 0 "      " "" "" ""))

(qa (qa251-widths 16 "	") '(16 16 0 "	" "" "" "."))

(qa (qa251-widths 16 "a	") '(16 17 1 "a	" "a" "" "a."))

(qa (qa251-widths 16 "	a") '(16 17 16 "	a" "" "" "."))

(qa (qa251-widths 16 "a	b	")
    '(16 34 18 "a	b	" "a" "" "a."))

(qa (qa251-widths 16 "	\n	") '(16 32 16 "	\n	" "" "" "."))

(qa (qa251-widths 16 "日	é")
    '(16 19 18 "日	é" "日" "" "日."))

(qa (qa251-widths 1000 "") '(1000 0 0 "      " "" "" ""))

(qa (qa251-widths 1000 "	") '(1000 1000 0 "	" "" "" "."))

(qa (qa251-widths 1000 "a	") '(1000 1001 1 "a	" "a" "" "a."))

(qa (qa251-widths 1000 "	a") '(1000 1001 1000 "	a" "" "" "."))

(qa (qa251-widths 1000 "a	b	")
    '(1000 2002 1002 "a	b	" "a" "" "a."))

(qa (qa251-widths 1000 "	\n	")
    '(1000 2000 1000 "	\n	" "" "" "."))

(qa (qa251-widths 1000 "日	é")
    '(1000 1003 1002 "日	é" "日" "" "日."))

(qa (qa251-widths 1001 "") '(8 0 0 "      " "" "" ""))

(qa (qa251-widths 1001 "	") '(8 8 0 "	" "" "" "."))

(qa (qa251-widths 1001 "a	") '(8 9 1 "a	" "a" "" "a."))

(qa (qa251-widths 1001 "	a") '(8 9 8 "	a" "" "" "."))

(qa (qa251-widths 1001 "a	b	")
    '(8 18 10 "a	b	" "a" "" "a."))

(qa (qa251-widths 1001 "	\n	")
    '(8 16 8 "	\n	" "" "" "."))

(qa (qa251-widths 1001 "日	é")
    '(8 11 10 "日	é" "日" "" "日."))

(qa (qa251-widths 1.5 "") '(8 0 0 "      " "" "" ""))

(qa (qa251-widths 1.5 "	") '(8 8 0 "	" "" "" "."))

(qa (qa251-widths 1.5 "a	") '(8 9 1 "a	" "a" "" "a."))

(qa (qa251-widths 1.5 "	a") '(8 9 8 "	a" "" "" "."))

(qa (qa251-widths 1.5 "a	b	")
    '(8 18 10 "a	b	" "a" "" "a."))

(qa (qa251-widths 1.5 "	\n	") '(8 16 8 "	\n	" "" "" "."))

(qa (qa251-widths 1.5 "日	é")
    '(8 11 10 "日	é" "日" "" "日."))

(qa (qa251-widths 'bad "") '(8 0 0 "      " "" "" ""))

(qa (qa251-widths 'bad "	") '(8 8 0 "	" "" "" "."))

(qa (qa251-widths 'bad "a	") '(8 9 1 "a	" "a" "" "a."))

(qa (qa251-widths 'bad "	a") '(8 9 8 "	a" "" "" "."))

(qa (qa251-widths 'bad "a	b	")
    '(8 18 10 "a	b	" "a" "" "a."))

(qa (qa251-widths 'bad "	\n	")
    '(8 16 8 "	\n	" "" "" "."))

(qa (qa251-widths 'bad "日	é")
    '(8 11 10 "日	é" "日" "" "日."))

(qa (qa251-widths (expt 2 200) "") '(8 0 0 "      " "" "" ""))

(qa (qa251-widths (expt 2 200) "	") '(8 8 0 "	" "" "" "."))

(qa (qa251-widths (expt 2 200) "a	") '(8 9 1 "a	" "a" "" "a."))

(qa (qa251-widths (expt 2 200) "	a") '(8 9 8 "	a" "" "" "."))

(qa (qa251-widths (expt 2 200) "a	b	")
    '(8 18 10 "a	b	" "a" "" "a."))

(qa (qa251-widths (expt 2 200) "	\n	")
    '(8 16 8 "	\n	" "" "" "."))

(qa (qa251-widths (expt 2 200) "日	é")
    '(8 11 10 "日	é" "日" "" "日."))

(qa
 (let ((tab-width 4))
   (list (char-width 9) (string-width "	")
	 (truncate-string-to-width "	X" 4)))
 '(4 4 "	"))

(qa
 (with-temp-buffer
   (setq-local tab-width 3)
   (let ((outer (list (char-width 9) (string-width "a	"))))
     (list outer
	   (let ((tab-width 5))
	     (list (char-width 9) (string-width "		")))
	   tab-width (string-width "	"))))
 '((3 4) (5 10) 3 3))

(qa
 (let
     ((first (generate-new-buffer " *qa251-first*"))
      (second (generate-new-buffer " *qa251-second*")))
   (unwind-protect
       (progn
	 (with-current-buffer first (setq-local tab-width 4))
	 (with-current-buffer second (setq-local tab-width 6))
	 (list
	  (with-current-buffer first (string-width "		"))
	  (with-current-buffer second (string-width "		"))
	  (with-current-buffer first (string-width "	"))))
     (kill-buffer first) (kill-buffer second)))
 '(8 12 4))

(qa
 (let
     ((tab-width 4)
      (char-width-table (copy-sequence char-width-table)))
   (set-char-table-range char-width-table 9 99)
   (set-char-table-range char-width-table 26085 3)
   (list (char-width 9) (string-width "	日")
	 (string-width "a	日b" 1 -1)))
 '(4 7 7))

(qa (let ((tab-width 4)) (string-width "x	日y" 1 3)) '6)

(qa (let ((tab-width 4)) (string-width "x	日y" -3 -1)) '6)

(qa
 (let ((tab-width 4))
   (condition-case err (string-width "	X" 0 3) (error err)))
 '(args-out-of-range "	X" 0 3))

(qa
 (let ((tab-width 4))
   (condition-case err (string-width 42 'bad 'bad) (error err)))
 '(wrong-type-argument stringp 42))


(princ (qa--done)) (terpri)
