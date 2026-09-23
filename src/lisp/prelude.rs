//! Startup Lisp: the subr.el subset remacs loads at interpreter boot.
//! These are real Lisp macros/functions (as in Emacs), not Rust code.

/// Source evaluated once per `Interp::new`.
pub const PRELUDE: &str = r##"
;; -*- lexical-binding: nil -*-

;; ---------- control-flow macros ----------

(defmacro when (cond &rest body)
  "If COND yields non-nil, do BODY, else return nil."
  (list 'if cond (cons 'progn body)))

(defmacro unless (cond &rest body)
  "If COND yields nil, do BODY."
  (cons 'if (cons cond (cons nil body))))

(defmacro dolist (spec &rest body)
  "Loop over a list.
Evaluate BODY with VAR bound to each element of LIST, in turn.
Then evaluate RESULT (default nil) with VAR bound to nil."
  (let ((tail (make-symbol "tail")))
    (append
     (list 'let (list (list tail (nth 1 spec)))
           (list 'while tail
                 (append (list 'let (list (list (nth 0 spec)
                                              (list 'car tail))))
                         body
                         (list (list 'setq tail (list 'cdr tail))))))
     (cdr (cdr spec)))))

(defmacro dotimes (spec &rest body)
  "Loop a certain number of times.
Evaluate BODY with VAR bound to successive integers from 0,
inclusive, to COUNT, exclusive."
  (let ((count (make-symbol "dotimes")))
    (list 'let (list (list count (nth 1 spec))
                     (list (nth 0 spec) 0))
          (cons 'while
                (cons (list '< (nth 0 spec) count)
                      (append body
                              (list (list 'setq (nth 0 spec)
                                          (list '1+ (nth 0 spec)))))))
          (nth 2 spec))))

(defmacro ignore-errors (&rest body)
  "Execute BODY; if an error occurs, return nil."
  (list 'condition-case nil (cons 'progn body) '(error nil)))

;; ---------- macroexp.el helpers (GNU: dumped, always loaded) ----------
;; Ported from GNU lisp/emacs-lisp/macroexp.el; the parts that were once
;; internal subrs here had incompatible signatures and were replaced by
;; these Lisp definitions.

(defvar macro-declarations-alist nil
  "Alist of (MACRO . DECLARATIONS-ALIST) for macro expanders.")
(defvar defun-declarations-alist nil
  "Alist of (PROP . FN) handlers for `declare' specs in `defun'.")

(defun macroexp-progn (exps)
  "Return EXPS (a list of expressions) with `progn' prepended.
If EXPS is a list with a single expression, `progn' is not
prepended, but that expression is returned instead."
  (if (cdr exps) `(progn ,@exps) (car exps)))

(defun macroexp-unprogn (exp)
  "Turn EXP into a list of expressions to execute in sequence.
Never returns an empty list."
  (if (eq (car-safe exp) 'progn) (or (cdr exp) '(nil)) (list exp)))

(defun macroexp-let* (bindings exp)
  "Return an expression equivalent to \\=`(let* ,BINDINGS ,EXP)."
  (cond
   ((null bindings) exp)
   ((eq 'let* (car-safe exp)) `(let* (,@bindings ,@(cadr exp)) ,@(cddr exp)))
   (t `(let* ,bindings ,exp))))

(defun macroexp-if (test then else)
  "Return an expression equivalent to \\=`(if ,TEST ,THEN ,ELSE)."
  (cond
   ((eq (car-safe else) 'if)
    (cond
     ((equal then (nth 2 else))
      `(if (or ,test ,(nth 1 else)) ,then ,@(nthcdr 3 else)))
     ((equal (macroexp-unprogn then) (nthcdr 3 else))
      `(if (or ,test (not ,(nth 1 else)))
           ,then ,@(macroexp-unprogn (nth 2 else))))
     (t
      `(cond (,test ,@(macroexp-unprogn then))
             (,(nth 1 else) ,@(macroexp-unprogn (nth 2 else)))
             ,@(let ((def (nthcdr 3 else))) (if def `((t ,@def))))))))
   ((eq (car-safe else) 'cond)
    `(cond (,test ,@(macroexp-unprogn then)) ,@(cdr else)))
   ;; Invert the test if that lets us reduce the depth of the tree.
   ((memq (car-safe then) '(if cond)) (macroexp-if `(not ,test) else then))
   (t `(if ,test ,then ,@(if else (macroexp-unprogn else))))))

(defmacro macroexp-let2 (test sym exp &rest body)
  "Evaluate BODY with SYM bound to an expression for EXP's value.
The intended usage is that BODY generates an expression that
will refer to EXP's value multiple times, but will evaluate
EXP only once.  As BODY generates that expression, it should
use SYM to stand for the value of EXP.

If EXP is a simple, safe expression, then SYM's value is EXP itself.
Otherwise, SYM's value is a symbol which holds the value produced by
evaluating EXP.  The return value incorporates the value of BODY, plus
additional code to evaluate EXP once and save the result so SYM can
refer to it.

If BODY consists of multiple forms, they are all evaluated
but only the last one's value matters.

TEST is a predicate to determine whether EXP qualifies as simple and
safe; if TEST is nil, only constant expressions qualify."
  (declare (indent 3))
  (let ((bodysym (make-symbol "body"))
        (expsym (make-symbol "exp")))
    `(let* ((,expsym ,exp)
            (,sym (if (funcall #',(or test #'macroexp-const-p) ,expsym)
                      ,expsym (make-symbol ,(symbol-name sym))))
            (,bodysym ,(macroexp-progn body)))
       (if (eq ,sym ,expsym) ,bodysym
         (macroexp-let* (list (list ,sym ,expsym))
                        ,bodysym)))))

(defmacro macroexp-let2* (test bindings &rest body)
  "Multiple binding version of `macroexp-let2'.

BINDINGS is a list of elements of the form (SYM EXP) or just SYM,
which then stands for (SYM SYM).
Each EXP can refer to symbols specified earlier in the binding list.

TEST has to be a symbol, and if it is nil it can be omitted."
  (declare (indent 2))
  (when (consp test) ;; `test' was omitted.
    (push bindings body)
    (setq bindings test)
    (setq test nil))
  (if (null bindings)
      (macroexp-progn body)
    (let* ((hd (car bindings))
           (var (if (consp hd) (car hd) hd))
           (exp (if (consp hd) (cadr hd) var)))
      `(macroexp-let2 ,test ,var ,exp
         (macroexp-let2* ,test ,(cdr bindings) ,@body)))))

(defun macroexp--maxsize (exp size)
  (cond ((< size 0) size)
        ((symbolp exp) (1- size))
        ((stringp exp) (- size (/ (length exp) 16)))
        ((vectorp exp)
         (dotimes (i (length exp))
           (setq size (macroexp--maxsize (aref exp i) size)))
         (1- size))
        ((consp exp)
         (dolist (e exp)
           (setq size (macroexp--maxsize e size)))
         (1- size))
        (t -1)))

(defun macroexp-small-p (exp)
  "Return non-nil if EXP can be considered small."
  (> (macroexp--maxsize exp 10) 0))

(defun macroexp--const-symbol-p (symbol &optional any-value)
  "Non-nil if SYMBOL is constant.
If ANY-VALUE is nil, only return non-nil if the value of the symbol is the
symbol itself."
  (or (memq symbol '(nil t))
      (keywordp symbol)
      (and any-value (boundp symbol))))

(defun macroexp-const-p (exp)
  "Return non-nil if EXP will always evaluate to the same value."
  (cond ((consp exp) (or (eq (car exp) 'quote)
                         (and (eq (car exp) 'function)
                              (symbolp (cadr exp)))))
        ;; It would sometimes make sense to pass `any-value', but it's not
        ;; always safe since a "constant" variable may not actually always have
        ;; the same value.
        ((symbolp exp) (macroexp--const-symbol-p exp))
        (t t)))

(defun macroexp-copyable-p (exp)
  "Return non-nil if EXP can be copied without extra cost."
  (or (symbolp exp) (macroexp-const-p exp)))

(defun macroexp-quote (v)
  "Return an expression E such that `(eval E)' is V.

E is either V or (quote V) depending on whether V evaluates to
itself or not."
  (if (and (not (consp v))
	   (or (keywordp v)
	       (not (symbolp v))
	       (memq v '(nil t))))
      v
    (list 'quote v)))

(defun macroexp-warn-and-return (msg form &optional _category compile-only _arg)
  "Return code equivalent to FORM labeled with warning MSG."
  (unless compile-only (message "%s" msg))
  form)

;; Bytecomp internals that gv.el may consult when a generalized variable
;; is marked obsolete; the byte-compiler itself is not present.
(defun byte-compile-warn-obsolete (&rest _)
  "Warn that an obsolete construct was used (stub: bytecomp absent)."
  nil)

;; GNU defines `function-get' in subr.el as a Lisp function that follows
;; autoloads and symbol-function indirections; port that exact behavior.
(defun function-get (f prop &optional autoload)
  "Return the value of property PROP of function F.
If AUTOLOAD is non-nil and F is autoloaded, try to load it
in the hope that it will set PROP.  If AUTOLOAD is `macro', do it only
if it's an autoloaded macro."
  (let ((val nil))
    (while (and (symbolp f)
                (null (setq val (get f prop)))
                (fboundp f))
      (let ((fundef (symbol-function f)))
        (if (and autoload (autoloadp fundef)
                 (not (equal fundef
                             (autoload-do-load fundef f
                                               (if (eq autoload 'macro)
                                                   'macro)))))
            nil                         ;Re-try `get' on the same `f'.
          (setq f fundef))))
    val))

;; GNU dumps macroexp.el, so `(featurep 'macroexp)' is t and
;; `(require 'macroexp)' in `push'/gv.el is a no-op.  Mirror that.
(provide 'macroexp)

(defvar remacs-coding-system-plists
  '(
    (adobe-standard-encoding . (:ascii-compatible-p nil :category coding-category-charset :name adobe-standard-encoding :docstring "Adobe `standard' encoding for PostScript" :coding-type charset :mnemonic 42 :charset-list (adobe-standard-encoding) :mime-charset adobe-standard-encoding))
    (alternativnyj . (:ascii-compatible-p t :category coding-category-charset :name cyrillic-alternativnyj :docstring "ALTERNATIVNYJ 8-bit encoding for Cyrillic." :coding-type charset :mnemonic 65 :charset-list (alternativnyj)))
    (ascii . (:ascii-compatible-p t :category coding-category-charset :name us-ascii :docstring "Encode ASCII as-is and encode non-ASCII characters to `?'." :coding-type charset :mnemonic 45 :charset-list (ascii) :default-char 63 :mime-charset us-ascii))
    (big5 . (:ascii-compatible-p t :category coding-category-big5 :name chinese-big5 :docstring "BIG5 8-bit encoding for Chinese (MIME:Big5)" :coding-type big5 :mnemonic 66 :charset-list (ascii big5) :mime-charset big5))
    (big5-hkscs . (:ascii-compatible-p t :category coding-category-charset :name chinese-big5-hkscs :docstring "BIG5-HKSCS 8-bit encoding for Chinese, Hong Kong supplement (MIME:Big5-HKSCS)" :coding-type charset :mnemonic 66 :charset-list (ascii big5-hkscs) :mime-charset big5-hkscs))
    (binary . (:ascii-compatible-p t :category coding-category-raw-text :name no-conversion :mnemonic 61 :coding-type raw-text :ascii-compatible-p t :default-char 0 :for-unibyte t :docstring "Do no conversion.
    
    When you visit a file with this coding, the file is read into a
    unibyte buffer as is, thus each byte of a file is treated as a
    character." :eol-type unix))
    (chinese-big5 . (:ascii-compatible-p t :category coding-category-big5 :name chinese-big5 :docstring "BIG5 8-bit encoding for Chinese (MIME:Big5)" :coding-type big5 :mnemonic 66 :charset-list (ascii big5) :mime-charset big5))
    (chinese-big5-hkscs . (:ascii-compatible-p t :category coding-category-charset :name chinese-big5-hkscs :docstring "BIG5-HKSCS 8-bit encoding for Chinese, Hong Kong supplement (MIME:Big5-HKSCS)" :coding-type charset :mnemonic 66 :charset-list (ascii big5-hkscs) :mime-charset big5-hkscs))
    (chinese-gb18030 . (:ascii-compatible-p t :category coding-category-charset :name chinese-gb18030 :docstring "GB18030 encoding for Chinese (MIME:GB18030)." :coding-type charset :mnemonic 99 :charset-list (ascii gb18030-2-byte gb18030-4-byte-bmp gb18030-4-byte-smp gb18030-4-byte-ext-1 gb18030-4-byte-ext-2) :mime-charset gb18030))
    (chinese-gbk . (:ascii-compatible-p t :category coding-category-charset :name chinese-gbk :docstring "GBK encoding for Chinese (MIME:GBK)." :coding-type charset :mnemonic 99 :charset-list (ascii chinese-gbk) :mime-charset gbk))
    (chinese-hz . (:ascii-compatible-p nil :category coding-category-utf-8 :name chinese-hz :docstring "Hz/ZW 7-bit encoding for Chinese GB2312 (MIME:HZ-GB-2312)." :coding-type utf-8 :mnemonic 122 :charset-list (ascii chinese-gb2312) :mime-charset hz-gb-2312 :post-read-conversion post-read-decode-hz :pre-write-conversion pre-write-encode-hz))
    (chinese-iso-7bit . (:ascii-compatible-p nil :category coding-category-iso-7-else :name iso-2022-cn :docstring "ISO 2022 based 7bit encoding for Chinese GB and CNS (MIME:ISO-2022-CN)." :coding-type iso-2022 :mnemonic 67 :charset-list (ascii chinese-gb2312 chinese-cns11643-1 chinese-cns11643-2) :designation [ascii (nil chinese-gb2312 chinese-cns11643-1) (nil chinese-cns11643-2) nil] :flags (ascii-at-eol ascii-at-cntl 7-bit designation locking-shift single-shift init-at-bol) :mime-charset iso-2022-cn :suitable-for-keyboard t))
    (chinese-iso-8bit . (:ascii-compatible-p t :category coding-category-iso-8-2 :name chinese-iso-8bit :docstring "ISO 2022 based EUC encoding for Chinese GB2312 (MIME:GB2312)." :coding-type iso-2022 :mnemonic 99 :charset-list (ascii chinese-gb2312) :designation [ascii chinese-gb2312 nil nil] :mime-charset gb2312))
    (cn-big5 . (:ascii-compatible-p t :category coding-category-big5 :name chinese-big5 :docstring "BIG5 8-bit encoding for Chinese (MIME:Big5)" :coding-type big5 :mnemonic 66 :charset-list (ascii big5) :mime-charset big5))
    (cn-big5-hkscs . (:ascii-compatible-p t :category coding-category-charset :name chinese-big5-hkscs :docstring "BIG5-HKSCS 8-bit encoding for Chinese, Hong Kong supplement (MIME:Big5-HKSCS)" :coding-type charset :mnemonic 66 :charset-list (ascii big5-hkscs) :mime-charset big5-hkscs))
    (cn-gb . (:ascii-compatible-p t :category coding-category-iso-8-2 :name chinese-iso-8bit :docstring "ISO 2022 based EUC encoding for Chinese GB2312 (MIME:GB2312)." :coding-type iso-2022 :mnemonic 99 :charset-list (ascii chinese-gb2312) :designation [ascii chinese-gb2312 nil nil] :mime-charset gb2312))
    (cn-gb-2312 . (:ascii-compatible-p t :category coding-category-iso-8-2 :name chinese-iso-8bit :docstring "ISO 2022 based EUC encoding for Chinese GB2312 (MIME:GB2312)." :coding-type iso-2022 :mnemonic 99 :charset-list (ascii chinese-gb2312) :designation [ascii chinese-gb2312 nil nil] :mime-charset gb2312))
    (compound-text . (:ascii-compatible-p nil :category coding-category-iso-8-else :name compound-text :docstring "Compound text based generic encoding.
    This coding system is an extension of X's \"Compound Text Encoding\".
    It encodes many characters using the normal ISO-2022 designation sequences,
    but it doesn't support extended segments of CTEXT." :coding-type iso-2022 :mnemonic 120 :charset-list iso-2022 :designation [(ascii 94) (latin-iso8859-1 katakana-jisx0201 96) nil nil] :flags (ascii-at-eol ascii-at-cntl long-form designation locking-shift single-shift composition) :mime-charset x-ctext))
    (compound-text-with-extensions . (:ascii-compatible-p nil :category coding-category-iso-8-else :name compound-text-with-extensions :docstring "Compound text encoding with ICCCM Extended Segment extensions.
    
    See the variables `ctext-standard-encodings' and
    `ctext-non-standard-encodings-alist' for the detail about how
    extended segments are handled.
    
    This coding system should be used only for X selections.  It is inappropriate
    for decoding and encoding files, process I/O, etc." :coding-type iso-2022 :mnemonic 120 :charset-list iso-2022 :designation [(ascii 94) (latin-iso8859-1 katakana-jisx0201 96) nil nil] :flags (ascii-at-eol ascii-at-cntl long-form designation locking-shift single-shift) :post-read-conversion ctext-post-read-conversion :pre-write-conversion ctext-pre-write-conversion :mime-charset x-ctext))
    (cp038 . (:ascii-compatible-p nil :category coding-category-charset :name ibm038 :docstring "International version of EBCDIC" :coding-type charset :charset-list (ibm038) :mnemonic 42))
    (cp1047 . (:ascii-compatible-p nil :category coding-category-charset :name ibm1047 :docstring "A version of EBCDIC used in OS/390 Unix" :coding-type charset :charset-list (ibm1047) :mnemonic 42))
    (cp1125 . (:ascii-compatible-p t :category coding-category-charset :name cp1125 :docstring "cp1125 8-bit encoding for Cyrillic" :coding-type charset :mnemonic 42 :charset-list (cp1125)))
    (cp1250 . (:ascii-compatible-p t :category coding-category-charset :name windows-1250 :docstring "windows-1250 (Central European) encoding (MIME: WINDOWS-1250)" :coding-type charset :mnemonic 42 :charset-list (windows-1250) :mime-charset windows-1250))
    (cp1251 . (:ascii-compatible-p t :category coding-category-charset :name windows-1251 :docstring "windows-1251 8-bit encoding for Cyrillic (MIME: WINDOWS-1251)" :coding-type charset :mnemonic 98 :charset-list (windows-1251) :mime-charset windows-1251))
    (cp1252 . (:ascii-compatible-p t :category coding-category-charset :name windows-1252 :docstring "windows-1252 (Western European) encoding (MIME: WINDOWS-1252)" :coding-type charset :mnemonic 42 :charset-list (windows-1252) :mime-charset windows-1252))
    (cp1253 . (:ascii-compatible-p t :category coding-category-charset :name windows-1253 :docstring "windows-1253 encoding for Greek" :coding-type charset :mnemonic 103 :charset-list (windows-1253) :mime-charset windows-1253))
    (cp1254 . (:ascii-compatible-p t :category coding-category-charset :name windows-1254 :docstring "windows-1254 (Turkish) encoding (MIME: WINDOWS-1254)" :coding-type charset :mnemonic 42 :charset-list (windows-1254) :mime-charset windows-1254))
    (cp1255 . (:ascii-compatible-p t :category coding-category-charset :name windows-1255 :docstring "windows-1255 (Hebrew) encoding (MIME: WINDOWS-1255)" :coding-type charset :mnemonic 104 :charset-list (windows-1255) :mime-charset windows-1255))
    (cp1256 . (:ascii-compatible-p t :category coding-category-charset :name windows-1256 :docstring "windows-1256 (Arabic) encoding (MIME: WINDOWS-1256)" :coding-type charset :mnemonic 65 :charset-list (windows-1256) :mime-charset windows-1256))
    (cp1257 . (:ascii-compatible-p t :category coding-category-charset :name windows-1257 :docstring "windows-1257 (Baltic) encoding (MIME: WINDOWS-1257)" :coding-type charset :mnemonic 42 :charset-list (windows-1257) :mime-charset windows-1257))
    (cp1258 . (:ascii-compatible-p t :category coding-category-charset :name windows-1258 :docstring "windows-1258 encoding for Vietnamese (MIME: WINDOWS-1258)" :coding-type charset :mnemonic 42 :charset-list (windows-1258) :mime-charset windows-1258))
    (cp256 . (:ascii-compatible-p nil :category coding-category-charset :name ibm256 :docstring "Netherlands version of EBCDIC" :coding-type charset :charset-list (ibm256) :mnemonic 42))
    (cp273 . (:ascii-compatible-p nil :category coding-category-charset :name ibm273 :docstring "Austrian / German version of EBCDIC" :coding-type charset :charset-list (ibm273) :mnemonic 42))
    (cp274 . (:ascii-compatible-p nil :category coding-category-charset :name ibm274 :docstring "Belgian version of EBCDIC" :coding-type charset :charset-list (ibm274) :mnemonic 42))
    (cp275 . (:ascii-compatible-p nil :category coding-category-charset :name ibm275 :docstring "Brazilian version of EBCDIC" :coding-type charset :charset-list (ibm275) :mnemonic 42))
    (cp277 . (:ascii-compatible-p nil :category coding-category-charset :name ibm277 :docstring "Danish / Norwegian version of EBCDIC" :coding-type charset :charset-list (ibm277) :mnemonic 42))
    (cp278 . (:ascii-compatible-p nil :category coding-category-charset :name ibm278 :docstring "Finnish / Swedish version of EBCDIC" :coding-type charset :charset-list (ibm278) :mnemonic 42))
    (cp280 . (:ascii-compatible-p nil :category coding-category-charset :name ibm280 :docstring "Italian version of EBCDIC" :coding-type charset :charset-list (ibm280) :mnemonic 42))
    (cp281 . (:ascii-compatible-p nil :category coding-category-charset :name ibm281 :docstring "Japanese-E version of EBCDIC" :coding-type charset :charset-list (ibm281) :mnemonic 42))
    (cp284 . (:ascii-compatible-p nil :category coding-category-charset :name ibm284 :docstring "Spanish version of EBCDIC" :coding-type charset :charset-list (ibm284) :mnemonic 42))
    (cp285 . (:ascii-compatible-p nil :category coding-category-charset :name ibm285 :docstring "UK English version of EBCDIC" :coding-type charset :charset-list (ibm285) :mnemonic 42))
    (cp290 . (:ascii-compatible-p nil :category coding-category-charset :name ibm290 :docstring "Japanese katakana version of EBCDIC" :coding-type charset :charset-list (ibm290) :mnemonic 42))
    (cp297 . (:ascii-compatible-p nil :category coding-category-charset :name ibm297 :docstring "French version of EBCDIC" :coding-type charset :charset-list (ibm297) :mnemonic 42))
    (cp437 . (:ascii-compatible-p t :category coding-category-charset :name cp437 :docstring "DOS codepage 437" :coding-type charset :mnemonic 68 :charset-list (cp437) :mime-charset cp437))
    (cp65001 . (:ascii-compatible-p t :category coding-category-utf-8 :name utf-8 :docstring "UTF-8 (no signature (BOM))" :coding-type utf-8 :mnemonic 85 :charset-list (unicode) :mime-charset utf-8))
    (cp737 . (:ascii-compatible-p t :category coding-category-charset :name cp737 :docstring "Codepage 737 (PC Greek)" :coding-type charset :mnemonic 68 :charset-list (cp737) :mime-charset cp737))
    (cp775 . (:ascii-compatible-p t :category coding-category-charset :name cp775 :docstring "DOS codepage 775 (PC Baltic, MS-DOS Baltic Rim)" :coding-type charset :mnemonic 68 :charset-list (cp775) :mime-charset cp775))
    (cp850 . (:ascii-compatible-p t :category coding-category-charset :name cp850 :docstring "DOS codepage 850 (Western European)" :coding-type charset :mnemonic 68 :charset-list (cp850) :mime-charset cp850))
    (cp851 . (:ascii-compatible-p t :category coding-category-charset :name cp851 :docstring "DOS codepage 851 (Greek)" :coding-type charset :mnemonic 68 :charset-list (cp851) :mime-charset cp851))
    (cp852 . (:ascii-compatible-p t :category coding-category-charset :name cp852 :docstring "DOS codepage 852 (Slavic)" :coding-type charset :mnemonic 68 :charset-list (cp852) :mime-charset cp852))
    (cp855 . (:ascii-compatible-p t :category coding-category-charset :name cp855 :docstring "DOS codepage 855 (Russian)" :coding-type charset :mnemonic 68 :charset-list (cp855) :mime-charset cp855))
    (cp857 . (:ascii-compatible-p t :category coding-category-charset :name cp857 :docstring "DOS codepage 857 (Turkish)" :coding-type charset :mnemonic 68 :charset-list (cp857) :mime-charset cp857))
    (cp858 . (:ascii-compatible-p t :category coding-category-charset :name cp858 :docstring "Codepage 858 (Multilingual Latin I + Euro)" :coding-type charset :mnemonic 68 :charset-list (cp858) :mime-charset cp858))
    (cp860 . (:ascii-compatible-p t :category coding-category-charset :name cp860 :docstring "DOS codepage 860 (Portuguese)" :coding-type charset :mnemonic 68 :charset-list (cp860) :mime-charset cp860))
    (cp861 . (:ascii-compatible-p t :category coding-category-charset :name cp861 :docstring "DOS codepage 861 (Icelandic)" :coding-type charset :mnemonic 68 :charset-list (cp861) :mime-charset cp861))
    (cp862 . (:ascii-compatible-p t :category coding-category-charset :name cp862 :docstring "DOS codepage 862 (Hebrew)" :coding-type charset :mnemonic 68 :charset-list (cp862) :mime-charset cp862))
    (cp863 . (:ascii-compatible-p t :category coding-category-charset :name cp863 :docstring "DOS codepage 863 (French Canadian)" :coding-type charset :mnemonic 68 :charset-list (cp863) :mime-charset cp863))
    (cp865 . (:ascii-compatible-p t :category coding-category-charset :name cp865 :docstring "DOS codepage 865 (Norwegian/Danish)" :coding-type charset :mnemonic 68 :charset-list (cp865) :mime-charset cp865))
    (cp866 . (:ascii-compatible-p t :category coding-category-charset :name cp866 :docstring "CP866 encoding for Cyrillic." :coding-type charset :mnemonic 42 :charset-list (ibm866) :mime-charset cp866))
    (cp866u . (:ascii-compatible-p t :category coding-category-charset :name cp1125 :docstring "cp1125 8-bit encoding for Cyrillic" :coding-type charset :mnemonic 42 :charset-list (cp1125)))
    (cp869 . (:ascii-compatible-p t :category coding-category-charset :name cp869 :docstring "DOS codepage 869 (Greek)" :coding-type charset :mnemonic 68 :charset-list (cp869) :mime-charset cp869))
    (cp874 . (:ascii-compatible-p t :category coding-category-charset :name cp874 :docstring "DOS codepage 874 (Thai)" :coding-type charset :mnemonic 68 :charset-list (cp874) :mime-charset cp874))
    (cp878 . (:ascii-compatible-p t :category coding-category-charset :name cyrillic-koi8 :docstring "KOI8 8-bit encoding for Cyrillic (MIME: KOI8-R)." :coding-type charset :mnemonic 82 :charset-list (koi8) :mime-charset koi8-r))
    (cp932 . (:ascii-compatible-p t :category coding-category-charset :name japanese-cp932 :docstring "CP932 (Microsoft shift-jis)" :coding-type charset :mnemonic 83 :charset-list (ascii katakana-sjis cp932-2-byte)))
    (cp936 . (:ascii-compatible-p t :category coding-category-charset :name chinese-gbk :docstring "GBK encoding for Chinese (MIME:GBK)." :coding-type charset :mnemonic 99 :charset-list (ascii chinese-gbk) :mime-charset gbk))
    (cp949 . (:ascii-compatible-p t :category coding-category-charset :name korean-cp949 :docstring "CP949 (Microsoft Unified Hangul Code)" :coding-type charset :mnemonic 75 :charset-list (ascii cp949)))
    (cp950 . (:ascii-compatible-p t :category coding-category-big5 :name chinese-big5 :docstring "BIG5 8-bit encoding for Chinese (MIME:Big5)" :coding-type big5 :mnemonic 66 :charset-list (ascii big5) :mime-charset big5))
    (ctext . (:ascii-compatible-p nil :category coding-category-iso-8-else :name compound-text :docstring "Compound text based generic encoding.
    This coding system is an extension of X's \"Compound Text Encoding\".
    It encodes many characters using the normal ISO-2022 designation sequences,
    but it doesn't support extended segments of CTEXT." :coding-type iso-2022 :mnemonic 120 :charset-list iso-2022 :designation [(ascii 94) (latin-iso8859-1 katakana-jisx0201 96) nil nil] :flags (ascii-at-eol ascii-at-cntl long-form designation locking-shift single-shift composition) :mime-charset x-ctext))
    (ctext-no-compositions . (:ascii-compatible-p nil :category coding-category-iso-8-else :name ctext-no-compositions :docstring "Compound text based generic encoding.
    
    Like `compound-text', but does not produce escape sequences for compositions." :coding-type iso-2022 :mnemonic 120 :charset-list iso-2022 :designation [(ascii 94) (latin-iso8859-1 katakana-jisx0201 96) nil nil] :flags (ascii-at-eol ascii-at-cntl designation locking-shift single-shift)))
    (ctext-with-extensions . (:ascii-compatible-p nil :category coding-category-iso-8-else :name compound-text-with-extensions :docstring "Compound text encoding with ICCCM Extended Segment extensions.
    
    See the variables `ctext-standard-encodings' and
    `ctext-non-standard-encodings-alist' for the detail about how
    extended segments are handled.
    
    This coding system should be used only for X selections.  It is inappropriate
    for decoding and encoding files, process I/O, etc." :coding-type iso-2022 :mnemonic 120 :charset-list iso-2022 :designation [(ascii 94) (latin-iso8859-1 katakana-jisx0201 96) nil nil] :flags (ascii-at-eol ascii-at-cntl long-form designation locking-shift single-shift) :post-read-conversion ctext-post-read-conversion :pre-write-conversion ctext-pre-write-conversion :mime-charset x-ctext))
    (cyrillic-alternativnyj . (:ascii-compatible-p t :category coding-category-charset :name cyrillic-alternativnyj :docstring "ALTERNATIVNYJ 8-bit encoding for Cyrillic." :coding-type charset :mnemonic 65 :charset-list (alternativnyj)))
    (cyrillic-iso-8bit . (:ascii-compatible-p t :category coding-category-charset :name cyrillic-iso-8bit :docstring "ISO 2022 based 8-bit encoding for Cyrillic script (MIME:ISO-8859-5)." :coding-type charset :mnemonic 53 :charset-list (iso-8859-5) :mime-charset iso-8859-5))
    (cyrillic-koi8 . (:ascii-compatible-p t :category coding-category-charset :name cyrillic-koi8 :docstring "KOI8 8-bit encoding for Cyrillic (MIME: KOI8-R)." :coding-type charset :mnemonic 82 :charset-list (koi8) :mime-charset koi8-r))
    (devanagari . (:ascii-compatible-p t :category coding-category-iso-8-1 :name in-is13194-devanagari :docstring "8-bit encoding for ASCII (MSB=0) and IS13194-Devanagari (MSB=1)." :coding-type iso-2022 :mnemonic 68 :designation [ascii indian-is13194 nil nil] :charset-list (ascii indian-is13194) :post-read-conversion in-is13194-post-read-conversion :pre-write-conversion in-is13194-pre-write-conversion))
    (ebcdic-be . (:ascii-compatible-p nil :category coding-category-charset :name ibm274 :docstring "Belgian version of EBCDIC" :coding-type charset :charset-list (ibm274) :mnemonic 42))
    (ebcdic-br . (:ascii-compatible-p nil :category coding-category-charset :name ibm275 :docstring "Brazilian version of EBCDIC" :coding-type charset :charset-list (ibm275) :mnemonic 42))
    (ebcdic-cp-dk . (:ascii-compatible-p nil :category coding-category-charset :name ibm277 :docstring "Danish / Norwegian version of EBCDIC" :coding-type charset :charset-list (ibm277) :mnemonic 42))
    (ebcdic-cp-es . (:ascii-compatible-p nil :category coding-category-charset :name ibm284 :docstring "Spanish version of EBCDIC" :coding-type charset :charset-list (ibm284) :mnemonic 42))
    (ebcdic-cp-fi . (:ascii-compatible-p nil :category coding-category-charset :name ibm278 :docstring "Finnish / Swedish version of EBCDIC" :coding-type charset :charset-list (ibm278) :mnemonic 42))
    (ebcdic-cp-fr . (:ascii-compatible-p nil :category coding-category-charset :name ibm297 :docstring "French version of EBCDIC" :coding-type charset :charset-list (ibm297) :mnemonic 42))
    (ebcdic-cp-gb . (:ascii-compatible-p nil :category coding-category-charset :name ibm285 :docstring "UK English version of EBCDIC" :coding-type charset :charset-list (ibm285) :mnemonic 42))
    (ebcdic-cp-it . (:ascii-compatible-p nil :category coding-category-charset :name ibm280 :docstring "Italian version of EBCDIC" :coding-type charset :charset-list (ibm280) :mnemonic 42))
    (ebcdic-cp-no . (:ascii-compatible-p nil :category coding-category-charset :name ibm277 :docstring "Danish / Norwegian version of EBCDIC" :coding-type charset :charset-list (ibm277) :mnemonic 42))
    (ebcdic-cp-se . (:ascii-compatible-p nil :category coding-category-charset :name ibm278 :docstring "Finnish / Swedish version of EBCDIC" :coding-type charset :charset-list (ibm278) :mnemonic 42))
    (ebcdic-int . (:ascii-compatible-p nil :category coding-category-charset :name ibm038 :docstring "International version of EBCDIC" :coding-type charset :charset-list (ibm038) :mnemonic 42))
    (ebcdic-int1 . (:ascii-compatible-p nil :category coding-category-charset :name ibm256 :docstring "Netherlands version of EBCDIC" :coding-type charset :charset-list (ibm256) :mnemonic 42))
    (ebcdic-jp-e . (:ascii-compatible-p nil :category coding-category-charset :name ibm281 :docstring "Japanese-E version of EBCDIC" :coding-type charset :charset-list (ibm281) :mnemonic 42))
    (ebcdic-jp-kana . (:ascii-compatible-p nil :category coding-category-charset :name ibm290 :docstring "Japanese katakana version of EBCDIC" :coding-type charset :charset-list (ibm290) :mnemonic 42))
    (ebcdic-uk . (:ascii-compatible-p nil :category coding-category-charset :name ebcdic-uk :docstring "UK version of EBCDIC" :coding-type charset :charset-list (ebcdic-uk) :mnemonic 42))
    (ebcdic-us . (:ascii-compatible-p nil :category coding-category-charset :name ebcdic-us :docstring "US version of EBCDIC" :coding-type charset :charset-list (ebcdic-us) :mnemonic 42))
    (emacs-mule . (:ascii-compatible-p t :category coding-category-emacs-mule :name emacs-mule :docstring "Emacs 21 internal format used in buffer and string." :coding-type emacs-mule :charset-list emacs-mule :mnemonic 77))
    (euc-china . (:ascii-compatible-p t :category coding-category-iso-8-2 :name chinese-iso-8bit :docstring "ISO 2022 based EUC encoding for Chinese GB2312 (MIME:GB2312)." :coding-type iso-2022 :mnemonic 99 :charset-list (ascii chinese-gb2312) :designation [ascii chinese-gb2312 nil nil] :mime-charset gb2312))
    (euc-cn . (:ascii-compatible-p t :category coding-category-iso-8-2 :name chinese-iso-8bit :docstring "ISO 2022 based EUC encoding for Chinese GB2312 (MIME:GB2312)." :coding-type iso-2022 :mnemonic 99 :charset-list (ascii chinese-gb2312) :designation [ascii chinese-gb2312 nil nil] :mime-charset gb2312))
    (euc-japan . (:ascii-compatible-p t :category coding-category-iso-8-2 :name japanese-iso-8bit :docstring "ISO 2022 based EUC encoding for Japanese (MIME:EUC-JP)." :coding-type iso-2022 :mnemonic 69 :designation [ascii japanese-jisx0208 katakana-jisx0201 japanese-jisx0212] :flags (short ascii-at-eol ascii-at-cntl single-shift) :charset-list (ascii latin-jisx0201 japanese-jisx0208 katakana-jisx0201 japanese-jisx0212 japanese-jisx0208-1978) :mime-charset euc-jp))
    (euc-japan-1990 . (:ascii-compatible-p t :category coding-category-iso-8-2 :name japanese-iso-8bit :docstring "ISO 2022 based EUC encoding for Japanese (MIME:EUC-JP)." :coding-type iso-2022 :mnemonic 69 :designation [ascii japanese-jisx0208 katakana-jisx0201 japanese-jisx0212] :flags (short ascii-at-eol ascii-at-cntl single-shift) :charset-list (ascii latin-jisx0201 japanese-jisx0208 katakana-jisx0201 japanese-jisx0212 japanese-jisx0208-1978) :mime-charset euc-jp))
    (euc-jis-2004 . (:ascii-compatible-p t :category coding-category-iso-8-2 :name euc-jis-2004 :docstring "ISO 2022 based EUC encoding for JIS X 0213 (MIME:EUC-JIS-2004)." :coding-type iso-2022 :mnemonic 69 :designation [ascii japanese-jisx0213.2004-1 katakana-jisx0201 japanese-jisx0213-2] :flags (short ascii-at-eol ascii-at-cntl single-shift) :charset-list (ascii latin-jisx0201 japanese-jisx0213.2004-1 japanese-jisx0213-1 katakana-jisx0201 japanese-jisx0213-2) :mime-charset euc-jis-2004))
    (euc-jisx0213 . (:ascii-compatible-p t :category coding-category-iso-8-2 :name euc-jis-2004 :docstring "ISO 2022 based EUC encoding for JIS X 0213 (MIME:EUC-JIS-2004)." :coding-type iso-2022 :mnemonic 69 :designation [ascii japanese-jisx0213.2004-1 katakana-jisx0201 japanese-jisx0213-2] :flags (short ascii-at-eol ascii-at-cntl single-shift) :charset-list (ascii latin-jisx0201 japanese-jisx0213.2004-1 japanese-jisx0213-1 katakana-jisx0201 japanese-jisx0213-2) :mime-charset euc-jis-2004))
    (euc-jp . (:ascii-compatible-p t :category coding-category-iso-8-2 :name japanese-iso-8bit :docstring "ISO 2022 based EUC encoding for Japanese (MIME:EUC-JP)." :coding-type iso-2022 :mnemonic 69 :designation [ascii japanese-jisx0208 katakana-jisx0201 japanese-jisx0212] :flags (short ascii-at-eol ascii-at-cntl single-shift) :charset-list (ascii latin-jisx0201 japanese-jisx0208 katakana-jisx0201 japanese-jisx0212 japanese-jisx0208-1978) :mime-charset euc-jp))
    (euc-korea . (:ascii-compatible-p t :category coding-category-iso-8-2 :name korean-iso-8bit :docstring "ISO 2022 based EUC encoding for Korean KSC5601 (MIME:EUC-KR)." :coding-type iso-2022 :mnemonic 75 :designation [ascii korean-ksc5601 nil nil] :charset-list (ascii korean-ksc5601) :mime-charset euc-kr))
    (euc-kr . (:ascii-compatible-p t :category coding-category-iso-8-2 :name korean-iso-8bit :docstring "ISO 2022 based EUC encoding for Korean KSC5601 (MIME:EUC-KR)." :coding-type iso-2022 :mnemonic 75 :designation [ascii korean-ksc5601 nil nil] :charset-list (ascii korean-ksc5601) :mime-charset euc-kr))
    (euc-taiwan . (:ascii-compatible-p t :category coding-category-iso-8-2 :name euc-tw :docstring "ISO 2022 based EUC encoding for Chinese CNS11643." :coding-type iso-2022 :mnemonic 90 :charset-list (ascii chinese-cns11643-1 chinese-cns11643-2 chinese-cns11643-3 chinese-cns11643-4 chinese-cns11643-5 chinese-cns11643-6 chinese-cns11643-7) :designation [ascii chinese-cns11643-1 (chinese-cns11643-1 chinese-cns11643-2 chinese-cns11643-3 chinese-cns11643-4 chinese-cns11643-5 chinese-cns11643-6 chinese-cns11643-7) nil] :mime-charset euc-tw))
    (euc-tw . (:ascii-compatible-p t :category coding-category-iso-8-2 :name euc-tw :docstring "ISO 2022 based EUC encoding for Chinese CNS11643." :coding-type iso-2022 :mnemonic 90 :charset-list (ascii chinese-cns11643-1 chinese-cns11643-2 chinese-cns11643-3 chinese-cns11643-4 chinese-cns11643-5 chinese-cns11643-6 chinese-cns11643-7) :designation [ascii chinese-cns11643-1 (chinese-cns11643-1 chinese-cns11643-2 chinese-cns11643-3 chinese-cns11643-4 chinese-cns11643-5 chinese-cns11643-6 chinese-cns11643-7) nil] :mime-charset euc-tw))
    (eucjp-ms . (:ascii-compatible-p t :category coding-category-iso-8-2 :name eucjp-ms :docstring "eucJP-ms (like EUC-JP but with CP932 extension).
    eucJP-ms is defined in <http://www.opengroup.or.jp/jvc/cde/appendix.html>." :coding-type iso-2022 :mnemonic 69 :designation [ascii japanese-jisx0208 katakana-jisx0201 japanese-jisx0212] :flags (short ascii-at-eol ascii-at-cntl single-shift) :charset-list (ascii latin-jisx0201 japanese-jisx0208 katakana-jisx0201 japanese-jisx0212) :decode-translation-table eucjp-ms-decode :encode-translation-table eucjp-ms-encode))
    (gb18030 . (:ascii-compatible-p t :category coding-category-charset :name chinese-gb18030 :docstring "GB18030 encoding for Chinese (MIME:GB18030)." :coding-type charset :mnemonic 99 :charset-list (ascii gb18030-2-byte gb18030-4-byte-bmp gb18030-4-byte-smp gb18030-4-byte-ext-1 gb18030-4-byte-ext-2) :mime-charset gb18030))
    (gb2312 . (:ascii-compatible-p t :category coding-category-iso-8-2 :name chinese-iso-8bit :docstring "ISO 2022 based EUC encoding for Chinese GB2312 (MIME:GB2312)." :coding-type iso-2022 :mnemonic 99 :charset-list (ascii chinese-gb2312) :designation [ascii chinese-gb2312 nil nil] :mime-charset gb2312))
    (gbk . (:ascii-compatible-p t :category coding-category-charset :name chinese-gbk :docstring "GBK encoding for Chinese (MIME:GBK)." :coding-type charset :mnemonic 99 :charset-list (ascii chinese-gbk) :mime-charset gbk))
    (georgian-academy . (:ascii-compatible-p t :category coding-category-charset :name georgian-academy :docstring "Georgian Academy encoding" :coding-type charset :mnemonic 71 :charset-list (georgian-academy)))
    (georgian-ps . (:ascii-compatible-p t :category coding-category-charset :name georgian-ps :docstring "Georgian PS encoding" :coding-type charset :mnemonic 71 :charset-list (georgian-ps)))
    (greek-iso-8bit . (:ascii-compatible-p t :category coding-category-charset :name greek-iso-8bit :docstring "ISO 2022 based 8-bit encoding for Greek (MIME:ISO-8859-7)." :coding-type charset :mnemonic 55 :charset-list (iso-8859-7) :mime-charset iso-8859-7))
    (hebrew-iso-8bit . (:ascii-compatible-p t :category coding-category-charset :name hebrew-iso-8bit :docstring "ISO 2022 based 8-bit encoding for Hebrew (MIME:ISO-8859-8)." :coding-type charset :mnemonic 56 :charset-list (iso-8859-8) :mime-charset iso-8859-8))
    (hp-roman8 . (:ascii-compatible-p t :category coding-category-charset :name hp-roman8 :docstring "Hewlet-Packard roman-8 encoding (MIME:ROMAN-8)" :coding-type charset :mnemonic 42 :charset-list (hp-roman8) :mime-charset hp-roman8))
    (hz . (:ascii-compatible-p nil :category coding-category-utf-8 :name chinese-hz :docstring "Hz/ZW 7-bit encoding for Chinese GB2312 (MIME:HZ-GB-2312)." :coding-type utf-8 :mnemonic 122 :charset-list (ascii chinese-gb2312) :mime-charset hz-gb-2312 :post-read-conversion post-read-decode-hz :pre-write-conversion pre-write-encode-hz))
    (hz-gb-2312 . (:ascii-compatible-p nil :category coding-category-utf-8 :name chinese-hz :docstring "Hz/ZW 7-bit encoding for Chinese GB2312 (MIME:HZ-GB-2312)." :coding-type utf-8 :mnemonic 122 :charset-list (ascii chinese-gb2312) :mime-charset hz-gb-2312 :post-read-conversion post-read-decode-hz :pre-write-conversion pre-write-encode-hz))
    (ibm038 . (:ascii-compatible-p nil :category coding-category-charset :name ibm038 :docstring "International version of EBCDIC" :coding-type charset :charset-list (ibm038) :mnemonic 42))
    (ibm1047 . (:ascii-compatible-p nil :category coding-category-charset :name ibm1047 :docstring "A version of EBCDIC used in OS/390 Unix" :coding-type charset :charset-list (ibm1047) :mnemonic 42))
    (ibm256 . (:ascii-compatible-p nil :category coding-category-charset :name ibm256 :docstring "Netherlands version of EBCDIC" :coding-type charset :charset-list (ibm256) :mnemonic 42))
    (ibm273 . (:ascii-compatible-p nil :category coding-category-charset :name ibm273 :docstring "Austrian / German version of EBCDIC" :coding-type charset :charset-list (ibm273) :mnemonic 42))
    (ibm274 . (:ascii-compatible-p nil :category coding-category-charset :name ibm274 :docstring "Belgian version of EBCDIC" :coding-type charset :charset-list (ibm274) :mnemonic 42))
    (ibm275 . (:ascii-compatible-p nil :category coding-category-charset :name ibm275 :docstring "Brazilian version of EBCDIC" :coding-type charset :charset-list (ibm275) :mnemonic 42))
    (ibm277 . (:ascii-compatible-p nil :category coding-category-charset :name ibm277 :docstring "Danish / Norwegian version of EBCDIC" :coding-type charset :charset-list (ibm277) :mnemonic 42))
    (ibm278 . (:ascii-compatible-p nil :category coding-category-charset :name ibm278 :docstring "Finnish / Swedish version of EBCDIC" :coding-type charset :charset-list (ibm278) :mnemonic 42))
    (ibm280 . (:ascii-compatible-p nil :category coding-category-charset :name ibm280 :docstring "Italian version of EBCDIC" :coding-type charset :charset-list (ibm280) :mnemonic 42))
    (ibm281 . (:ascii-compatible-p nil :category coding-category-charset :name ibm281 :docstring "Japanese-E version of EBCDIC" :coding-type charset :charset-list (ibm281) :mnemonic 42))
    (ibm284 . (:ascii-compatible-p nil :category coding-category-charset :name ibm284 :docstring "Spanish version of EBCDIC" :coding-type charset :charset-list (ibm284) :mnemonic 42))
    (ibm285 . (:ascii-compatible-p nil :category coding-category-charset :name ibm285 :docstring "UK English version of EBCDIC" :coding-type charset :charset-list (ibm285) :mnemonic 42))
    (ibm290 . (:ascii-compatible-p nil :category coding-category-charset :name ibm290 :docstring "Japanese katakana version of EBCDIC" :coding-type charset :charset-list (ibm290) :mnemonic 42))
    (ibm297 . (:ascii-compatible-p nil :category coding-category-charset :name ibm297 :docstring "French version of EBCDIC" :coding-type charset :charset-list (ibm297) :mnemonic 42))
    (ibm437 . (:ascii-compatible-p t :category coding-category-charset :name cp437 :docstring "DOS codepage 437" :coding-type charset :mnemonic 68 :charset-list (cp437) :mime-charset cp437))
    (ibm775 . (:ascii-compatible-p t :category coding-category-charset :name cp775 :docstring "DOS codepage 775 (PC Baltic, MS-DOS Baltic Rim)" :coding-type charset :mnemonic 68 :charset-list (cp775) :mime-charset cp775))
    (ibm850 . (:ascii-compatible-p t :category coding-category-charset :name cp850 :docstring "DOS codepage 850 (Western European)" :coding-type charset :mnemonic 68 :charset-list (cp850) :mime-charset cp850))
    (ibm851 . (:ascii-compatible-p t :category coding-category-charset :name cp851 :docstring "DOS codepage 851 (Greek)" :coding-type charset :mnemonic 68 :charset-list (cp851) :mime-charset cp851))
    (ibm852 . (:ascii-compatible-p t :category coding-category-charset :name cp852 :docstring "DOS codepage 852 (Slavic)" :coding-type charset :mnemonic 68 :charset-list (cp852) :mime-charset cp852))
    (ibm855 . (:ascii-compatible-p t :category coding-category-charset :name cp855 :docstring "DOS codepage 855 (Russian)" :coding-type charset :mnemonic 68 :charset-list (cp855) :mime-charset cp855))
    (ibm857 . (:ascii-compatible-p t :category coding-category-charset :name cp857 :docstring "DOS codepage 857 (Turkish)" :coding-type charset :mnemonic 68 :charset-list (cp857) :mime-charset cp857))
    (ibm860 . (:ascii-compatible-p t :category coding-category-charset :name cp860 :docstring "DOS codepage 860 (Portuguese)" :coding-type charset :mnemonic 68 :charset-list (cp860) :mime-charset cp860))
    (ibm861 . (:ascii-compatible-p t :category coding-category-charset :name cp861 :docstring "DOS codepage 861 (Icelandic)" :coding-type charset :mnemonic 68 :charset-list (cp861) :mime-charset cp861))
    (ibm862 . (:ascii-compatible-p t :category coding-category-charset :name cp862 :docstring "DOS codepage 862 (Hebrew)" :coding-type charset :mnemonic 68 :charset-list (cp862) :mime-charset cp862))
    (ibm863 . (:ascii-compatible-p t :category coding-category-charset :name cp863 :docstring "DOS codepage 863 (French Canadian)" :coding-type charset :mnemonic 68 :charset-list (cp863) :mime-charset cp863))
    (ibm865 . (:ascii-compatible-p t :category coding-category-charset :name cp865 :docstring "DOS codepage 865 (Norwegian/Danish)" :coding-type charset :mnemonic 68 :charset-list (cp865) :mime-charset cp865))
    (ibm869 . (:ascii-compatible-p t :category coding-category-charset :name cp869 :docstring "DOS codepage 869 (Greek)" :coding-type charset :mnemonic 68 :charset-list (cp869) :mime-charset cp869))
    (ibm874 . (:ascii-compatible-p t :category coding-category-charset :name cp874 :docstring "DOS codepage 874 (Thai)" :coding-type charset :mnemonic 68 :charset-list (cp874) :mime-charset cp874))
    (in-is13194-devanagari . (:ascii-compatible-p t :category coding-category-iso-8-1 :name in-is13194-devanagari :docstring "8-bit encoding for ASCII (MSB=0) and IS13194-Devanagari (MSB=1)." :coding-type iso-2022 :mnemonic 68 :designation [ascii indian-is13194 nil nil] :charset-list (ascii indian-is13194) :post-read-conversion in-is13194-post-read-conversion :pre-write-conversion in-is13194-pre-write-conversion))
    (iso-2022-7bit . (:ascii-compatible-p nil :category coding-category-iso-7 :name iso-2022-7bit :docstring "ISO 2022 based 7-bit encoding using only G0." :coding-type iso-2022 :mnemonic 74 :charset-list iso-2022 :designation [(ascii t) nil nil nil] :flags (short ascii-at-eol ascii-at-cntl 7-bit designation composition)))
    (iso-2022-7bit-lock . (:ascii-compatible-p nil :category coding-category-iso-7-else :name iso-2022-7bit-lock :docstring "ISO-2022 coding system using Locking-Shift for 96-charset." :coding-type iso-2022 :mnemonic 38 :charset-list iso-2022 :designation [(ascii 94) (nil 96) nil nil] :flags (ascii-at-eol ascii-at-cntl 7-bit designation locking-shift composition)))
    (iso-2022-7bit-lock-ss2 . (:ascii-compatible-p nil :category coding-category-iso-7-else :name iso-2022-7bit-lock-ss2 :docstring "Mixture of ISO-2022-JP, ISO-2022-KR, and ISO-2022-CN." :coding-type iso-2022 :mnemonic 105 :charset-list (ascii japanese-jisx0208 japanese-jisx0208-1978 latin-jisx0201 korean-ksc5601 chinese-gb2312 chinese-cns11643-1 chinese-cns11643-2 chinese-cns11643-3 chinese-cns11643-4 chinese-cns11643-5 chinese-cns11643-6 chinese-cns11643-7) :designation [(ascii 94) (nil korean-ksc5601 chinese-gb2312 chinese-cns11643-1 96) (nil chinese-cns11643-2) (nil chinese-cns11643-3 chinese-cns11643-4 chinese-cns11643-5 chinese-cns11643-6 chinese-cns11643-7)] :flags (short ascii-at-eol ascii-at-cntl 7-bit locking-shift single-shift init-bol)))
    (iso-2022-7bit-ss2 . (:ascii-compatible-p nil :category coding-category-iso-7-else :name iso-2022-7bit-ss2 :docstring "ISO 2022 based 7-bit encoding using SS2 for 96-charset." :coding-type iso-2022 :mnemonic 36 :charset-list iso-2022 :designation [(ascii 94) nil (nil 96) nil] :flags (short ascii-at-eol ascii-at-cntl 7-bit designation single-shift composition)))
    (iso-2022-8bit-ss2 . (:ascii-compatible-p nil :category coding-category-iso-8-else :name iso-2022-8bit-ss2 :docstring "ISO 2022 based 8-bit encoding using SS2 for 96-charset." :coding-type iso-2022 :mnemonic 64 :charset-list iso-2022 :designation [(ascii 94) nil (nil 96) nil] :flags (ascii-at-eol ascii-at-cntl designation single-shift composition)))
    (iso-2022-cjk . (:ascii-compatible-p nil :category coding-category-iso-7-else :name iso-2022-7bit-lock-ss2 :docstring "Mixture of ISO-2022-JP, ISO-2022-KR, and ISO-2022-CN." :coding-type iso-2022 :mnemonic 105 :charset-list (ascii japanese-jisx0208 japanese-jisx0208-1978 latin-jisx0201 korean-ksc5601 chinese-gb2312 chinese-cns11643-1 chinese-cns11643-2 chinese-cns11643-3 chinese-cns11643-4 chinese-cns11643-5 chinese-cns11643-6 chinese-cns11643-7) :designation [(ascii 94) (nil korean-ksc5601 chinese-gb2312 chinese-cns11643-1 96) (nil chinese-cns11643-2) (nil chinese-cns11643-3 chinese-cns11643-4 chinese-cns11643-5 chinese-cns11643-6 chinese-cns11643-7)] :flags (short ascii-at-eol ascii-at-cntl 7-bit locking-shift single-shift init-bol)))
    (iso-2022-cn . (:ascii-compatible-p nil :category coding-category-iso-7-else :name iso-2022-cn :docstring "ISO 2022 based 7bit encoding for Chinese GB and CNS (MIME:ISO-2022-CN)." :coding-type iso-2022 :mnemonic 67 :charset-list (ascii chinese-gb2312 chinese-cns11643-1 chinese-cns11643-2) :designation [ascii (nil chinese-gb2312 chinese-cns11643-1) (nil chinese-cns11643-2) nil] :flags (ascii-at-eol ascii-at-cntl 7-bit designation locking-shift single-shift init-at-bol) :mime-charset iso-2022-cn :suitable-for-keyboard t))
    (iso-2022-cn-ext . (:ascii-compatible-p nil :category coding-category-iso-7-else :name iso-2022-cn-ext :docstring "ISO 2022 based 7bit encoding for Chinese GB and CNS (MIME:ISO-2022-CN-EXT)." :coding-type iso-2022 :mnemonic 67 :charset-list (ascii chinese-gb2312 chinese-cns11643-1 chinese-cns11643-2 chinese-cns11643-3 chinese-cns11643-4 chinese-cns11643-5 chinese-cns11643-6 chinese-cns11643-7) :designation [ascii (nil chinese-gb2312 chinese-cns11643-1) (nil chinese-cns11643-2) (nil chinese-cns11643-3 chinese-cns11643-4 chinese-cns11643-5 chinese-cns11643-6 chinese-cns11643-7)] :flags (ascii-at-eol ascii-at-cntl 7-bit designation locking-shift single-shift init-at-bol) :mime-charset iso-2022-cn-ext :suitable-for-keyboard t))
    (iso-2022-int-1 . (:ascii-compatible-p nil :category coding-category-iso-7-else :name iso-2022-7bit-lock :docstring "ISO-2022 coding system using Locking-Shift for 96-charset." :coding-type iso-2022 :mnemonic 38 :charset-list iso-2022 :designation [(ascii 94) (nil 96) nil nil] :flags (ascii-at-eol ascii-at-cntl 7-bit designation locking-shift composition)))
    (iso-2022-jp . (:ascii-compatible-p nil :category coding-category-iso-7-tight :name iso-2022-jp :docstring "ISO 2022 based 7bit encoding for Japanese (MIME:ISO-2022-JP)." :coding-type iso-2022 :mnemonic 74 :designation [(ascii japanese-jisx0208-1978 japanese-jisx0208 latin-jisx0201) nil nil nil] :flags (short ascii-at-eol ascii-at-cntl 7-bit designation) :charset-list (ascii japanese-jisx0208 japanese-jisx0208-1978 latin-jisx0201) :mime-charset iso-2022-jp :suitable-for-keyboard t))
    (iso-2022-jp-1978-irv . (:ascii-compatible-p nil :category coding-category-iso-7-tight :name japanese-iso-7bit-1978-irv :docstring "ISO 2022 based 7-bit encoding for Japanese JISX0208-1978 and JISX0201-Roman." :coding-type iso-2022 :mnemonic 106 :designation [(latin-jisx0201 japanese-jisx0208-1978 japanese-jisx0208 japanese-jisx0212 katakana-jisx0201) nil nil nil] :flags (short ascii-at-eol ascii-at-cntl 7-bit designation use-roman use-oldjis) :charset-list (ascii latin-jisx0201 japanese-jisx0208-1978 japanese-jisx0208 japanese-jisx0212)))
    (iso-2022-jp-2 . (:ascii-compatible-p nil :category coding-category-iso-7-else :name iso-2022-jp-2 :docstring "ISO 2022 based 7bit encoding for CJK, Latin-1, Greek (MIME:ISO-2022-JP-2)." :coding-type iso-2022 :mnemonic 74 :designation [(ascii japanese-jisx0208-1978 japanese-jisx0208 latin-jisx0201 japanese-jisx0212 chinese-gb2312 korean-ksc5601) nil (nil latin-iso8859-1 greek-iso8859-7) nil] :flags (short ascii-at-eol ascii-at-cntl 7-bit designation single-shift init-at-bol) :charset-list (ascii japanese-jisx0208 japanese-jisx0212 latin-jisx0201 japanese-jisx0208-1978 chinese-gb2312 korean-ksc5601 latin-iso8859-1 greek-iso8859-7) :mime-charset iso-2022-jp-2 :suitable-for-keyboard t))
    (iso-2022-jp-2004 . (:ascii-compatible-p nil :category coding-category-iso-7-tight :name iso-2022-jp-2004 :docstring "ISO 2022 based 7bit encoding for JIS X 0213:2004 (MIME:ISO-2022-JP-2004)." :coding-type iso-2022 :mnemonic 74 :designation [(ascii japanese-jisx0208 japanese-jisx0213.2004-1 japanese-jisx0213-1 japanese-jisx0213-2) nil nil nil] :flags (short ascii-at-eol ascii-at-cntl 7-bit designation) :charset-list (ascii japanese-jisx0208 japanese-jisx0213.2004-1 japanese-jisx0213-1 japanese-jisx0213-2) :mime-charset iso-2022-jp-2004 :suitable-for-keyboard t))
    (iso-2022-jp-3 . (:ascii-compatible-p nil :category coding-category-iso-7-tight :name iso-2022-jp-2004 :docstring "ISO 2022 based 7bit encoding for JIS X 0213:2004 (MIME:ISO-2022-JP-2004)." :coding-type iso-2022 :mnemonic 74 :designation [(ascii japanese-jisx0208 japanese-jisx0213.2004-1 japanese-jisx0213-1 japanese-jisx0213-2) nil nil nil] :flags (short ascii-at-eol ascii-at-cntl 7-bit designation) :charset-list (ascii japanese-jisx0208 japanese-jisx0213.2004-1 japanese-jisx0213-1 japanese-jisx0213-2) :mime-charset iso-2022-jp-2004 :suitable-for-keyboard t))
    (iso-2022-kr . (:ascii-compatible-p nil :category coding-category-iso-7-else :name iso-2022-kr :docstring "ISO 2022 based 7-bit encoding for Korean KSC5601 (MIME:ISO-2022-KR)." :coding-type iso-2022 :mnemonic 107 :designation [ascii (nil korean-ksc5601) nil nil] :flags (ascii-at-eol ascii-at-cntl 7-bit designation locking-shift designation-bol) :charset-list (ascii korean-ksc5601) :mime-charset iso-2022-kr :suitable-for-keyboard t))
    (iso-8859-1 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-1 :docstring "ISO 2022 based 8-bit encoding for Latin-1 (MIME:ISO-8859-1)." :coding-type charset :mnemonic 49 :charset-list (iso-8859-1) :mime-charset iso-8859-1))
    (iso-8859-10 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-6 :docstring "ISO 2022 based 8-bit encoding for Latin-6 (MIME:ISO-8859-10)." :coding-type charset :mnemonic 57 :charset-list (iso-8859-10) :mime-charset iso-8859-10))
    (iso-8859-11 . (:ascii-compatible-p t :category coding-category-charset :name iso-8859-11 :docstring "ISO/IEC 8859/11 (Latin/Thai)
    This is the same as `thai-tis620' with the addition of no-break-space." :coding-type charset :mnemonic 42 :mime-charset iso-8859-11 :charset-list (iso-8859-11)))
    (iso-8859-13 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-7 :docstring "ISO 2022 based 8-bit encoding for Latin-7 (MIME:ISO-8859-13)." :coding-type charset :mnemonic 57 :charset-list (iso-8859-13) :mime-charset iso-8859-13))
    (iso-8859-14 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-8 :docstring "ISO 2022 based 8-bit encoding for Latin-8 (MIME:ISO-8859-14)." :coding-type charset :mnemonic 87 :charset-list (iso-8859-14) :mime-charset iso-8859-14))
    (iso-8859-15 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-9 :docstring "ISO 2022 based 8-bit encoding for Latin-9 (MIME:ISO-8859-15)." :coding-type charset :mnemonic 48 :charset-list (iso-8859-15) :mime-charset iso-8859-15))
    (iso-8859-16 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-10 :docstring "ISO 2022 based 8-bit encoding for Latin-10." :coding-type charset :mnemonic 42 :charset-list (iso-8859-16) :mime-charset iso-8859-16))
    (iso-8859-2 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-2 :docstring "ISO 2022 based 8-bit encoding for Latin-2 (MIME:ISO-8859-2)." :coding-type charset :mnemonic 50 :charset-list (iso-8859-2) :mime-charset iso-8859-2))
    (iso-8859-3 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-3 :docstring "ISO 2022 based 8-bit encoding for Latin-3 (MIME:ISO-8859-3)." :coding-type charset :mnemonic 51 :charset-list (iso-8859-3) :mime-charset iso-8859-3))
    (iso-8859-4 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-4 :docstring "ISO 2022 based 8-bit encoding for Latin-4 (MIME:ISO-8859-4)." :coding-type charset :mnemonic 52 :charset-list (iso-8859-4) :mime-charset iso-8859-4))
    (iso-8859-5 . (:ascii-compatible-p t :category coding-category-charset :name cyrillic-iso-8bit :docstring "ISO 2022 based 8-bit encoding for Cyrillic script (MIME:ISO-8859-5)." :coding-type charset :mnemonic 53 :charset-list (iso-8859-5) :mime-charset iso-8859-5))
    (iso-8859-6 . (:ascii-compatible-p t :category coding-category-charset :name iso-8859-6 :docstring "ISO-8859-6 based encoding (MIME:ISO-8859-6)." :coding-type charset :mnemonic 54 :charset-list (iso-8859-6) :mime-charset iso-8859-6))
    (iso-8859-7 . (:ascii-compatible-p t :category coding-category-charset :name greek-iso-8bit :docstring "ISO 2022 based 8-bit encoding for Greek (MIME:ISO-8859-7)." :coding-type charset :mnemonic 55 :charset-list (iso-8859-7) :mime-charset iso-8859-7))
    (iso-8859-8 . (:ascii-compatible-p t :category coding-category-charset :name hebrew-iso-8bit :docstring "ISO 2022 based 8-bit encoding for Hebrew (MIME:ISO-8859-8)." :coding-type charset :mnemonic 56 :charset-list (iso-8859-8) :mime-charset iso-8859-8))
    (iso-8859-8-e . (:ascii-compatible-p t :category coding-category-charset :name hebrew-iso-8bit :docstring "ISO 2022 based 8-bit encoding for Hebrew (MIME:ISO-8859-8)." :coding-type charset :mnemonic 56 :charset-list (iso-8859-8) :mime-charset iso-8859-8))
    (iso-8859-8-i . (:ascii-compatible-p t :category coding-category-charset :name hebrew-iso-8bit :docstring "ISO 2022 based 8-bit encoding for Hebrew (MIME:ISO-8859-8)." :coding-type charset :mnemonic 56 :charset-list (iso-8859-8) :mime-charset iso-8859-8))
    (iso-8859-9 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-5 :docstring "ISO 2022 based 8-bit encoding for Latin-5 (MIME:ISO-8859-9)." :coding-type charset :mnemonic 57 :charset-list (iso-8859-9) :mime-charset iso-8859-9))
    (iso-latin-1 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-1 :docstring "ISO 2022 based 8-bit encoding for Latin-1 (MIME:ISO-8859-1)." :coding-type charset :mnemonic 49 :charset-list (iso-8859-1) :mime-charset iso-8859-1))
    (iso-latin-10 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-10 :docstring "ISO 2022 based 8-bit encoding for Latin-10." :coding-type charset :mnemonic 42 :charset-list (iso-8859-16) :mime-charset iso-8859-16))
    (iso-latin-2 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-2 :docstring "ISO 2022 based 8-bit encoding for Latin-2 (MIME:ISO-8859-2)." :coding-type charset :mnemonic 50 :charset-list (iso-8859-2) :mime-charset iso-8859-2))
    (iso-latin-3 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-3 :docstring "ISO 2022 based 8-bit encoding for Latin-3 (MIME:ISO-8859-3)." :coding-type charset :mnemonic 51 :charset-list (iso-8859-3) :mime-charset iso-8859-3))
    (iso-latin-4 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-4 :docstring "ISO 2022 based 8-bit encoding for Latin-4 (MIME:ISO-8859-4)." :coding-type charset :mnemonic 52 :charset-list (iso-8859-4) :mime-charset iso-8859-4))
    (iso-latin-5 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-5 :docstring "ISO 2022 based 8-bit encoding for Latin-5 (MIME:ISO-8859-9)." :coding-type charset :mnemonic 57 :charset-list (iso-8859-9) :mime-charset iso-8859-9))
    (iso-latin-6 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-6 :docstring "ISO 2022 based 8-bit encoding for Latin-6 (MIME:ISO-8859-10)." :coding-type charset :mnemonic 57 :charset-list (iso-8859-10) :mime-charset iso-8859-10))
    (iso-latin-7 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-7 :docstring "ISO 2022 based 8-bit encoding for Latin-7 (MIME:ISO-8859-13)." :coding-type charset :mnemonic 57 :charset-list (iso-8859-13) :mime-charset iso-8859-13))
    (iso-latin-8 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-8 :docstring "ISO 2022 based 8-bit encoding for Latin-8 (MIME:ISO-8859-14)." :coding-type charset :mnemonic 87 :charset-list (iso-8859-14) :mime-charset iso-8859-14))
    (iso-latin-9 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-9 :docstring "ISO 2022 based 8-bit encoding for Latin-9 (MIME:ISO-8859-15)." :coding-type charset :mnemonic 48 :charset-list (iso-8859-15) :mime-charset iso-8859-15))
    (iso-safe . (:ascii-compatible-p t :category coding-category-charset :name us-ascii :docstring "Encode ASCII as-is and encode non-ASCII characters to `?'." :coding-type charset :mnemonic 45 :charset-list (ascii) :default-char 63 :mime-charset us-ascii))
    (japanese-cp932 . (:ascii-compatible-p t :category coding-category-charset :name japanese-cp932 :docstring "CP932 (Microsoft shift-jis)" :coding-type charset :mnemonic 83 :charset-list (ascii katakana-sjis cp932-2-byte)))
    (japanese-iso-7bit-1978-irv . (:ascii-compatible-p nil :category coding-category-iso-7-tight :name japanese-iso-7bit-1978-irv :docstring "ISO 2022 based 7-bit encoding for Japanese JISX0208-1978 and JISX0201-Roman." :coding-type iso-2022 :mnemonic 106 :designation [(latin-jisx0201 japanese-jisx0208-1978 japanese-jisx0208 japanese-jisx0212 katakana-jisx0201) nil nil nil] :flags (short ascii-at-eol ascii-at-cntl 7-bit designation use-roman use-oldjis) :charset-list (ascii latin-jisx0201 japanese-jisx0208-1978 japanese-jisx0208 japanese-jisx0212)))
    (japanese-iso-8bit . (:ascii-compatible-p t :category coding-category-iso-8-2 :name japanese-iso-8bit :docstring "ISO 2022 based EUC encoding for Japanese (MIME:EUC-JP)." :coding-type iso-2022 :mnemonic 69 :designation [ascii japanese-jisx0208 katakana-jisx0201 japanese-jisx0212] :flags (short ascii-at-eol ascii-at-cntl single-shift) :charset-list (ascii latin-jisx0201 japanese-jisx0208 katakana-jisx0201 japanese-jisx0212 japanese-jisx0208-1978) :mime-charset euc-jp))
    (japanese-shift-jis . (:ascii-compatible-p t :category coding-category-sjis :name japanese-shift-jis :docstring "Shift-JIS 8-bit encoding for Japanese (MIME:SHIFT_JIS)" :coding-type shift-jis :mnemonic 83 :charset-list (ascii katakana-jisx0201 japanese-jisx0208) :mime-charset shift_jis))
    (japanese-shift-jis-2004 . (:ascii-compatible-p t :category coding-category-sjis :name japanese-shift-jis-2004 :docstring "Shift_JIS 8-bit encoding for Japanese (MIME:SHIFT_JIS-2004)" :coding-type shift-jis :mnemonic 83 :charset-list (ascii katakana-jisx0201 japanese-jisx0213.2004-1 japanese-jisx0213-2)))
    (junet . (:ascii-compatible-p nil :category coding-category-iso-7-tight :name iso-2022-jp :docstring "ISO 2022 based 7bit encoding for Japanese (MIME:ISO-2022-JP)." :coding-type iso-2022 :mnemonic 74 :designation [(ascii japanese-jisx0208-1978 japanese-jisx0208 latin-jisx0201) nil nil nil] :flags (short ascii-at-eol ascii-at-cntl 7-bit designation) :charset-list (ascii japanese-jisx0208 japanese-jisx0208-1978 latin-jisx0201) :mime-charset iso-2022-jp :suitable-for-keyboard t))
    (koi8 . (:ascii-compatible-p t :category coding-category-charset :name cyrillic-koi8 :docstring "KOI8 8-bit encoding for Cyrillic (MIME: KOI8-R)." :coding-type charset :mnemonic 82 :charset-list (koi8) :mime-charset koi8-r))
    (koi8-r . (:ascii-compatible-p t :category coding-category-charset :name cyrillic-koi8 :docstring "KOI8 8-bit encoding for Cyrillic (MIME: KOI8-R)." :coding-type charset :mnemonic 82 :charset-list (koi8) :mime-charset koi8-r))
    (koi8-t . (:ascii-compatible-p t :category coding-category-charset :name koi8-t :docstring "KOI8-T 8-bit encoding for Cyrillic" :coding-type charset :mnemonic 42 :charset-list (koi8-t) :mime-charset koi8-t))
    (koi8-u . (:ascii-compatible-p t :category coding-category-charset :name koi8-u :docstring "KOI8-U 8-bit encoding for Cyrillic (MIME: KOI8-U)" :coding-type charset :mnemonic 1059 :charset-list (koi8-u) :mime-charset koi8-u))
    (korean-cp949 . (:ascii-compatible-p t :category coding-category-charset :name korean-cp949 :docstring "CP949 (Microsoft Unified Hangul Code)" :coding-type charset :mnemonic 75 :charset-list (ascii cp949)))
    (korean-iso-7bit-lock . (:ascii-compatible-p nil :category coding-category-iso-7-else :name iso-2022-kr :docstring "ISO 2022 based 7-bit encoding for Korean KSC5601 (MIME:ISO-2022-KR)." :coding-type iso-2022 :mnemonic 107 :designation [ascii (nil korean-ksc5601) nil nil] :flags (ascii-at-eol ascii-at-cntl 7-bit designation locking-shift designation-bol) :charset-list (ascii korean-ksc5601) :mime-charset iso-2022-kr :suitable-for-keyboard t))
    (korean-iso-8bit . (:ascii-compatible-p t :category coding-category-iso-8-2 :name korean-iso-8bit :docstring "ISO 2022 based EUC encoding for Korean KSC5601 (MIME:EUC-KR)." :coding-type iso-2022 :mnemonic 75 :designation [ascii korean-ksc5601 nil nil] :charset-list (ascii korean-ksc5601) :mime-charset euc-kr))
    (ks_c_5601-1987 . (:ascii-compatible-p t :category coding-category-iso-8-2 :name korean-iso-8bit :docstring "ISO 2022 based EUC encoding for Korean KSC5601 (MIME:EUC-KR)." :coding-type iso-2022 :mnemonic 75 :designation [ascii korean-ksc5601 nil nil] :charset-list (ascii korean-ksc5601) :mime-charset euc-kr))
    (lao . (:ascii-compatible-p nil :category coding-category-charset :name lao :docstring "8-bit encoding for ASCII (MSB=0) and LAO (MSB=1)." :coding-type charset :mnemonic 76 :charset-list (lao)))
    (latin-0 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-9 :docstring "ISO 2022 based 8-bit encoding for Latin-9 (MIME:ISO-8859-15)." :coding-type charset :mnemonic 48 :charset-list (iso-8859-15) :mime-charset iso-8859-15))
    (latin-1 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-1 :docstring "ISO 2022 based 8-bit encoding for Latin-1 (MIME:ISO-8859-1)." :coding-type charset :mnemonic 49 :charset-list (iso-8859-1) :mime-charset iso-8859-1))
    (latin-10 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-10 :docstring "ISO 2022 based 8-bit encoding for Latin-10." :coding-type charset :mnemonic 42 :charset-list (iso-8859-16) :mime-charset iso-8859-16))
    (latin-2 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-2 :docstring "ISO 2022 based 8-bit encoding for Latin-2 (MIME:ISO-8859-2)." :coding-type charset :mnemonic 50 :charset-list (iso-8859-2) :mime-charset iso-8859-2))
    (latin-3 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-3 :docstring "ISO 2022 based 8-bit encoding for Latin-3 (MIME:ISO-8859-3)." :coding-type charset :mnemonic 51 :charset-list (iso-8859-3) :mime-charset iso-8859-3))
    (latin-4 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-4 :docstring "ISO 2022 based 8-bit encoding for Latin-4 (MIME:ISO-8859-4)." :coding-type charset :mnemonic 52 :charset-list (iso-8859-4) :mime-charset iso-8859-4))
    (latin-5 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-5 :docstring "ISO 2022 based 8-bit encoding for Latin-5 (MIME:ISO-8859-9)." :coding-type charset :mnemonic 57 :charset-list (iso-8859-9) :mime-charset iso-8859-9))
    (latin-6 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-6 :docstring "ISO 2022 based 8-bit encoding for Latin-6 (MIME:ISO-8859-10)." :coding-type charset :mnemonic 57 :charset-list (iso-8859-10) :mime-charset iso-8859-10))
    (latin-7 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-7 :docstring "ISO 2022 based 8-bit encoding for Latin-7 (MIME:ISO-8859-13)." :coding-type charset :mnemonic 57 :charset-list (iso-8859-13) :mime-charset iso-8859-13))
    (latin-8 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-8 :docstring "ISO 2022 based 8-bit encoding for Latin-8 (MIME:ISO-8859-14)." :coding-type charset :mnemonic 87 :charset-list (iso-8859-14) :mime-charset iso-8859-14))
    (latin-9 . (:ascii-compatible-p t :category coding-category-charset :name iso-latin-9 :docstring "ISO 2022 based 8-bit encoding for Latin-9 (MIME:ISO-8859-15)." :coding-type charset :mnemonic 48 :charset-list (iso-8859-15) :mime-charset iso-8859-15))
    (mac-roman . (:ascii-compatible-p t :category coding-category-charset :name mac-roman :docstring "Mac Roman Encoding (MIME:MACINTOSH)." :coding-type charset :mnemonic 77 :charset-list (mac-roman) :mime-charset macintosh))
    (macintosh . (:ascii-compatible-p t :category coding-category-charset :name mac-roman :docstring "Mac Roman Encoding (MIME:MACINTOSH)." :coding-type charset :mnemonic 77 :charset-list (mac-roman) :mime-charset macintosh))
    (mik . (:ascii-compatible-p t :category coding-category-charset :name mik :docstring "Bulgarian DOS codepage" :coding-type charset :mnemonic 68 :charset-list (mik)))
    (mule-utf-8 . (:ascii-compatible-p t :category coding-category-utf-8 :name utf-8 :docstring "UTF-8 (no signature (BOM))" :coding-type utf-8 :mnemonic 85 :charset-list (unicode) :mime-charset utf-8))
    (next . (:ascii-compatible-p t :category coding-category-charset :name next :docstring "NeXTstep encoding" :coding-type charset :mnemonic 42 :charset-list (next) :mime-charset next))
    (no-conversion . (:ascii-compatible-p t :category coding-category-raw-text :name no-conversion :mnemonic 61 :coding-type raw-text :ascii-compatible-p t :default-char 0 :for-unibyte t :docstring "Do no conversion.
    
    When you visit a file with this coding, the file is read into a
    unibyte buffer as is, thus each byte of a file is treated as a
    character." :eol-type unix))
    (no-conversion-multibyte . (:ascii-compatible-p t :category coding-category-raw-text :name no-conversion-multibyte :docstring "Like `no-conversion' but don't read a file into a unibyte buffer." :coding-type raw-text :eol-type unix :mnemonic 61))
    (old-jis . (:ascii-compatible-p nil :category coding-category-iso-7-tight :name japanese-iso-7bit-1978-irv :docstring "ISO 2022 based 7-bit encoding for Japanese JISX0208-1978 and JISX0201-Roman." :coding-type iso-2022 :mnemonic 106 :designation [(latin-jisx0201 japanese-jisx0208-1978 japanese-jisx0208 japanese-jisx0212 katakana-jisx0201) nil nil nil] :flags (short ascii-at-eol ascii-at-cntl 7-bit designation use-roman use-oldjis) :charset-list (ascii latin-jisx0201 japanese-jisx0208-1978 japanese-jisx0208 japanese-jisx0212)))
    (prefer-utf-8 . (:ascii-compatible-p nil :category coding-category-undecided :name prefer-utf-8 :docstring "Like `undecided' but prefer UTF-8 when appropriate.
    On decoding, if the source contains 8-bit codes and they all
    are valid UTF-8 sequences, detect the source as UTF-8 encoding
    regardless of the coding priority.
    On encoding, if the source contains non-ASCII characters, encode them
    by UTF-8." :coding-type undecided :mnemonic 45 :charset-list (emacs) :prefer-utf-8 t :inhibit-null-byte-detection 0 :inhibit-iso-escape-detection 0))
    (pt154 . (:ascii-compatible-p t :category coding-category-charset :name pt154 :docstring "ParaType Asian Cyrillic codepage" :coding-type charset :mnemonic 68 :charset-list (pt154)))
    (raw-text . (:ascii-compatible-p t :category coding-category-raw-text :name raw-text :docstring "Raw text, which means text contains random 8-bit codes.
    Encoding text with this coding system produces the actual byte
    sequence of the text in buffers and strings.  An exception is made for
    characters from the `eight-bit' character set.  Each of them is encoded
    into a single byte.
    
    When you visit a file with this coding, the file is read into a
    unibyte buffer as is (except for EOL format), thus each byte of a file
    is treated as a character." :coding-type raw-text :for-unibyte t :mnemonic 116))
    (roman8 . (:ascii-compatible-p t :category coding-category-charset :name hp-roman8 :docstring "Hewlet-Packard roman-8 encoding (MIME:ROMAN-8)" :coding-type charset :mnemonic 42 :charset-list (hp-roman8) :mime-charset hp-roman8))
    (ruscii . (:ascii-compatible-p t :category coding-category-charset :name cp1125 :docstring "cp1125 8-bit encoding for Cyrillic" :coding-type charset :mnemonic 42 :charset-list (cp1125)))
    (shift_jis . (:ascii-compatible-p t :category coding-category-sjis :name japanese-shift-jis :docstring "Shift-JIS 8-bit encoding for Japanese (MIME:SHIFT_JIS)" :coding-type shift-jis :mnemonic 83 :charset-list (ascii katakana-jisx0201 japanese-jisx0208) :mime-charset shift_jis))
    (shift_jis-2004 . (:ascii-compatible-p t :category coding-category-sjis :name japanese-shift-jis-2004 :docstring "Shift_JIS 8-bit encoding for Japanese (MIME:SHIFT_JIS-2004)" :coding-type shift-jis :mnemonic 83 :charset-list (ascii katakana-jisx0201 japanese-jisx0213.2004-1 japanese-jisx0213-2)))
    (sjis . (:ascii-compatible-p t :category coding-category-sjis :name japanese-shift-jis :docstring "Shift-JIS 8-bit encoding for Japanese (MIME:SHIFT_JIS)" :coding-type shift-jis :mnemonic 83 :charset-list (ascii katakana-jisx0201 japanese-jisx0208) :mime-charset shift_jis))
    (tcvn . (:ascii-compatible-p nil :category coding-category-charset :name vietnamese-vscii :docstring "8-bit encoding for Vietnamese VSCII-1 (TCVN-5712)." :coding-type charset :mnemonic 118 :charset-list (vscii) :suitable-for-file-name t))
    (tcvn-5712 . (:ascii-compatible-p nil :category coding-category-charset :name vietnamese-vscii :docstring "8-bit encoding for Vietnamese VSCII-1 (TCVN-5712)." :coding-type charset :mnemonic 118 :charset-list (vscii) :suitable-for-file-name t))
    (th-tis620 . (:ascii-compatible-p t :category coding-category-charset :name thai-tis620 :docstring "8-bit encoding for ASCII (MSB=0) and Thai TIS620 (MSB=1)." :coding-type charset :mnemonic 84 :charset-list (tis620-2533)))
    (thai-tis620 . (:ascii-compatible-p t :category coding-category-charset :name thai-tis620 :docstring "8-bit encoding for ASCII (MSB=0) and Thai TIS620 (MSB=1)." :coding-type charset :mnemonic 84 :charset-list (tis620-2533)))
    (tibetan . (:ascii-compatible-p t :category coding-category-iso-8-2 :name tibetan-iso-8bit :docstring "8-bit encoding for ASCII (MSB=0) and TIBETAN (MSB=1)." :coding-type iso-2022 :mnemonic 81 :designation [ascii tibetan nil nil] :charset-list (ascii tibetan)))
    (tibetan-iso-8bit . (:ascii-compatible-p t :category coding-category-iso-8-2 :name tibetan-iso-8bit :docstring "8-bit encoding for ASCII (MSB=0) and TIBETAN (MSB=1)." :coding-type iso-2022 :mnemonic 81 :designation [ascii tibetan nil nil] :charset-list (ascii tibetan)))
    (tis-620 . (:ascii-compatible-p t :category coding-category-charset :name thai-tis620 :docstring "8-bit encoding for ASCII (MSB=0) and Thai TIS620 (MSB=1)." :coding-type charset :mnemonic 84 :charset-list (tis620-2533)))
    (tis620 . (:ascii-compatible-p t :category coding-category-charset :name thai-tis620 :docstring "8-bit encoding for ASCII (MSB=0) and Thai TIS620 (MSB=1)." :coding-type charset :mnemonic 84 :charset-list (tis620-2533)))
    (undecided . (:ascii-compatible-p t :category coding-category-undecided :name undecided :mnemonic 45 :coding-type undecided :ascii-compatible-p t :charset-list (ascii) :for-unibyte nil :docstring "No conversion on encoding, automatic conversion on decoding." :eol-type nil))
    (us-ascii . (:ascii-compatible-p t :category coding-category-charset :name us-ascii :docstring "Encode ASCII as-is and encode non-ASCII characters to `?'." :coding-type charset :mnemonic 45 :charset-list (ascii) :default-char 63 :mime-charset us-ascii))
    (utf-16 . (:ascii-compatible-p nil :category coding-category-utf-16-auto :name utf-16 :docstring "UTF-16 (detect endian on decoding, use big endian on encoding with BOM)." :coding-type utf-16 :mnemonic 85 :charset-list (unicode) :bom (utf-16le-with-signature . utf-16be-with-signature) :endian big :mime-text-unsuitable t :mime-charset utf-16))
    (utf-16-be . (:ascii-compatible-p nil :category coding-category-utf-16-be :name utf-16be-with-signature :docstring "UTF-16 (big endian, with signature (BOM))." :coding-type utf-16 :mnemonic 85 :charset-list (unicode) :bom t :endian big :mime-text-unsuitable t :mime-charset utf-16))
    (utf-16-le . (:ascii-compatible-p nil :category coding-category-utf-16-le :name utf-16le-with-signature :docstring "UTF-16 (little endian, with signature (BOM))." :coding-type utf-16 :mnemonic 85 :charset-list (unicode) :bom t :endian little :mime-text-unsuitable t :mime-charset utf-16))
    (utf-16be . (:ascii-compatible-p nil :category coding-category-utf-16-be-nosig :name utf-16be :docstring "UTF-16BE (big endian, no signature (BOM))." :coding-type utf-16 :mnemonic 85 :charset-list (unicode) :endian big :mime-text-unsuitable t :mime-charset utf-16be))
    (utf-16be-with-signature . (:ascii-compatible-p nil :category coding-category-utf-16-be :name utf-16be-with-signature :docstring "UTF-16 (big endian, with signature (BOM))." :coding-type utf-16 :mnemonic 85 :charset-list (unicode) :bom t :endian big :mime-text-unsuitable t :mime-charset utf-16))
    (utf-16le . (:ascii-compatible-p nil :category coding-category-utf-16-le-nosig :name utf-16le :docstring "UTF-16LE (little endian, no signature (BOM))." :coding-type utf-16 :mnemonic 85 :charset-list (unicode) :endian little :mime-text-unsuitable t :mime-charset utf-16le))
    (utf-16le-with-signature . (:ascii-compatible-p nil :category coding-category-utf-16-le :name utf-16le-with-signature :docstring "UTF-16 (little endian, with signature (BOM))." :coding-type utf-16 :mnemonic 85 :charset-list (unicode) :bom t :endian little :mime-text-unsuitable t :mime-charset utf-16))
    (utf-7 . (:ascii-compatible-p nil :category coding-category-utf-8 :name utf-7 :docstring "UTF-7 encoding of Unicode (RFC 2152)." :coding-type utf-8 :mnemonic 117 :mime-charset utf-7 :charset-list (unicode) :pre-write-conversion utf-7-pre-write-conversion :post-read-conversion utf-7-post-read-conversion))
    (utf-7-imap . (:ascii-compatible-p nil :category coding-category-utf-8 :name utf-7-imap :docstring "UTF-7 encoding of Unicode, IMAP version (RFC 2060)" :coding-type utf-8 :mnemonic 117 :charset-list (unicode) :pre-write-conversion utf-7-imap-pre-write-conversion :post-read-conversion utf-7-imap-post-read-conversion))
    (utf-8 . (:ascii-compatible-p t :category coding-category-utf-8 :name utf-8 :docstring "UTF-8 (no signature (BOM))" :coding-type utf-8 :mnemonic 85 :charset-list (unicode) :mime-charset utf-8))
    (utf-8-auto . (:ascii-compatible-p nil :category coding-category-utf-8-auto :name utf-8-auto :docstring "UTF-8 (auto-detect signature (BOM))" :coding-type utf-8 :mnemonic 85 :charset-list (unicode) :bom (utf-8-with-signature . utf-8)))
    (utf-8-emacs . (:ascii-compatible-p t :category coding-category-utf-8 :name utf-8-emacs :docstring "Support for all Emacs characters (including non-Unicode characters)." :coding-type utf-8 :mnemonic 85 :charset-list (emacs)))
    (utf-8-hfs . (:ascii-compatible-p t :category coding-category-utf-8 :name utf-8-hfs :docstring "UTF-8 based coding system for macOS HFS file names.
    The singleton characters in HFS normalization exclusion will not
    be decomposed." :coding-type utf-8 :mnemonic 85 :charset-list (unicode) :post-read-conversion ucs-normalize-hfs-nfd-post-read-conversion :pre-write-conversion ucs-normalize-hfs-nfd-pre-write-conversion decomposed-characters t))
    (utf-8-nfd . (:ascii-compatible-p t :category coding-category-utf-8 :name utf-8-hfs :docstring "UTF-8 based coding system for macOS HFS file names.
    The singleton characters in HFS normalization exclusion will not
    be decomposed." :coding-type utf-8 :mnemonic 85 :charset-list (unicode) :post-read-conversion ucs-normalize-hfs-nfd-post-read-conversion :pre-write-conversion ucs-normalize-hfs-nfd-pre-write-conversion decomposed-characters t))
    (utf-8-with-signature . (:ascii-compatible-p nil :category coding-category-utf-8-sig :name utf-8-with-signature :docstring "UTF-8 (with signature (BOM))" :coding-type utf-8 :mnemonic 85 :charset-list (unicode) :bom t))
    (vietnamese-tcvn . (:ascii-compatible-p nil :category coding-category-charset :name vietnamese-vscii :docstring "8-bit encoding for Vietnamese VSCII-1 (TCVN-5712)." :coding-type charset :mnemonic 118 :charset-list (vscii) :suitable-for-file-name t))
    (vietnamese-viqr . (:ascii-compatible-p t :category coding-category-utf-8 :name vietnamese-viqr :docstring "Vietnamese latin transcription (VIQR)." :coding-type utf-8 :mnemonic 113 :charset-list (ascii viscii) :post-read-conversion viqr-post-read-conversion :pre-write-conversion viqr-pre-write-conversion))
    (vietnamese-viscii . (:ascii-compatible-p nil :category coding-category-charset :name vietnamese-viscii :docstring "8-bit encoding for Vietnamese VISCII 1.1 (MIME:VISCII)." :coding-type charset :mnemonic 86 :charset-list (viscii) :mime-charset viscii :suitable-for-file-name t))
    (vietnamese-vscii . (:ascii-compatible-p nil :category coding-category-charset :name vietnamese-vscii :docstring "8-bit encoding for Vietnamese VSCII-1 (TCVN-5712)." :coding-type charset :mnemonic 118 :charset-list (vscii) :suitable-for-file-name t))
    (viqr . (:ascii-compatible-p t :category coding-category-utf-8 :name vietnamese-viqr :docstring "Vietnamese latin transcription (VIQR)." :coding-type utf-8 :mnemonic 113 :charset-list (ascii viscii) :post-read-conversion viqr-post-read-conversion :pre-write-conversion viqr-pre-write-conversion))
    (viscii . (:ascii-compatible-p nil :category coding-category-charset :name vietnamese-viscii :docstring "8-bit encoding for Vietnamese VISCII 1.1 (MIME:VISCII)." :coding-type charset :mnemonic 86 :charset-list (viscii) :mime-charset viscii :suitable-for-file-name t))
    (vscii . (:ascii-compatible-p nil :category coding-category-charset :name vietnamese-vscii :docstring "8-bit encoding for Vietnamese VSCII-1 (TCVN-5712)." :coding-type charset :mnemonic 118 :charset-list (vscii) :suitable-for-file-name t))
    (windows-1250 . (:ascii-compatible-p t :category coding-category-charset :name windows-1250 :docstring "windows-1250 (Central European) encoding (MIME: WINDOWS-1250)" :coding-type charset :mnemonic 42 :charset-list (windows-1250) :mime-charset windows-1250))
    (windows-1251 . (:ascii-compatible-p t :category coding-category-charset :name windows-1251 :docstring "windows-1251 8-bit encoding for Cyrillic (MIME: WINDOWS-1251)" :coding-type charset :mnemonic 98 :charset-list (windows-1251) :mime-charset windows-1251))
    (windows-1252 . (:ascii-compatible-p t :category coding-category-charset :name windows-1252 :docstring "windows-1252 (Western European) encoding (MIME: WINDOWS-1252)" :coding-type charset :mnemonic 42 :charset-list (windows-1252) :mime-charset windows-1252))
    (windows-1253 . (:ascii-compatible-p t :category coding-category-charset :name windows-1253 :docstring "windows-1253 encoding for Greek" :coding-type charset :mnemonic 103 :charset-list (windows-1253) :mime-charset windows-1253))
    (windows-1254 . (:ascii-compatible-p t :category coding-category-charset :name windows-1254 :docstring "windows-1254 (Turkish) encoding (MIME: WINDOWS-1254)" :coding-type charset :mnemonic 42 :charset-list (windows-1254) :mime-charset windows-1254))
    (windows-1255 . (:ascii-compatible-p t :category coding-category-charset :name windows-1255 :docstring "windows-1255 (Hebrew) encoding (MIME: WINDOWS-1255)" :coding-type charset :mnemonic 104 :charset-list (windows-1255) :mime-charset windows-1255))
    (windows-1256 . (:ascii-compatible-p t :category coding-category-charset :name windows-1256 :docstring "windows-1256 (Arabic) encoding (MIME: WINDOWS-1256)" :coding-type charset :mnemonic 65 :charset-list (windows-1256) :mime-charset windows-1256))
    (windows-1257 . (:ascii-compatible-p t :category coding-category-charset :name windows-1257 :docstring "windows-1257 (Baltic) encoding (MIME: WINDOWS-1257)" :coding-type charset :mnemonic 42 :charset-list (windows-1257) :mime-charset windows-1257))
    (windows-1258 . (:ascii-compatible-p t :category coding-category-charset :name windows-1258 :docstring "windows-1258 encoding for Vietnamese (MIME: WINDOWS-1258)" :coding-type charset :mnemonic 42 :charset-list (windows-1258) :mime-charset windows-1258))
    (windows-936 . (:ascii-compatible-p t :category coding-category-charset :name chinese-gbk :docstring "GBK encoding for Chinese (MIME:GBK)." :coding-type charset :mnemonic 99 :charset-list (ascii chinese-gbk) :mime-charset gbk))
    (x-ctext . (:ascii-compatible-p nil :category coding-category-iso-8-else :name compound-text :docstring "Compound text based generic encoding.
    This coding system is an extension of X's \"Compound Text Encoding\".
    It encodes many characters using the normal ISO-2022 designation sequences,
    but it doesn't support extended segments of CTEXT." :coding-type iso-2022 :mnemonic 120 :charset-list iso-2022 :designation [(ascii 94) (latin-iso8859-1 katakana-jisx0201 96) nil nil] :flags (ascii-at-eol ascii-at-cntl long-form designation locking-shift single-shift composition) :mime-charset x-ctext))
    (x-ctext-with-extensions . (:ascii-compatible-p nil :category coding-category-iso-8-else :name compound-text-with-extensions :docstring "Compound text encoding with ICCCM Extended Segment extensions.
    
    See the variables `ctext-standard-encodings' and
    `ctext-non-standard-encodings-alist' for the detail about how
    extended segments are handled.
    
    This coding system should be used only for X selections.  It is inappropriate
    for decoding and encoding files, process I/O, etc." :coding-type iso-2022 :mnemonic 120 :charset-list iso-2022 :designation [(ascii 94) (latin-iso8859-1 katakana-jisx0201 96) nil nil] :flags (ascii-at-eol ascii-at-cntl long-form designation locking-shift single-shift) :post-read-conversion ctext-post-read-conversion :pre-write-conversion ctext-pre-write-conversion :mime-charset x-ctext))
)
  "GNU 31 `coding-system-plist' data for every defined coding-system name.")

;; ---------- buffer macros ----------

(defmacro with-temp-buffer (&rest body)
  "Create a temporary buffer, and evaluate BODY there like `progn'."
  (let ((temp-buffer (make-symbol "temp-buffer")))
    (list 'let (list (list temp-buffer '(generate-new-buffer " *temp*")))
          (list 'with-current-buffer temp-buffer
                (list 'unwind-protect
                      (cons 'progn body)
                      (list 'and
                            (list 'buffer-name temp-buffer)
                            (list 'kill-buffer temp-buffer)))))))

(defmacro with-syntax-table (table &rest body)
  "Evaluate BODY with syntax table of current buffer set to TABLE.
The syntax table of the current buffer is saved, BODY is evaluated, and
the saved table is restored, even in case of an abnormal exit.
Value is what BODY returns."
  (let ((tbl (make-symbol "table"))
        (buf (make-symbol "buffer")))
    (list 'let (list (list tbl '(syntax-table))
                     (list buf '(current-buffer)))
          (list 'unwind-protect
                (cons 'progn (cons (list 'set-syntax-table table) body))
                (list 'save-current-buffer
                      (list 'set-buffer buf)
                      (list 'set-syntax-table tbl))))))

;; GNU case-table.el.
(defun char-uppercase-p (char)
  "Return non-nil if CHAR is an uppercase character."
  (and (characterp char)
       (not (eq (downcase char) char))
       (eq (upcase char) char)))

;; `copy-case-table' is a subr in GNU 31 (the case-table.el defun is
;; dead code there — its `dotimes (i 4)' overruns the 3 extra slots).

(defmacro with-temp-file (file &rest body)
  "Create a temporary buffer, evaluate BODY, write it to FILE."
  (let ((temp-buffer (make-symbol "temp-buffer"))
        (temp-file (make-symbol "temp-file")))
    (list 'let (list (list temp-file file)
                     (list temp-buffer '(generate-new-buffer " *temp file*")))
          (list 'with-current-buffer temp-buffer
                (list 'unwind-protect
                      (list 'prog1 (cons 'progn body)
                            (list 'write-region nil nil temp-file nil 0))
                      (list 'and
                            (list 'buffer-name temp-buffer)
                            (list 'kill-buffer temp-buffer)))))))

;; `save-current-buffer' is a primitive special form (as in GNU), not a
;; macro — see special.rs.

;; `push'/`pop' are GNU subr.el defmacros, verbatim (the symbol fast-path
;; avoids triggering GV; other places expand through `gv-letplace').
(defmacro push (newelt place)
  "Add NEWELT to the list stored in the generalized variable PLACE.

This is morally equivalent to (setf PLACE (cons NEWELT PLACE)),
except that PLACE is evaluated only once (after NEWELT).

For more information about generalized variables, see Info node
`(elisp) Generalized Variables'."
  (declare (debug (form gv-place)))
  (if (symbolp place)
      ;; Important special case, to avoid triggering GV too early in
      ;; the bootstrap.
      (list 'setq place
            (list 'cons newelt place))
    (require 'macroexp)
    (macroexp-let2 macroexp-copyable-p x newelt
      (gv-letplace (getter setter) place
        (funcall setter `(cons ,x ,getter))))))

(defmacro pop (place)
  "Return the first element of PLACE's value, and remove it from the list.

PLACE must be a generalized variable whose value is a list.
If the value is nil, `pop' returns nil but does not actually
change the list.

For more information about generalized variables, see Info node
`(elisp) Generalized Variables'."
  (declare (debug (gv-place)))
  ;; We use `car-safe' here instead of `car' because the behavior is the same
  ;; (if it's not a cons cell, the `cdr' would have signaled an error already),
  ;; but `car-safe' is total, so the byte-compiler can safely remove it if the
  ;; result is not used.
  `(car-safe
    ,(if (symbolp place)
         ;; So we can use `pop' in the bootstrap before `gv' can be used.
         (list 'prog1 place (list 'setq place (list 'cdr place)))
       (gv-letplace (getter setter) place
         (macroexp-let2 macroexp-copyable-p x getter
           `(prog1 ,x ,(funcall setter `(cdr ,x))))))))

(defmacro setq-local (var val)
  "Make variable VAR buffer-local and set it to VAL."
  (list 'set (list 'make-local-variable (list 'quote var)) val))

;; ---------- small functions ----------

(defun number-sequence (from &optional to inc)
  "Return list of numbers FROM to TO by INC.
If TO is nil or equal to FROM, return (FROM)."
  (if (or (not to) (= from to))
      (list from)
    (or inc (setq inc 1))
    (let ((res nil) (n from))
      (if (> inc 0)
          (while (<= n to)
            (setq res (cons n res))
            (setq n (+ n inc)))
        (while (>= n to)
          (setq res (cons n res))
          (setq n (+ n inc))))
      (nreverse res))))

(defmacro with-output-to-string (&rest body)
  "Execute BODY with `standard-output' bound to a temporary buffer,
returning that buffer's contents as a string."
  (let ((buf (make-symbol "out-buffer")))
    (list 'let (list (list buf '(generate-new-buffer " *string-output*")))
          (list 'unwind-protect
                (list 'let (list (list 'standard-output buf))
                      (cons 'progn body)
                      (list 'with-current-buffer buf '(buffer-string)))
                (list 'and
                      (list 'buffer-name buf)
                      (list 'kill-buffer buf))))))

;; ---------- string predicates ----------
;; GNU: string-equal/string-lessp/string-greaterp are primitives that accept
;; strings or symbols; the =/</> spellings are Lisp-level aliases.
;; string-empty-p et al. are Lisp defuns in subr.el.

(defalias 'string= 'string-equal)
(defalias 'string< 'string-lessp)
(defalias 'string> 'string-greaterp)

(defun string-greaterp (string1 string2)
  "Return non-nil if STRING1 is greater than STRING2 in lexicographic order.
Symbols are also allowed; their print names are used instead."
  (string-lessp string2 string1))

(defun string-empty-p (string)
  "Check whether STRING is empty."
  (string= string ""))

(defun string-blank-p (string)
  "Check whether STRING is either empty or only whitespace."
  (string-match-p "\\`[ \11\n\15]*\\'" string))

(defun string-prefix-p (prefix string &optional ignore-case)
  "Return non-nil if STRING begins with PREFIX."
  (let ((prefix-length (length prefix)))
    (if (> prefix-length (length string)) nil
      (eq t (compare-strings prefix 0 prefix-length string
                             0 prefix-length ignore-case)))))

(defun string-suffix-p (suffix string &optional ignore-case)
  "Return non-nil if STRING ends with SUFFIX."
  (let ((start-pos (- (length string) (length suffix))))
    (and (>= start-pos 0)
         (eq t (compare-strings suffix nil nil
                                string start-pos nil ignore-case)))))

;; ---------- cl-lib list accessors ----------
;; These are plain Lisp defaliases in cl-lib.el / cl-macs.el.

(defalias 'cl-first 'car)
(defalias 'cl-second 'cadr)
(defalias 'cl-third 'caddr)
(defalias 'cl-fourth 'cadddr)
(defun cl-fifth (x) (nth 4 x))
(defun cl-sixth (x) (nth 5 x))
(defun cl-seventh (x) (nth 6 x))
(defun cl-eighth (x) (nth 7 x))
(defun cl-ninth (x) (nth 8 x))
(defun cl-tenth (x) (nth 9 x))
(defalias 'cl-rest 'cdr)
(defalias 'cl-endp 'null)

(defun cl-list-length (x)
  "Return the length of list X, or nil for a circular/dotted list."
  (let ((n 0) (tail x))
    (while (consp tail)
      (setq tail (cdr tail))
      (setq n (1+ n)))
    (and (null tail) n)))

;; ---------- more subr.el-style helpers ----------

(defmacro with-silent-modifications (&rest body)
  "Execute BODY, suppressing modification hooks (simplified)."
  (list 'let '((inhibit-modification-hooks t)
              (inhibit-read-only t)
              deactivate-mark
              buffer-file-format
              buffer-file-coding-system)
        (cons 'progn body)))

(defun member-ignore-case (elt list)
  "Like `member', but ignore string case differences."
  (let ((tail list))
    (while (and tail
                (not (if (and (stringp (car tail)) (stringp elt))
                         (eq t (compare-strings (car tail) 0 nil elt 0 nil t))
                       (equal (car tail) elt))))
      (setq tail (cdr tail)))
    tail))

(defun delete-dups (list)
  "Destructively remove duplicate `equal' elements from LIST."
  (let ((tail list))
    (while tail
      (setcdr tail (delete (car tail) (cdr tail)))
      (setq tail (cdr tail))))
  list)

;; ---------- simple.el-style interactive commands ----------

;; ---------- vertical motion (simple.el) ----------

(defvar hard-newline (propertize "\n" 'hard t 'rear-nonsticky '(hard))
  "Propertized newline representing a \"hard\" newline.")

(defvar next-line-add-newlines nil
  "Non-nil means `next-line' inserts newlines when called at the end of a buffer."
  :type 'boolean
  :group 'editing-basics)

(defvar track-eol nil
  "Non-nil means vertical motion starting at end of line keeps to ends of lines.
This means moving to the end of each line moved onto.
The beginning of a blank line does not count as the end of a line.
This has no effect when the variable `line-move-visual' is non-nil."
  :type 'boolean
  :group 'editing-basics)

(defvar goal-column nil
  "Semipermanent goal column for vertical motion, as set by \\[set-goal-column], or nil.
A non-nil setting overrides the variable `line-move-visual', which see."
  :type '(choice integer
		 (const :tag "None" nil))
  :local t
  :group 'editing-basics)

(defvar temporary-goal-column 0
  "Current goal column for vertical motion.
It is the column where point was at the start of the current run
of vertical motion commands.

When moving by visual lines via the function `line-move-visual', it is a cons
cell (COL . HSCROLL), where COL is the x-position, in pixels,
divided by the default column width, and HSCROLL is the number of
columns by which window is scrolled from left margin.

When the `track-eol' feature is doing its job, the value is
`most-positive-fixnum'.")

(defvar line-move-ignore-invisible t
  "Non-nil means commands that move by lines ignore invisible newlines.
When this option is non-nil, \\[next-line], \\[previous-line], \\[move-end-of-line], and \\[move-beginning-of-line] behave
as if newlines that are invisible didn't exist, and count
only visible newlines.  Thus, moving across 2 newlines
of which one is invisible will move point to the second visible newline.
\(This is the default, because it is behavior users expect.)
Note that this option does not affect motion commands that are invoked
via `line-move', like \\[move-end-of-line] and \\[move-beginning-of-line]."
  :type 'boolean
  :group 'editing-basics)

(defvar line-move-visual t
  "When non-nil, `line-move' moves point by visual lines.
This movement is based on where the cursor is displayed on the
screen, instead of relying on buffer contents alone.  It takes
into account variable-width characters and line continuation.
If nil, `line-move' moves point by logical lines.
A non-nil setting of `goal-column' overrides the value of this variable
and forces movement by logical lines.
A window that is horizontally scrolled also forces movement by logical
lines."
  :type 'boolean
  :group 'editing-basics)

(defvar auto-window-vscroll t
  "Non-nil means to automatically adjust `window-vscroll' to view tall lines."
  :type 'boolean
  :group 'windows)

(defvar overflow-newline-into-fringe nil
  "Non-nil means to display the newline on right fringe in a truncated line.")

(defun line-move-to-column (col)
  "Try to find column COL, considering invisibility.
This function works only in certain cases,
because what we really need is for `move-to-column'
and `current-column' to be able to ignore invisible text."
  (if (zerop col)
      (beginning-of-line)
    (move-to-column col))

  (when (and line-move-ignore-invisible
	     (not (bolp)) (invisible-p (1- (point))))
    (let ((normal-location (point))
	  (normal-column (current-column)))
      ;; If the following character is currently invisible,
      ;; skip all characters with that same `invisible' property value.
      (while (and (not (eobp))
		  (invisible-p (point)))
	(goto-char (next-char-property-change (point))))
      ;; Have we advanced to a larger column position?
      (if (> (current-column) normal-column)
	  ;; We have made some progress towards the desired column.
	  ;; See if we can make any further progress.
	  (line-move-to-column (+ (current-column) (- col normal-column)))
	;; Otherwise, go to the place we originally found
	;; and move back over invisible text.
	;; that will get us to the same place on the screen
	;; but with a more reasonable buffer position.
	(goto-char normal-location)
	(let ((line-beg
               ;; We want the real line beginning, so it's consistent
               ;; with bolp below, otherwise we might infloop.
               (let ((inhibit-field-text-motion t))
                 (line-beginning-position))))
	  (while (and (not (bolp)) (invisible-p (1- (point))))
	    (goto-char (previous-char-property-change (point) line-beg))))))))

(defun line-move-finish (column opoint forward &optional not-ipmh)
  (let ((repeat t))
    (while repeat
      ;; Set REPEAT to t to repeat the whole thing.
      (setq repeat nil)

      (let (new
	    (old (point))
	    (line-beg (line-beginning-position))
	    (line-end
	     ;; Compute the end of the line
	     ;; ignoring effectively invisible newlines.
	     (save-excursion
	       ;; Like end-of-line but ignores fields.
	       (skip-chars-forward "^\n")
	       (while (and (not (eobp)) (invisible-p (point)))
		 (goto-char (next-char-property-change (point)))
		 (skip-chars-forward "^\n"))
	       (point))))

	;; Move to the desired column.  GNU takes a pixel-accurate
	;; `vertical-motion' path under `line-move-visual'; on our
	;; fixed-pitch display both paths reduce to `move-to-column'.
        (line-move-to-column (truncate column))

	;; Corner case: suppose we start out in a field boundary in
	;; the middle of a continued line.  When we get to
	;; line-move-finish, point is at the start of a new *screen*
	;; line but the same text line; then line-move-to-column would
	;; move us backwards.  Test using C-n with point on the "x" in
	;;   (insert "a" (propertize "x" 'field t) (make-string 89 ?y))
	(and forward
	     (< (point) old)
	     (goto-char old))

	(setq new (point))

	;; Process intangibility within a line.
	;; With inhibit-point-motion-hooks bound to nil, a call to
	;; goto-char moves point past intangible text.

	;; However, inhibit-point-motion-hooks controls both the
	;; intangibility and the point-entered/point-left hooks.  The
	;; following hack avoids calling the point-* hooks
	;; unnecessarily.  Note that we move *forward* past intangible
	;; text when the initial and final points are the same.
	(goto-char new)
	(with-suppressed-warnings ((obsolete inhibit-point-motion-hooks))
	  (let ((inhibit-point-motion-hooks (not not-ipmh)))
	    (goto-char new)

	    ;; If intangibility moves us to a different (later) place
	    ;; in the same line, use that as the destination.
	    (if (<= (point) line-end)
	        (setq new (point))
	      ;; If that position is "too late",
	      ;; try the previous allowable position.
	      ;; See if it is ok.
	      (backward-char)
	      (if (if forward
		      ;; If going forward, don't accept the previous
		      ;; allowable position if it is before the target line.
		      (< line-beg (point))
		    ;; If going backward, don't accept the previous
		    ;; allowable position if it is still after the target line.
		    (<= (point) line-end))
		  (setq new (point))
		;; As a last resort, use the end of the line.
		(setq new line-end)))))

	;; Now move to the updated destination, processing fields
	;; as well as intangibility.
	(goto-char opoint)
	(with-suppressed-warnings ((obsolete inhibit-point-motion-hooks))
	  (let ((inhibit-point-motion-hooks (not not-ipmh)))
	    (goto-char
	     ;; Ignore field boundaries if the initial and final
	     ;; positions have the same `field' property, even if the
	     ;; fields are non-contiguous.  This seems to be "nicer"
	     ;; behavior in many situations.
	     (if (eq (get-char-property new 'field)
		     (get-char-property opoint 'field))
		 new
	       (constrain-to-field new opoint t t
				   'inhibit-line-move-field-capture)))))

	;; If all this moved us to a different line,
	;; retry everything within that new line.
	(when (or (< (point) line-beg) (> (point) line-end))
	  ;; Repeat the intangibility and field processing.
	  (setq repeat t))))))

;; This is the guts of next-line and previous-line.
;; Arg says how many lines to move.
;; The value is t if we can move the specified number of lines.
(defun line-move-1 (arg &optional noerror _to-end)
  ;; Don't run any point-motion hooks, and disregard intangibility,
  ;; for intermediate positions.
  (with-suppressed-warnings ((obsolete inhibit-point-motion-hooks))
  (let ((outer-ipmh inhibit-point-motion-hooks)
	(inhibit-point-motion-hooks t)
	(opoint (point))
	(orig-arg arg))
    (if (consp temporary-goal-column)
	(setq temporary-goal-column (+ (car temporary-goal-column)
				       (cdr temporary-goal-column))))
    (unwind-protect
	(progn
	  (if (not (memq last-command '(next-line previous-line)))
	      (setq temporary-goal-column
		    (if (and track-eol (eolp)
			     ;; Don't count beg of empty line as end of line
			     ;; unless we just did explicit end-of-line.
			     (or (not (bolp)) (eq last-command 'move-end-of-line)))
			most-positive-fixnum
		      (current-column))))

	  (if (not (or (integerp selective-display)
                       line-move-ignore-invisible))
	      ;; Use just newline characters.
	      ;; Set ARG to 0 if we move as many lines as requested.
	      (or (if (> arg 0)
		      (progn (if (> arg 1) (forward-line (1- arg)))
			     ;; This way of moving forward ARG lines
			     ;; verifies that we have a newline after the last one.
			     ;; It doesn't get confused by intangible text.
			     (end-of-line)
			     (if (zerop (forward-line 1))
				 (setq arg 0)))
		    (and (zerop (forward-line arg))
			 (bolp)
			 (setq arg 0)))
		  (unless noerror
		    (signal (if (< arg 0)
				'beginning-of-buffer
			      'end-of-buffer)
			    nil)))
	    ;; Move by arg lines, but ignore invisible ones.
	    (let (done)
	      (while (and (> arg 0) (not done))
		;; If the following character is currently invisible,
		;; skip all characters with that same `invisible' property value.
		(while (and (not (eobp)) (invisible-p (point)))
		  (goto-char (next-char-property-change (point))))
		;; Move a line.
		;; We don't use `end-of-line', since we want to escape
		;; from field boundaries occurring exactly at point.
		(goto-char (constrain-to-field
			    (let ((inhibit-field-text-motion t))
			      (line-end-position))
			    (point) t t
			    'inhibit-line-move-field-capture))
		;; If there's no invisibility here, move over the newline.
		(cond
		 ((eobp)
		  (if (not noerror)
		      (signal 'end-of-buffer nil)
		    (setq done t)))
		 ((and (> arg 1)  ;; Use vertical-motion for last move
		       (not (integerp selective-display))
		       (not (invisible-p (point))))
		  ;; We avoid vertical-motion when possible
		  ;; because that has to fontify.
		  (forward-line 1))
		 ;; Otherwise move a more sophisticated way.
		 ((zerop (vertical-motion 1))
		  (if (not noerror)
		      (signal 'end-of-buffer nil)
		    (setq done t))))
		(unless done
		  (setq arg (1- arg))))
	      ;; The logic of this is the same as the loop above,
	      ;; it just goes in the other direction.
	      (while (and (< arg 0) (not done))
		;; For completely consistency with the forward-motion
		;; case, we should call beginning-of-line here.
		;; However, if point is inside a field and on a
		;; continued line, the call to (vertical-motion -1)
		;; below won't move us back far enough; then we return
		;; to the same column in line-move-finish, and point
		;; gets stuck -- cyd
		(forward-line 0)
		(cond
		 ((bobp)
		  (if (not noerror)
		      (signal 'beginning-of-buffer nil)
		    (setq done t)))
		 ((and (< arg -1) ;; Use vertical-motion for last move
		       (not (integerp selective-display))
		       (not (invisible-p (1- (point)))))
		  (forward-line -1))
		 ((zerop (vertical-motion -1))
		  (if (not noerror)
		      (signal 'beginning-of-buffer nil)
		    (setq done t))))
		(unless done
		  (setq arg (1+ arg))
		  (while (and ;; Don't move over previous invis lines
			  ;; if our target is the middle of this line.
			  (or (zerop (or goal-column temporary-goal-column))
			      (< arg 0))
			  (not (bobp)) (invisible-p (1- (point))))
		    (goto-char (previous-char-property-change (point))))))))
	  ;; This is the value the function returns.
	  (= arg 0))

      (cond ((> arg 0)
	     ;; If we did not move down as far as desired, at least go
	     ;; to end of line.  Be sure to call point-entered and
	     ;; point-left-hooks.
	     (let* ((npoint (prog1 (line-end-position)
			      (goto-char opoint)))
		    (inhibit-point-motion-hooks outer-ipmh))
	       (goto-char npoint)))
	    ((< arg 0)
	     ;; If we did not move up as far as desired,
	     ;; at least go to beginning of line.
	     (let* ((npoint (prog1 (line-beginning-position)
			      (goto-char opoint)))
		    (inhibit-point-motion-hooks outer-ipmh))
	       (goto-char npoint)))
	    (t
	     (line-move-finish (or goal-column temporary-goal-column)
			       opoint (> orig-arg 0) (not outer-ipmh))))))))

;; Display-based alternative to line-move-1.
;; Arg says how many lines to move.  The value is t if we can move the
;; specified number of lines.
;; FIXME: our display is fixed-pitch and does not wrap lines, so visual
;; and buffer-line motion coincide; delegate to the buffer-line engine.
(defun line-move-visual (arg &optional noerror)
  "Move ARG lines forward.
If NOERROR, don't signal an error if we can't move that many lines."
  (line-move-1 arg noerror))

(defun line-move-partial (arg noerror &optional _to-end)
  "Fallback for GNU's pixel-scrolling partial line move."
  (line-move-1 arg noerror))

(defun line-move (arg &optional noerror _to-end try-vscroll)
  "Move forward ARG lines.
If NOERROR, don't signal an error if we can't move ARG lines.
TO-END is unused.
TRY-VSCROLL controls whether to vscroll tall lines: if either
`auto-window-vscroll' or TRY-VSCROLL is nil, this function will
not vscroll."
  (if noninteractive
      (line-move-1 arg noerror)
    (unless (and auto-window-vscroll try-vscroll
		 ;; Only vscroll for single line moves
		 (= (abs arg) 1)
		 ;; Under scroll-conservatively, the display engine
		 ;; does this better.
		 (zerop scroll-conservatively)
		 ;; But don't vscroll in a keyboard macro.
		 (not defining-kbd-macro)
		 (not executing-kbd-macro)
                 ;; Lines are not truncated...
                 (not
                  (and
                   (or truncate-lines (truncated-partial-width-window-p))
                   ;; ...or if lines are truncated, this buffer
                   ;; doesn't have very long lines.
                   (long-line-optimizations-p)))
		 (line-move-partial arg noerror))
      (set-window-vscroll nil 0 t)
      (if (and line-move-visual
	       ;; Display-based column are incompatible with goal-column.
	       (not goal-column)
               ;; Lines aren't truncated.
               (not
                (and
                 (or truncate-lines (truncated-partial-width-window-p))
                 (long-line-optimizations-p)))
	       ;; When the text in the window is scrolled to the left,
	       ;; display-based motion doesn't make sense (because each
	       ;; logical line occupies exactly one screen line).
	       (not (> (window-hscroll) 0))
	       ;; Likewise when the text _was_ scrolled to the left
	       ;; when the current run of vertical motion commands
	       ;; started.
	       (not (and (memq last-command
			       `(next-line previous-line ,this-command))
			 auto-hscroll-mode
			 (numberp temporary-goal-column)
			 (>= temporary-goal-column
			    (- (window-width) hscroll-margin)))))
	  (prog1 (line-move-visual arg noerror)
	    ;; If we moved into a tall line, set vscroll to make
	    ;; scrolling through tall images more smooth.
	    (let ((lh (line-pixel-height))
		  (edges (window-inside-pixel-edges))
		  (dlh (line-pixel-height))
		  winh)
	      (setq winh (- (nth 3 edges) (nth 1 edges) 1))
	      (if (and (< arg 0)
		       (< (point) (window-start))
		       (> lh winh))
		  (set-window-vscroll
		   nil
		   (- lh dlh) t))))
	(line-move-1 arg noerror)))))

(defun next-line (&optional arg try-vscroll)
  "Move cursor vertically down ARG lines.
Interactively, vscroll tall lines if `auto-window-vscroll' is enabled.
Non-interactively, use TRY-VSCROLL to control whether to vscroll tall
lines: if either `auto-window-vscroll' or TRY-VSCROLL is nil, this
function will not vscroll.

ARG defaults to 1.

If there is no character in the target line exactly under the current column,
the cursor is positioned after the character in that line that spans this
column, or at the end of the line if it is not long enough.
If there is no line in the buffer after this one, behavior depends on the
value of `next-line-add-newlines'.  If non-nil, it inserts a newline character
to create a line, and moves the cursor to that line.  Otherwise it moves the
cursor to the end of the buffer.

If the variable `line-move-visual' is non-nil, this command moves
by display lines.  Otherwise, it moves by buffer lines, without
taking variable-width characters or continued lines into account.
See \\[next-logical-line] for a command that always moves by buffer lines.

The command \\[set-goal-column] can be used to create
a semipermanent goal column for this command.
Then instead of trying to move exactly vertically (or as close as possible),
this command moves to the specified goal column (or as close as possible).
The goal column is stored in the variable `goal-column', which is nil
when there is no goal column.  Note that setting `goal-column'
overrides `line-move-visual' and causes this command to move by buffer
lines rather than by display lines."
  (declare (interactive-only forward-line))
  (interactive "^p\np")
  (or arg (setq arg 1))
  (if (and next-line-add-newlines (= arg 1)
	   (save-excursion (end-of-line) (eobp)))
      ;; When adding a newline, don't expand an abbrev.
      (let ((abbrev-mode nil))
	(end-of-line)
	(insert (if use-hard-newlines hard-newline "\n")))
    (line-move arg nil nil try-vscroll))
  nil)

(defun previous-line (&optional arg try-vscroll)
  "Move cursor vertically up ARG lines.
Interactively, vscroll tall lines if `auto-window-vscroll' is enabled.
Non-interactively, use TRY-VSCROLL to control whether to vscroll tall
lines: if either `auto-window-vscroll' or TRY-VSCROLL is nil, this
function will not vscroll.

ARG defaults to 1.

If there is no character in the target line exactly over the current column,
the cursor is positioned after the character in that line that spans this
column, or at the end of the line if it is not long enough.

If the variable `line-move-visual' is non-nil, this command moves
by display lines.  Otherwise, it moves by buffer lines, without
taking variable-width characters or continued lines into account.
See \\[previous-logical-line] for a command that always moves by buffer lines.

The command \\[set-goal-column] can be used to create
a semipermanent goal column for this command.
Then instead of trying to move exactly vertically (or as close as possible),
this command moves to the specified goal column (or as close as possible).
The goal column is stored in the variable `goal-column', which is nil
when there is no goal column.  Note that setting `goal-column'
overrides `line-move-visual' and causes this command to move by buffer
lines rather than by display lines."
  (declare (interactive-only
            "use `forward-line' with negative argument instead."))
  (interactive "^p\np")
  (or arg (setq arg 1))
  (line-move (- arg) nil nil try-vscroll)
  nil)

(defun next-logical-line (&optional arg try-vscroll)
  "Move cursor vertically down ARG lines.
This is identical to `next-line', except that it always moves
by logical lines instead of visual lines, ignoring the value of
the variable `line-move-visual'."
  (interactive "^p\np")
  (let ((line-move-visual nil))
    (with-no-warnings
      (next-line arg try-vscroll))))

(defun previous-logical-line (&optional arg try-vscroll)
  "Move cursor vertically up ARG lines.
This is identical to `previous-line', except that it always moves
by logical lines instead of visual lines, ignoring the value of
the variable `line-move-visual'."
  (interactive "^p\np")
  (let ((line-move-visual nil))
    (with-no-warnings
      (previous-line arg try-vscroll))))

(defun set-goal-column (arg)
  "Set the current horizontal position as a goal column.
This goal column will affect the \\[next-line] and \\[previous-line] commands,
as well as the \\[scroll-up-command] and \\[scroll-down-command] commands.

Those commands will move to this position in the line moved to
rather than trying to keep the same horizontal position.

With a non-nil argument ARG, clears out the goal column so that
these commands resume normal motion.

The goal column is stored in the variable `goal-column'.  This is
a buffer-local setting."
  (interactive "P")
  (if arg
      (progn
        (setq goal-column nil)
        (message "No goal column"))
    (setq goal-column (current-column))
    (message "Goal column %d %s"
             goal-column
	     (substitute-command-keys
	      "(use \\[set-goal-column] with an arg to unset it)")))
  nil)

(defun beginning-of-buffer (&optional arg)
  "Move point to the beginning of the buffer."
  (interactive "^P")
  (goto-char (point-min)))

(defun end-of-buffer (&optional arg)
  "Move point to the end of the buffer."
  (interactive "^P")
  (goto-char (point-max)))

(defun mark-whole-buffer ()
  "Put point at beginning and mark at end of buffer."
  (interactive)
  (push-mark (point))
  (push-mark (point-max) nil t)
  (goto-char (point-min)))

(defun set-mark-command (arg)
  "Set the mark at point, or jump to the mark with a prefix argument."
  (interactive "P")
  (if arg
      (when (mark)
        (goto-char (mark))
        (deactivate-mark))
    (push-mark)))

(defun keyboard-quit ()
  "Signal a `quit' condition (C-g)."
  (interactive)
  (signal 'quit nil))

(defun keyboard-escape-quit ()
  "Abort the current operation (ESC ESC ESC)."
  (interactive)
  (signal 'quit nil))

(defun mark-word (arg)
  "Set mark ARG words from point."
  (interactive "P")
  (set-mark (point))
  (forward-word (prefix-numeric-value arg)))

(defun mark-sexp (arg)
  "Set mark ARG sexps from point."
  (interactive "P")
  (set-mark (point))
  (forward-sexp (prefix-numeric-value arg)))

(defun mark-paragraph (&optional arg)
  "Put mark at end of this paragraph, point at beginning."
  (interactive "P")
  (forward-paragraph (or arg 1))
  (push-mark nil t t)
  (backward-paragraph))

(defun back-to-indentation ()
  "Move point to the first non-whitespace character on this line."
  (interactive "^")
  (beginning-of-line)
  (skip-chars-forward " \t"))

(defun goto-line (line)
  "Go to LINE, counting from line 1 at beginning of buffer."
  (interactive "NGoto line: ")
  (goto-char (point-min))
  (forward-line (1- line)))

(defun recenter-top-bottom (&optional arg)
  "Center point in window; with ARG, cycle positions."
  (interactive "P")
  (recenter arg))

(defun quoted-insert (arg)
  "Read next input character and insert it ARG times."
  (interactive "*p")
  (insert-char (read-char) arg))

(defun indent-for-tab-command (&optional arg)
  "Indent the current line (inserts a tab in fundamental mode)."
  (interactive "P")
  (insert "\t"))

(defun indent-relative (&optional unindented-ok)
  "Space out to under next indent point in previous nonblank line."
  (interactive "P")
  (insert "\t"))

(defun newline-and-indent ()
  "Insert a newline, then indent."
  (interactive "*")
  (newline)
  (insert "\t"))

(defun transpose-words (arg)
  "Interchange the word at point with the previous word ARG times."
  (interactive "*p")
  (transpose-subr 'forward-word arg))

(defun transpose-sexps-default-function (arg)
  "Default method to locate a pair of points for `transpose-sexps'."
  (if (if (> arg 0)
          (looking-at "\\sw\\|\\s_")
        (and (not (bobp))
             (save-excursion
               (forward-char -1)
               (looking-at "\\sw\\|\\s_"))))
      ;; Jumping over a symbol.  We might be inside it, mind you.
      (progn (funcall (if (> arg 0)
                          #'skip-syntax-backward #'skip-syntax-forward)
                      "w_")
             (cons (save-excursion (forward-sexp arg) (point)) (point)))
    ;; Otherwise, we're between sexps.  Take a step back before jumping
    ;; to make sure we'll obey the same precedence no matter which
    ;; direction we're going.
    (funcall (if (> arg 0) #'skip-syntax-backward #'skip-syntax-forward)
             " .")
    (cons (save-excursion (forward-sexp arg) (point))
          (progn (while (or (forward-comment (if (> arg 0) 1 -1))
                            (not (zerop (funcall (if (> arg 0)
                                                     #'skip-syntax-forward
                                                   #'skip-syntax-backward)
                                                 ".")))))
                 (point)))))

(defun transpose-sexps (arg)
  "Like \\[transpose-chars] (`transpose-chars'), but applies to sexps."
  (interactive "*p")
  (transpose-subr 'transpose-sexps-default-function arg 'special))

(defun transpose-subr-1 (pos1 pos2)
  (unless (and pos1 pos2)
    (error "Don't have two things to transpose"))
  (when (> (car pos1) (cdr pos1)) (setq pos1 (cons (cdr pos1) (car pos1))))
  (when (> (car pos2) (cdr pos2)) (setq pos2 (cons (cdr pos2) (car pos2))))
  (when (> (car pos1) (car pos2))
    (let ((swap pos1))
      (setq pos1 pos2 pos2 swap)))
  (if (> (cdr pos1) (car pos2)) (error "Don't have two things to transpose"))
  (let* ((a (buffer-substring (car pos1) (cdr pos1)))
         (m (buffer-substring (cdr pos1) (car pos2)))
         (b (buffer-substring (car pos2) (cdr pos2))))
    (delete-region (car pos1) (cdr pos2))
    (goto-char (car pos1))
    (insert b m a)))

(defun transpose-subr (mover arg &optional special)
  "Subroutine to do the work of transposing objects."
  (let ((aux (if special mover
               (lambda (x)
                 (cons (progn (funcall mover x) (point))
                       (progn (funcall mover (- x)) (point))))))
        pos1 pos2)
    (cond
     ((= arg 0)
      (save-excursion
        (setq pos1 (funcall aux 1))
        (goto-char (or (mark) (error "No mark set in this buffer")))
        (setq pos2 (funcall aux 1))
        (transpose-subr-1 pos1 pos2))
      (exchange-point-and-mark))
     ((> arg 0)
      (setq pos1 (funcall aux -1))
      (setq pos2 (funcall aux arg))
      (transpose-subr-1 pos1 pos2)
      (goto-char (car pos2)))
     (t
      (setq pos1 (funcall aux -1))
      (goto-char (car pos1))
      (setq pos2 (funcall aux arg))
      (transpose-subr-1 pos1 pos2)
      (goto-char (+ (car pos2) (- (cdr pos1) (car pos1))))))))

;; ---------- undo (GNU simple.el) ----------

(defconst undo-equiv-table (make-hash-table :test 'eq :weakness t)
  "Translation table of `undo-list' elements, used to locate state after redo.")

(defvar undo-in-region nil
  "Non-nil if `pending-undo-list' is not just a tail of `buffer-undo-list'.")

(defvar undo-no-redo nil
  "If t, `undo' doesn't go through redo entries.")

(defvar pending-undo-list nil
  "Within a run of consecutive undo commands, list remaining to be undone.
If t, we undid all the way to the end of it.")

(defun undo--last-change-was-undo-p (undo-list)
  "Return non-nil if the last change was the result of an undo.
The result is usually nil but can be a list of undo elements that
were produced by the undo."
  (while (and (consp undo-list) (eq (car undo-list) nil))
    (setq undo-list (cdr undo-list)))
  (gethash undo-list undo-equiv-table))

(defun undo (&optional arg)
  "Undo some previous changes.
Repeat this command to undo more changes.
A numeric ARG serves as a repeat count.

In Transient Mark mode when the mark is active, undo changes only within
the current region.  Similarly, when not in Transient Mark mode, just \\[universal-argument]
as an argument limits undo to changes within the current region."
  (interactive "*P")
  ;; Make last-command indicate for the next command that this was an undo.
  ;; That way, another undo will undo more.
  ;; If we get to the end of the undo history and get an error,
  ;; another undo command will find the undo history empty
  ;; and will get another error.  To begin undoing the undos,
  ;; you must type some other command.
  (let* ((modified (buffer-modified-p))
	 ;; For an indirect buffer, look in the base buffer for the
	 ;; auto-save data.
	 (base-buffer (or (buffer-base-buffer) (current-buffer)))
	 (recent-save (with-current-buffer base-buffer
			(recent-auto-save-p)))
         ;; Allow certain commands to inhibit an immediately following
         ;; undo-in-region.
         (inhibit-region (and (symbolp last-command)
                              (get last-command 'undo-inhibit-region)))
	 message)
    ;; If we get an error in undo-start,
    ;; the next command should not be a "consecutive undo".
    ;; So set `this-command' to something other than `undo'.
    (setq this-command 'undo-start)
    ;; Here we decide whether to break the undo chain.  If the
    ;; previous command is `undo', we don't call `undo-start', i.e.,
    ;; don't break the undo chain.
    (unless (and (eq last-command 'undo)
		 (or (eq pending-undo-list t)
		     ;; If something (a timer or filter?) changed the buffer
		     ;; since the previous command, don't continue the undo seq.
		     (undo--last-change-was-undo-p buffer-undo-list)))
      (setq undo-in-region
	    (and (or (region-active-p) (and arg (not (numberp arg))))
                 (not inhibit-region)))
      (if undo-in-region
	  (undo-start (region-beginning) (region-end))
	(undo-start))
      ;; get rid of initial undo boundary
      (undo-more 1))
    ;; If we got this far, the next command should be a consecutive undo.
    (setq this-command 'undo)
    ;; Check to see whether we're hitting a redo record, and if
    ;; so, ask the user whether she wants to skip the redo/undo pair.
    (let ((equiv (gethash pending-undo-list undo-equiv-table)))
      (or (eq (selected-window) (active-minibuffer-window))
	  (setq message (format "%s%s"
                                (if (or undo-no-redo (not equiv))
                                    "Undo" "Redo")
                                (if undo-in-region " in region" ""))))
      (when (and (consp equiv) undo-no-redo)
	;; The equiv entry might point to another redo record if we have done
	;; undo-redo-undo-redo-... so skip to the very last equiv.
	(while (let ((next (gethash equiv undo-equiv-table)))
		 (if next (setq equiv next))))
	(setq pending-undo-list (if (consp equiv) equiv t))))
    (undo-more
     (if (numberp arg)
	 (prefix-numeric-value arg)
       1))
    ;; Record the fact that the just-generated undo records come from an
    ;; undo operation--that is, they are redo records.
    ;; In the ordinary case (not within a region), map the redo
    ;; record to the following undos.
    ;; I don't know how to do that in the undo-in-region case.
    (let ((list buffer-undo-list))
      ;; Strip any leading undo boundaries there might be, like we do
      ;; above when checking.
      (while (eq (car list) nil)
	(setq list (cdr list)))
      (puthash list
               (cond
                (undo-in-region 'undo-in-region)
                ;; Prevent identity mapping.  This can happen if
                ;; consecutive nils are erroneously in undo list.  It
                ;; has to map to _something_ so that the next `undo'
                ;; command recognizes that the previous command is
                ;; `undo' and doesn't break the undo chain.
                ((eq list pending-undo-list)
                 (or (gethash list undo-equiv-table)
                     'empty))
                (t pending-undo-list))
	       undo-equiv-table))
    ;; Don't specify a position in the undo record for the undo command.
    ;; Instead, undoing this should move point to where the change is.
    (let ((tail buffer-undo-list)
	  (prev nil))
      (while (car tail)
	(when (integerp (car tail))
	  (let ((pos (car tail)))
	    (if prev
		(setcdr prev (cdr tail))
	      (setq buffer-undo-list (cdr tail)))
	    (setq tail (cdr tail))
	    (while (car tail)
	      (if (eq pos (car tail))
		  (if prev
		      (setcdr prev (cdr tail))
		    (setq buffer-undo-list (cdr tail)))
		(setq prev tail))
	      (setq tail (cdr tail)))
	    (setq tail nil)))
	(setq prev tail tail (cdr tail))))
    ;; Record what the current undo list says,
    ;; so the next command can tell if the buffer was modified in between.
    (and modified (not (buffer-modified-p))
	 (with-current-buffer base-buffer
	   (delete-auto-save-file-if-necessary recent-save)))
    ;; Display a message announcing success.
    (if message
	(message "%s" message))))

(defun delete-auto-save-file-if-necessary (&optional _force)
  "Delete the auto-save file if it is no longer needed.
Called from `undo' when a save state has been restored; remacs does
not write auto-save files, so there is nothing to delete."
  nil)

(defun undo-ignore-read-only (&optional arg)
  "Perform `undo', ignoring the buffer's read-only status.
A numeric ARG serves as a repeat count."
  (interactive "P")
  (let ((inhibit-read-only t))
    (undo arg)))

(defun buffer-disable-undo (&optional buffer)
  "Make BUFFER stop keeping undo information.
No argument or nil as argument means do this for the current buffer."
  (interactive)
  (with-current-buffer (if buffer (get-buffer buffer) (current-buffer))
    (setq buffer-undo-list t)))

(defun undo-only (&optional arg)
  "Undo some previous changes.
Repeat this command to undo more changes.
A numeric ARG serves as a repeat count.
Contrary to `undo', this will not redo a previous undo."
  (interactive "*p")
  (let ((undo-no-redo t)) (undo arg)))

(defun undo-redo (&optional arg)
  "Undo the last ARG undos, i.e., redo the last ARG changes.
Interactively, ARG is the prefix numeric argument and defaults to 1."
  (interactive "*p")
  (cond
   ((not (undo--last-change-was-undo-p buffer-undo-list))
    (user-error "No undone changes to redo"))
   (t
    (let* ((ul buffer-undo-list)
           (new-ul
            (let ((undo-in-progress t))
              (while (and (consp ul) (eq (car ul) nil))
                (setq ul (cdr ul)))
              (primitive-undo (or arg 1) ul)))
           (new-pul (undo--last-change-was-undo-p new-ul)))
      (message "Redo%s" (if undo-in-region " in region" ""))
      (setq this-command 'undo)
      (setq pending-undo-list new-pul)
      (setq buffer-undo-list new-ul)))))

(defun undo-more (n)
  "Undo back N undo-boundaries beyond what was already undone recently.
Call `undo-start' to get ready to undo recent changes,
then call `undo-more' one or more times to undo them."
  (or (listp pending-undo-list)
      (user-error (concat "No further undo information"
                          (and undo-in-region " for region"))))
  (let ((undo-in-progress t))
    ;; Note: The following, while pulling elements off
    ;; `pending-undo-list' will call primitive change functions which
    ;; will push more elements onto `buffer-undo-list'.
    (setq pending-undo-list (primitive-undo n pending-undo-list))
    (if (null pending-undo-list)
	(setq pending-undo-list t))))

(defun undo-start (&optional beg end)
  "Set `pending-undo-list' to the front of the undo list.
The next call to `undo-more' will undo the most recently made change.
If BEG and END are specified, then undo only elements
that apply to text between BEG and END are used; other undo elements
are ignored.  If BEG and END are nil, all undo elements are used."
  (if (eq buffer-undo-list t)
      (user-error "No undo information in this buffer"))
  (setq pending-undo-list
	(if (and beg end (not (= beg end)))
	    (undo-make-selective-list (min beg end) (max beg end))
	  buffer-undo-list)))

;; Deep copy of a list
(defun undo-copy-list (list)
  "Make a copy of undo list LIST."
  (mapcar 'undo-copy-list-1 list))

(defun undo-copy-list-1 (elt)
  (if (consp elt)
      (cons (car elt) (undo-copy-list-1 (cdr elt)))
    elt))

(defun undo-make-selective-list (start end)
  "Return a list of undo elements for the region START to END.
The elements come from `buffer-undo-list', but we keep only the
elements inside this region, and discard those outside this
region.  The elements' positions are adjusted so as the returned
list can be applied to the current buffer."
  (let ((ulist buffer-undo-list)
        ;; A list of position adjusted undo elements in the region.
        (selective-list (list nil))
        ;; A list of undo-deltas for out of region undo elements.
        undo-deltas
        undo-elt)
    (while ulist
      (when undo-no-redo
        (while (consp (gethash ulist undo-equiv-table))
          (setq ulist (gethash ulist undo-equiv-table))))
      (setq undo-elt (car ulist))
      (cond
       ((null undo-elt)
        ;; Don't put two nils together in the list
        (when (car selective-list)
          (push nil selective-list)))
       ((and (consp undo-elt) (eq (car undo-elt) t))
        ;; This is a "was unmodified" element.  Keep it
        ;; if we have kept everything thus far.
        (when (not undo-deltas)
          (push undo-elt selective-list)))
       ;; Skip over marker adjustments, instead relying
       ;; on finding them after (TEXT . POS) elements
       ((markerp (car-safe undo-elt))
        nil)
       (t
        (let ((adjusted-undo-elt (undo-adjust-elt undo-elt
                                                  undo-deltas)))
          (if (undo-elt-in-region adjusted-undo-elt start end)
              (progn
                (setq end (+ end (cdr (undo-delta adjusted-undo-elt))))
                (push adjusted-undo-elt selective-list)
                ;; Keep (MARKER . ADJUSTMENT) if their (TEXT . POS) was
                ;; kept.  primitive-undo may discard them later.
                (when (and (stringp (car-safe adjusted-undo-elt))
                           (integerp (cdr-safe adjusted-undo-elt)))
                  (let ((list-i (cdr ulist)))
                    (while (markerp (car-safe (car list-i)))
                      (push (pop list-i) selective-list)))))
            (let ((delta (undo-delta undo-elt)))
              (when (/= 0 (cdr delta))
                (push delta undo-deltas)))))))
      (pop ulist))
    (nreverse selective-list)))

(defun undo-elt-in-region (undo-elt start end)
  "Determine whether UNDO-ELT falls inside the region START ... END.
If it crosses the edge, we return nil.

Generally this function is not useful for determining
whether (MARKER . ADJUSTMENT) undo elements are in the region,
because markers can be arbitrarily relocated.  Instead, pass the
marker adjustment's corresponding (TEXT . POS) element."
  (cond ((integerp undo-elt)
         (<= start undo-elt end))
	((eq undo-elt nil)
	 t)
	((atom undo-elt)
	 nil)
	((stringp (car undo-elt))
	 ;; (TEXT . POSITION)
	 (<= start (abs (cdr undo-elt)) end))
	((and (consp undo-elt) (markerp (car undo-elt)))
	 ;; (MARKER . ADJUSTMENT)
         (<= start (car undo-elt) end))
	((null (car undo-elt))
	 ;; (nil PROPERTY VALUE BEG . END)
	 (let ((tail (nthcdr 3 undo-elt)))
	   (and (>= (car tail) start)
		(<= (cdr tail) end))))
	((integerp (car undo-elt))
	 ;; (BEGIN . END)
	 (and (>= (car undo-elt) start)
	      (<= (cdr undo-elt) end)))))

(defun undo-elt-crosses-region (undo-elt start end)
  "Test whether UNDO-ELT crosses one edge of that region START ... END.
This assumes we have already decided that UNDO-ELT
is not *inside* the region START...END."
  (declare (obsolete nil "25.1"))
  (cond ((atom undo-elt) nil)
	((null (car undo-elt))
	 ;; (nil PROPERTY VALUE BEG . END)
	 (let ((tail (nthcdr 3 undo-elt)))
	   (and (< (car tail) end)
		(> (cdr tail) start))))
	((integerp (car undo-elt))
	 ;; (BEGIN . END)
	 (and (< (car undo-elt) end)
	      (> (cdr undo-elt) start)))))

(defun undo-adjust-elt (elt deltas)
  "Return adjustment of undo element ELT by the undo DELTAS list."
  (cond
   ;; POSITION
   ((integerp elt)
    (undo-adjust-pos elt deltas))
   ((consp elt)
    (cond
     ;; (BEG . END)
     ((and (integerp (car elt)) (integerp (cdr elt)))
      (undo-adjust-beg-end (car elt) (cdr elt) deltas))
     ;; (TEXT . POSITION)
     ((and (stringp (car elt)) (integerp (cdr elt)))
      (cons (car elt) (* (if (< (cdr elt) 0) -1 1)
			 (undo-adjust-pos (abs (cdr elt)) deltas))))
     ;; (nil PROPERTY VALUE BEG . END)
     ((null (car elt))
      (let* ((l (cdr elt))
	     (prop (car l))
	     (val (cadr l))
	     (tail (nthcdr 2 l)))
	(list nil prop val
	      (car (undo-adjust-beg-end (car tail) (cdr tail) deltas))
	      (cdr (undo-adjust-beg-end (car tail) (cdr tail) deltas)))))
     ;; (apply DELTA START END FUN . ARGS)
     ;; FIXME
     ;; All others return same elt
     (t elt)))
   (t elt)))

;; (BEG . END) can adjust to the same positions, commonly when an
;; insertion was undone and they are out of region, for example:
;;
;; buf pos:
;; 123456789 buffer-undo-list undo-deltas
;; --------- ---------------- -----------
;; [...]
;; abbaa     (2 . 4)          (2 . -2)
;; aaa       ("bb" . 2)       (2 . 2)
;; [...]
;;
;; "bb" insertion (2 . 4) adjusts to (2 . 2) because of the subsequent
;; undo.  Further adjustments to such an element should be the same as
;; for (TEXT . POSITION) elements.  The options are:
;;
;;   1: POSITION adjusts using <= (use-< nil), resulting in behavior
;;      analogous to marker insertion-type t.
;;
;;   2: POSITION adjusts using <, resulting in behavior analogous to
;;      marker insertion-type nil.
;;
;; There was no strong reason to prefer one or the other, except that
;; the first is more consistent with prior undo in region behavior.
(defun undo-adjust-beg-end (beg end deltas)
  "Return cons of adjustments to BEG and END by the undo DELTAS list."
  (let ((adj-beg (undo-adjust-pos beg deltas)))
    ;; Note: option 2 above would be like (cons (min ...) adj-end)
    (cons adj-beg
          (max adj-beg (undo-adjust-pos end deltas t)))))

(defun undo-adjust-pos (pos deltas &optional use-<)
  "Return adjustment of POS by the undo DELTAS list, comparing
with < or <= based on USE-<."
  (dolist (d deltas pos)
    (when (if use-<
              (< (car d) pos)
            (<= (car d) pos))
      (setq pos
            ;; Don't allow pos to become less than the undo-delta
            ;; position.  This edge case is described in the overview
            ;; comments.
            (max (car d) (- pos (cdr d)))))))

;; Return the first affected buffer position and the delta for an undo element
;; delta is defined as the change in subsequent buffer positions if we *did*
;; the undo.
(defun undo-delta (undo-elt)
  (if (consp undo-elt)
      (cond ((stringp (car undo-elt))
	     ;; (TEXT . POSITION)
	     (cons (abs (cdr undo-elt)) (length (car undo-elt))))
	    ((integerp (car undo-elt))
	     ;; (BEGIN . END)
	     (cons (car undo-elt) (- (car undo-elt) (cdr undo-elt))))
	    ;; (apply DELTA BEG END FUNC . ARGS)
	    ((and (eq (car undo-elt) 'apply) (integerp (nth 1 undo-elt)))
	     (cons (nth 2 undo-elt) (nth 1 undo-elt)))
	    (t
	     '(0 . 0)))
    '(0 . 0)))

;; ---------- change groups / atomic changes (GNU subr.el port) ----------

(defun prepare-change-group (&optional buffer)
  "Return a handle for the current buffer's state, for a change group.
If you specify BUFFER, make a handle for BUFFER's state instead."
  (if buffer
      (list (cons buffer (with-current-buffer buffer buffer-undo-list)))
    (list (cons (current-buffer) buffer-undo-list))))

(defun activate-change-group (handle)
  "Activate a change group made with `prepare-change-group' (which see)."
  (dolist (elt handle)
    (with-current-buffer (car elt)
      (if (eq buffer-undo-list t)
	  (setq buffer-undo-list nil)
	;; Add a boundary to make sure the upcoming changes won't be
	;; merged/combined with any previous changes (bug#33341).
        ;; We use for that an "empty insertion", but in order to be harmless,
        ;; it has to be at a harmless position.  Currently only
        ;; insertions are ever merged/combined, so we use such a "boundary"
        ;; only when the last change was an insertion and we use the position
        ;; of the last insertion.
        (when (numberp (car-safe (car buffer-undo-list)))
          (push (cons (caar buffer-undo-list) (caar buffer-undo-list))
                buffer-undo-list))))))

(defun accept-change-group (handle)
  "Finish a change group made with `prepare-change-group' (which see).
This finishes the change group by accepting its changes as final."
  (dolist (elt handle)
    (with-current-buffer (car elt)
      (if (eq (cdr elt) t)
	  (setq buffer-undo-list t)))))

(defun cancel-change-group (handle)
  "Finish a change group made with `prepare-change-group' (which see).
This finishes the change group by reverting all of its changes."
  (dolist (elt handle)
    (with-current-buffer (car elt)
      (setq elt (cdr elt))
      (save-restriction
	;; Widen buffer temporarily so if the buffer was narrowed within
	;; the body of `atomic-change-group' all changes can be undone.
	(widen)
	(let ((old-car (car-safe elt))
	      (old-cdr (cdr-safe elt))
	      ;; Use `pending-undo-list' temporarily since `undo-more' needs
	      ;; it, but restore it afterwards so as not to mess with an
	      ;; ongoing sequence of `undo's.
	      (pending-undo-list
	       ;; Use `buffer-undo-list' unconditionally (bug#39680).
	       buffer-undo-list))
          (unwind-protect
              (progn
                ;; Temporarily truncate the undo log at ELT.
                (when (consp elt)
                  (setcar elt nil) (setcdr elt nil))
                ;; Make sure there's no confusion.
                (when (and (consp elt) (not (eq elt (last pending-undo-list))))
                  (error "Undoing to some unrelated state"))
                ;; Undo it all.
                (save-excursion
                  (while (listp pending-undo-list) (undo-more 1)))
                ;; Revert the undo info to what it was when we grabbed
                ;; the state.
                (setq buffer-undo-list elt))
            ;; Reset the modified cons cell ELT to its original content.
            (when (consp elt)
              (setcar elt old-car)
              (setcdr elt old-cdr))))))))

(defmacro atomic-change-group (&rest body)
  "Like `progn' but perform BODY as an atomic change group.
This means that if BODY exits abnormally,
all of its changes to the current buffer are undone.
This works regardless of whether undo is enabled in the buffer."
  (declare (indent 0) (debug t))
  (let ((handle (make-symbol "--change-group-handle--"))
	(success (make-symbol "--change-group-success--")))
    `(let ((,handle (prepare-change-group))
	   ;; Don't truncate any undo data in the middle of this.
	   (undo-outer-limit nil)
	   (undo-limit most-positive-fixnum)
	   (undo-strong-limit most-positive-fixnum)
	   (,success nil))
       (unwind-protect
	   (progn
	     ;; This is inside the unwind-protect because
	     ;; it enables undo if that was disabled; we need
	     ;; to make sure that it gets disabled again.
	     (activate-change-group ,handle)
	     (prog1 ,(macroexp-progn body)
	       (setq ,success t)))
	 ;; Either of these functions will disable undo
	 ;; if it was disabled before.
	 (if ,success
	     (accept-change-group ,handle)
	   (cancel-change-group ,handle))))))

(defmacro with-undo-amalgamate (&rest body)
  "Like `progn' but perform BODY with amalgamated undo barriers.
This allows multiple operations to be undone in a single step.
When undo is disabled this behaves like `progn'."
  (declare (indent 0) (debug t))
  (let ((handle (make-symbol "--change-group-handle--")))
    `(let ((,handle (prepare-change-group))
           ;; Don't truncate any undo data in the middle of this,
           ;; otherwise Emacs might truncate part of the resulting
           ;; undo step: we want to mimic the behavior we'd get if the
           ;; undo-boundaries were never added in the first place.
           (undo-outer-limit nil)
           (undo-limit most-positive-fixnum)
           (undo-strong-limit most-positive-fixnum))
       (unwind-protect
           (progn
             (activate-change-group ,handle)
             ,@body)
         (progn
           (accept-change-group ,handle)
           (undo-amalgamate-change-group ,handle))))))

(defun undo-amalgamate-change-group (handle)
  "Amalgamate changes in change-group since HANDLE.
Remove all undo boundaries between the state of HANDLE and now.
HANDLE is as returned by `prepare-change-group'."
  (dolist (elt handle)
    (with-current-buffer (car elt)
      (setq elt (cdr elt))
      (when (consp buffer-undo-list)
        (let ((old-car (car-safe elt))
              (old-cdr (cdr-safe elt)))
          (unwind-protect
              (progn
                ;; Temporarily truncate the undo log at ELT.
                (when (consp elt)
                  (setcar elt t) (setcdr elt nil))
                (when
                    (or (null elt)        ;The undo-log was empty.
                        ;; `elt' is still in the log: normal case.
                        (eq elt (last buffer-undo-list))
                        ;; `elt' is not in the log any more, but that's because
                        ;; the log is "all new", so we should remove all
                        ;; boundaries from it.
                        (not (eq (last buffer-undo-list) (last old-cdr))))
                  (setq buffer-undo-list
                        (if (car buffer-undo-list)
                            (delq nil buffer-undo-list)
                          ;; Preserve the undo-boundaries at either ends of the
                          ;; change-groups.
                          (cons nil (delq nil (cdr buffer-undo-list)))))))
            ;; Reset the modified cons cell ELT to its original content.
            (when (consp elt)
              (setcar elt old-car)
              (setcdr elt old-cdr))))))))

;; ---------- whitespace deletion / cycle-spacing (GNU simple.el) ----------

(defun just-one-space (&optional n)
  "Delete all spaces and tabs around point, leaving one space (or N spaces).
Interactively, N is the prefix numeric argument.
If N is negative, delete newlines as well, leaving -N spaces.
See also `cycle-spacing'."
  (interactive "*p")
  (let ((orig-pos        (point))
        (skip-characters (if (and n (< n 0)) " \t\n\r" " \t"))
        (num             (abs (or n 1))))
    (skip-chars-backward skip-characters)
    (constrain-to-field nil orig-pos)
    (let* ((num   (- num (skip-chars-forward " " (+ num (point)))))
           (mid   (point))
           (end   (progn
                    (skip-chars-forward skip-characters)
                    (constrain-to-field nil orig-pos t))))
      (delete-region mid end)
      (insert (make-string num ?\s)))))

(defun delete-space--internal (chars backward-only)
  "Delete CHARS around point.
If BACKWARD-ONLY is non-nil, delete them only before point."
  (let ((orig-pos (point)))
    (delete-region
     (if backward-only
         orig-pos
       (progn
         (skip-chars-forward chars)
         (constrain-to-field nil orig-pos t)))
     (progn
       (skip-chars-backward chars)
       (constrain-to-field nil orig-pos)))))

(defun delete-all-space (&optional backward-only)
  "Delete all spaces, tabs, and newlines around point.
If BACKWARD-ONLY is non-nil, delete them only before point."
  (interactive "*P")
  (delete-space--internal " \t\r\n" backward-only))

(defvar cycle-spacing--context nil
  "Stored context used in consecutive calls to `cycle-spacing' command.
The value is a property list with the following elements:
- `:orig-pos'    The original position of point when starting the
                 sequence.
- `:whitespace-string' All whitespace characters around point
                       including newlines.
- `:n'            The prefix arg given to the initial invocation
                  which is reused for all actions in this cycle.
- `:last-action'  The last action performed in the cycle.")

;; `defcustom' is defined later in this file; a `defvar' is
;; behavior-equivalent here.
(defvar cycle-spacing-actions
  '( just-one-space
     delete-all-space
     restore)
  "List of actions cycled through by `cycle-spacing'.")

(defun cycle-spacing (&optional n)
  "Manipulate whitespace around point in a smart way.
Repeated calls perform the actions in `cycle-spacing-actions' one
after the other, wrapping around after the last one."
  (interactive "*P")
  ;; Initialize `cycle-spacing--context' if needed.
  (when (or (not (equal last-command this-command))
            (not cycle-spacing--context)
            ;; With M-5 M-SPC M-SPC... we pass the prefix arg 5 to
            ;; each action and only start a new cycle when a different
            ;; prefix arg is given and which is not the default value
            ;; 1.
            (and n (not (equal (plist-get cycle-spacing--context :n)
                               n))))
    (let ((orig-pos (point))
          (skip-characters " \t\n\r"))
      (save-excursion
        (skip-chars-backward skip-characters)
        (constrain-to-field nil orig-pos)
        (let ((start (point))
              (end   (progn
                       (skip-chars-forward skip-characters)
                       (constrain-to-field nil orig-pos t))))
          (setq cycle-spacing--context  ;; Save for later.
                (list :orig-pos orig-pos
                      :whitespace-string (buffer-substring start end)
                      :n n
                      :last-action nil))))))

  ;; Cycle through the actions in `cycle-spacing-actions'.
  (when cycle-spacing--context
    (cl-labels ((next-action ()
                  (let* ((l cycle-spacing-actions)
                         (elt (plist-get cycle-spacing--context
                                         :last-action)))
                    (if (null elt)
                        (car cycle-spacing-actions)
                      (catch 'found
                        (while l
                          (cond
                           ((null (cdr l))
                            (throw 'found
                                   (when (eq elt (car l))
                                     (car cycle-spacing-actions))))
                           ((and (eq elt (car l))
                                 (cdr l))
                            (throw 'found (cadr l)))
                           (t (setq l (cdr l)))))))))
                (skip-chars (chars max-dist direction)
                  (if (eq direction 'forward)
                      (skip-chars-forward
                       chars
                       (and max-dist (+ (point) max-dist)))
                    (skip-chars-backward
                     chars
                     (and max-dist (- (point) max-dist)))))
                (delete-space (n include-newlines direction)
                  (let ((orig-point (point))
                        (chars (if include-newlines
                                   " \t\r\n"
                                 " \t")))
                    (when (or (zerop n)
                              (= n (abs (skip-chars chars n direction))))
                      (let ((start (point))
                            (end (progn
                                   (skip-chars chars nil direction)
                                   (point))))
                        (unless (= start end)
                          (delete-region start end))
                        (goto-char (if (eq direction 'forward)
                                       orig-point
                                     (+ n end)))))))
                (restore ()
                  (delete-all-space)
                  (insert (plist-get cycle-spacing--context
                                     :whitespace-string))
                  (goto-char (plist-get cycle-spacing--context
                                        :orig-pos))))
      (let ((action (next-action)))
        (atomic-change-group
          (restore)
          (unless (eq action 'restore)
            ;; action can be some-action or (some-action <arg>) where
            ;; arg is either an integer, the arg to be always used for
            ;; this action or - to use the inverted context n for this
            ;; action.
            (let* ((actual-action (if (listp action)
                                      (car action)
                                    action))
                   (arg (when (listp action)
                          (nth 1 action)))
                   (context-n (plist-get cycle-spacing--context :n))
                   (actual-n (cond
                              ((integerp arg) arg)
                              ((eq 'inverted-arg arg)
                               (* -1 (prefix-numeric-value context-n)))
                              ((eq '- arg) '-)
                              (t context-n)))
                   (numeric-n (prefix-numeric-value actual-n))
                   (include-newlines (or (eq actual-n '-)
                                         (and (integerp actual-n)
                                              (< actual-n 0)))))
              (cond
               ((eq actual-action 'just-one-space)
                (just-one-space numeric-n))
               ((eq actual-action 'delete-space-after)
                (delete-space (if (eq actual-n '-) 0 (abs numeric-n))
                              include-newlines 'forward))
               ((eq actual-action 'delete-space-before)
                (delete-space (if (eq actual-n '-) 0 (abs numeric-n))
                              include-newlines 'backward))
               ((eq actual-action 'delete-all-space)
                (if include-newlines
                    (delete-all-space)
                  (delete-horizontal-space)))
               ((functionp actual-action)
                (funcall actual-action actual-n))
               (t
                (error "Don't know how to handle action %S" action)))))
          (setf (plist-get cycle-spacing--context :last-action)
                action))))))

;; ---------- whitespace.el cleanup (GNU port) ----------

(defmacro without-restriction (&rest rest)
  "Execute BODY without restrictions.

The current restrictions, if any, are restored upon return.

When the optional LABEL argument is present, the restrictions set
by `with-restriction' with the same LABEL argument are lifted.

\(fn [:label LABEL] BODY)"
  (declare (indent 0) (debug t))
  (if (eq (car rest) :label)
      `(save-restriction (internal--labeled-widen ,(cadr rest)) ,@(cddr rest))
    `(save-restriction (widen) ,@rest)))

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
  "Cleanup some blank problems in all buffer or at region."
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

;; ---------- tabify.el (GNU port) ----------

(defvar tabify-regexp " [ \t]+"
  "Regexp matching whitespace that tabify should consider.
Usually this will be \" [ \\t]+\" to match a space followed by whitespace.
\"^\\t* [ \\t]+\" is also useful, for tabifying only initial whitespace.")

(defun untabify (start end &optional _arg)
  "Convert all tabs in region to multiple spaces, preserving columns.
If called interactively with prefix ARG, convert for the entire
buffer.

Called non-interactively, the region is specified by arguments
START and END, rather than by the position of point and mark.
The variable `tab-width' controls the spacing of tab stops."
  (interactive (if current-prefix-arg
		   (list (point-min) (point-max) current-prefix-arg)
		 (list (region-beginning) (region-end) nil)))
  (let ((c (current-column)))
    (save-excursion
      (save-restriction
        (narrow-to-region (point-min) end)
        (goto-char start)
        (while (search-forward "\t" nil t)      ; faster than re-search
          (forward-char -1)
          (let ((tab-beg (point))
                (indent-tabs-mode nil)
                column)
            (skip-chars-forward "\t")
            (setq column (current-column))
            (delete-region tab-beg (point))
            (indent-to column)))))
    (move-to-column c)))

(defun tabify (start end &optional _arg)
  "Convert multiple spaces in region to tabs when possible.
A group of spaces is partially replaced by tabs
when this can be done without changing the column they end at.
If called interactively with prefix ARG, convert for the entire
buffer.

Called non-interactively, the region is specified by arguments
START and END, rather than by the position of point and mark.
The variable `tab-width' controls the spacing of tab stops."
  (interactive (if current-prefix-arg
		   (list (point-min) (point-max) current-prefix-arg)
		 (list (region-beginning) (region-end) nil)))
  (save-excursion
    (save-restriction
      ;; Include the beginning of the line in the narrowing
      ;; since otherwise it will throw off current-column.
      (goto-char start)
      (beginning-of-line)
      (narrow-to-region (point) end)
      (goto-char start)
      (let ((indent-tabs-mode t))
        (while (re-search-forward tabify-regexp nil t)
          ;; The region between (match-beginning 0) and (match-end 0) is just
          ;; spacing which we want to adjust to use TABs where possible.
          (let ((end-col (current-column))
                (beg-col (save-excursion (goto-char (match-beginning 0))
                                         (skip-chars-forward "\t")
                                         (current-column))))
            (if (= (/ end-col tab-width) (/ beg-col tab-width))
                ;; The spacing (after some leading TABs which we wouldn't
                ;; want to touch anyway) does not straddle a TAB boundary,
                ;; so it neither contains a TAB, nor will we be able to use
                ;; a TAB here anyway: there's nothing to do.
                nil
              (delete-region (match-beginning 0) (point))
              (indent-to end-col))))))))

(defun revert-buffer (&rest _ignore)
  "Replace the buffer text with the contents of the visited file."
  (interactive)
  (when buffer-file-name
    (erase-buffer)
    (insert-file-contents buffer-file-name)))

(defun insert-file (filename)
  "Insert the contents of FILENAME into the buffer after point."
  (interactive "*fInsert file: ")
  (insert-file-contents filename))

(defun read-only-mode (&optional arg)
  "Toggle whether the buffer is read-only."
  (interactive "P")
  (setq buffer-read-only (if arg (> (prefix-numeric-value arg) 0)
                           (not buffer-read-only))))

(defun toggle-read-only (&optional arg)
  "Change whether this buffer is read-only."
  (interactive "P")
  (read-only-mode arg))

(defun kill-sexp (&optional arg interactive)
  "Kill the sexp (balanced expression) following point.
With ARG, kill that many sexps after point.
Negative arg -N means kill N sexps before point.
This command assumes point is not in a string or comment.
If INTERACTIVE is non-nil, as it is interactively,
report errors as appropriate for this kind of usage."
  (interactive "p\nd")
  (if interactive
      (condition-case _
          (kill-sexp arg nil)
        (scan-error (user-error (if (> arg 0)
                                    "No next sexp"
                                  "No previous sexp"))))
    (let ((opoint (point)))
      (forward-sexp (or arg 1))
      (kill-region opoint (point)))))

(defun backward-kill-sexp (&optional arg interactive)
  "Kill the sexp (balanced expression) preceding point.
With ARG, kill that many sexps before point.
Negative arg -N means kill N sexps after point.
This command assumes point is not in a string or comment.
If INTERACTIVE is non-nil, as it is interactively,
report errors as appropriate for this kind of usage."
  (interactive "p\nd")
  (kill-sexp (- (or arg 1)) interactive))

(defun kill-sentence (&optional arg)
  "Kill from point to end of sentence."
  (interactive "p")
  (kill-region (point) (save-excursion (forward-sentence arg) (point))))

(defun backward-kill-sentence (&optional arg)
  "Kill back from point to start of sentence."
  (interactive "p")
  (kill-region (point) (save-excursion (backward-sentence arg) (point))))

(defun kill-paragraph (arg)
  "Kill forward to end of paragraph.
With ARG N, kill forward to Nth end of paragraph;
negative ARG -N means kill backward to Nth start of paragraph."
  (interactive "p")
  (kill-region (point) (progn (forward-paragraph arg) (point))))

(defun backward-kill-paragraph (arg)
  "Kill back to start of paragraph.
With ARG N, kill back to Nth start of paragraph;
negative ARG -N means kill forward to Nth end of paragraph."
  (interactive "p")
  (kill-region (point) (progn (backward-paragraph arg) (point))))

(defun transpose-paragraphs (arg)
  "Interchange the current paragraph with the next one.
With prefix argument ARG a non-zero integer, moves the current
paragraph past ARG paragraphs, leaving point after the current paragraph.
If ARG is positive, moves the current paragraph forwards, if
ARG is negative moves it backwards.  If ARG is zero, exchanges
the current paragraph with the one containing the mark."
  (interactive "*p")
  (transpose-subr 'forward-paragraph arg))

(defun transpose-sentences (arg)
  "Interchange the current sentence with the next one.
With prefix argument ARG a non-zero integer, moves the current
sentence past ARG sentences, leaving point after the current sentence.
If ARG is positive, moves the current sentence forwards, if
ARG is negative moves it backwards.  If ARG is zero, exchanges
the current sentence with the one containing the mark."
  (interactive "*p")
  (transpose-subr 'forward-sentence arg))

(defun start-of-paragraph-text ()
  "Move to the start of the current paragraph."
  (let ((opoint (point)) npoint)
    (forward-paragraph -1)
    (setq npoint (point))
    (skip-chars-forward " \t\n")
    ;; If the range of blank lines found spans the original start point,
    ;; try again from the beginning of it.
    ;; Must be careful to avoid infinite loop
    ;; when following a single return at start of buffer.
    (if (and (>= (point) opoint) (< npoint opoint))
	(progn
	  (goto-char npoint)
	  (if (> npoint (point-min))
	      (start-of-paragraph-text))))))

(defun end-of-paragraph-text ()
  "Move to the end of the current paragraph."
  (let ((opoint (point)))
    (forward-paragraph 1)
    (if (eq (preceding-char) ?\n) (forward-char -1))
    (if (<= (point) opoint)
	(progn
	  (forward-char 1)
	  (if (< (point) (point-max))
	      (end-of-paragraph-text))))))

(defun mark-end-of-sentence (arg)
  "Put mark at end of sentence.
ARG works as in `forward-sentence'.  If this command is repeated,
it marks the next ARG sentences after the ones already marked."
  (interactive "p")
  (push-mark
   (save-excursion
     (if (and (eq last-command this-command) (mark t))
	 (goto-char (mark)))
     (forward-sentence arg)
     (point))
   nil t))

(defun forward-paragraph (&optional arg)
  "Move forward to end of paragraph.
With argument ARG, do it ARG times;
a negative argument ARG = -N means move backward N paragraphs.

A line which `paragraph-start' matches either separates paragraphs
\(if `paragraph-separate' matches it also) or is the first line of a paragraph.
A paragraph end is the beginning of a line which is not part of the paragraph
to which the end of the previous line belongs, or the end of the buffer.
Returns the count of paragraphs left to move."
  (interactive "^p")
  (or arg (setq arg 1))
  (let* ((opoint (point))
	 (fill-prefix-regexp
	  (and (boundp 'fill-prefix)
	       fill-prefix (not (equal fill-prefix ""))
	       (not paragraph-ignore-fill-prefix)
	       (regexp-quote fill-prefix)))
	 ;; Remove ^ from paragraph-start and paragraph-sep if they are there.
	 ;; These regexps shouldn't be anchored, because we look for them
	 ;; starting at the left-margin.  This allows paragraph commands to
	 ;; work normally with indented text.
	 (parstart (if (and (not (equal "" paragraph-start))
			    (equal ?^ (aref paragraph-start 0)))
		       (substring paragraph-start 1)
		     paragraph-start))
	 (parsep (if (and (not (equal "" paragraph-separate))
			  (equal ?^ (aref paragraph-separate 0)))
		     (substring paragraph-separate 1)
		   paragraph-separate))
	 (parsep
	  (if fill-prefix-regexp
	      (concat parsep "\\|"
		      fill-prefix-regexp "[ \t]*$")
	    parsep))
	 ;; This is used for searching.
	 (sp-parstart (concat "^[ \t]*\\(?:" parstart "\\|" parsep "\\)"))
	 start found-start)
    (while (and (< arg 0) (not (bobp)))
      (if (and (not (looking-at parsep))
	       (re-search-backward "^\n" (max (1- (point)) (point-min)) t)
	       (looking-at parsep))
	  (setq arg (1+ arg))
	(setq start (point))
	;; Move back over paragraph-separating lines.
	(forward-char -1) (beginning-of-line)
	(while (and (not (bobp))
		    (progn (move-to-left-margin)
			   (looking-at parsep)))
	  (forward-line -1))
	(if (bobp)
	    nil
	  (setq arg (1+ arg))
	  ;; Go to end of the previous (non-separating) line.
	  (end-of-line)
	  ;; Search back for line that starts or separates paragraphs.
	  (if (if fill-prefix-regexp
		  ;; There is a fill prefix; it overrides parstart.
		  (progn
		    (while (and (progn (beginning-of-line) (not (bobp)))
				(progn (move-to-left-margin)
				       (not (looking-at parsep)))
				(looking-at fill-prefix-regexp))
		      (forward-line -1))
		    (move-to-left-margin)
		    (not (bobp)))
		(while (and (re-search-backward sp-parstart nil 1)
			    (setq found-start t)
			    ;; Found a candidate, but need to check if it is a
			    ;; REAL parstart.
			    (progn (setq start (point))
				   (move-to-left-margin)
				   (not (looking-at parsep)))
			    (not (and (looking-at parstart)
				      (or (not use-hard-newlines)
					  (bobp)
					  (get-text-property
					   (1- start) 'hard)))))
		  (setq found-start nil)
		  (goto-char start))
		found-start)
	      ;; Found one.
	      (progn
		;; Move forward over paragraph separators.
		;; We know this cannot reach the place we started
		;; because we know we moved back over a non-separator.
		(while (and (not (eobp))
			    (progn (move-to-left-margin)
				   (looking-at parsep)))
		  (forward-line 1))
		;; If line before paragraph is just margin, back up to there.
		(end-of-line 0)
		(if (> (current-column) (current-left-margin))
		    (forward-char 1)
		  (skip-chars-backward " \t")
		  (if (not (bolp))
		      (forward-line 1))))
	    ;; No starter or separator line => use buffer beg.
	    (goto-char (point-min))))))

    (while (and (> arg 0) (not (eobp)))
      ;; Move forward over separator lines...
      (while (and (not (eobp))
		  (progn (move-to-left-margin) (not (eobp)))
		  (looking-at parsep))
	(forward-line 1))
      (unless (eobp) (setq arg (1- arg)))
      ;; ... and one more line.
      (forward-line 1)
      (if fill-prefix-regexp
	  ;; There is a fill prefix; it overrides parstart.
	  (while (and (not (eobp))
		      (progn (move-to-left-margin) (not (eobp)))
		      (not (looking-at parsep))
		      (looking-at fill-prefix-regexp))
	    (forward-line 1))
	(while (and (re-search-forward sp-parstart nil 1)
		    (progn (setq start (match-beginning 0))
			   (goto-char start)
			   (not (eobp)))
		    (progn (move-to-left-margin)
			   (not (looking-at parsep)))
		    (or (not (looking-at parstart))
			(and use-hard-newlines
			     (not (get-text-property (1- start) 'hard)))))
	  (forward-char 1))
	(if (< (point) (point-max))
	    (goto-char start))))
    (constrain-to-field nil opoint t)
    ;; Return the number of steps that could not be done.
    arg))

(defun backward-paragraph (&optional arg)
  "Move backward to start of paragraph.
With argument ARG, do it ARG times;
a negative argument ARG = -N means move forward N paragraphs.

A paragraph start is the beginning of a line which is a
`paragraph-start' or which is ordinary text and follows a
`paragraph-separate'ing line; except: if the first real line of a
paragraph is preceded by a blank line, the paragraph starts at that
blank line.

See `forward-paragraph' for more information."
  (interactive "^p")
  (or arg (setq arg 1))
  (forward-paragraph (- arg)))

(defvar sentence-end-double-space t
  "Non-nil means a single space does not end a sentence.")

(defvar sentence-end-without-period nil
  "Non-nil means a sentence will end without a period.")

(defvar sentence-end-without-space "。．？！"
  "String of characters that end sentence without following spaces.")

(defvar sentence-end-base "[.?!…‽][]\"'”’)}»›]*"
  "Regexp matching the basic end of a sentence, not including following space.")

(defun sentence-end ()
  "Return the regexp describing the end of a sentence.

This function returns either the value of the variable `sentence-end'
if it is non-nil, or the default value constructed from the
variables `sentence-end-base', `sentence-end-double-space',
`sentence-end-without-period' and `sentence-end-without-space'."
  (or sentence-end
      ;; We accept non-break space along with space.
      (concat (if sentence-end-without-period "\\w[ \u00a0][ \u00a0]\\|")
	      "\\("
	      sentence-end-base
              (if sentence-end-double-space
                  "\\($\\|[ \u00a0]$\\|\t\\|[ \u00a0][ \u00a0]\\)" "\\($\\|[\t \u00a0]\\)")
              "\\|[" sentence-end-without-space "]+"
	      "\\)"
              "[ \u00a0\t\n]*")))

(defun forward-sentence-default-function (&optional arg)
  "Move forward to next end of sentence.  With argument, repeat.
When ARG is negative, move backward repeatedly to start of sentence.

The variable `sentence-end' is a regular expression that matches ends of
sentences.  Also, every paragraph boundary terminates sentences as well."
  (or arg (setq arg 1))
  (let ((opoint (point))
        (sentence-end (sentence-end)))
    (while (< arg 0)
      (let ((pos (point))
	    par-beg par-text-beg)
	(save-excursion
	  (start-of-paragraph-text)
	  ;; Start of real text in the paragraph.
	  ;; We move back to here if we don't see a sentence-end.
	  (setq par-text-beg (point))
	  ;; Start of the first line of the paragraph.
	  ;; We use this as the search limit
	  ;; to allow sentence-end to match if it is anchored at
	  ;; BOL and the paragraph starts indented.
	  (beginning-of-line)
	  (setq par-beg (point)))
	(if (and (re-search-backward sentence-end par-beg t)
		 (or (< (match-end 0) pos)
		     (re-search-backward sentence-end par-beg t)))
	    (goto-char (match-end 0))
	  (goto-char par-text-beg)))
      (setq arg (1+ arg)))
    (while (> arg 0)
      (let ((par-end (save-excursion (end-of-paragraph-text) (point))))
	(if (re-search-forward sentence-end par-end t)
	    (skip-chars-backward " \t\n")
	  (goto-char par-end)))
      (setq arg (1- arg)))
    (constrain-to-field nil opoint t)))

(defvar forward-sentence-function #'forward-sentence-default-function
  "Function to be used to calculate sentence movements.
See `forward-sentence' for a description of its behavior.")

(defun forward-sentence (&optional arg)
  "Move forward to next end of sentence.  With argument ARG, repeat.
If ARG is negative, move backward repeatedly to start of
sentence.  Delegates its work to `forward-sentence-function'."
  (interactive "^p")
  (or arg (setq arg 1))
  (funcall forward-sentence-function arg))

(defun backward-sentence (&optional arg)
  "Move backward to start of sentence.  With argument, repeat.
With negative argument, move forward repeatedly to end of sentence.
See `forward-sentence' for more information."
  (interactive "^p")
  (or arg (setq arg 1))
  (forward-sentence (- arg)))

(defun count-sentences (start end)
  "Count sentences in current buffer from START to END."
  (let ((sentences 0)
        (inhibit-field-text-motion t))
    (save-excursion
      (save-restriction
        (narrow-to-region start end)
        (goto-char (point-min))
        (let* ((prev (point))
               (next (forward-sentence)))
          (while (and (not (null next))
                      (not (= prev next)))
            (setq prev next
                  next (ignore-errors (forward-sentence))
                  sentences (1+ sentences))))
        ;; Remove last possibly empty sentence
        (when (/= (skip-chars-backward " \t\n") 0)
          (setq sentences (1- sentences)))
	sentences))))

;; ---------- page motion (textmodes/page.el) ----------

(defun forward-page (&optional count)
  "Move forward to page boundary.  With arg, repeat, or go back if negative.
A page boundary is any line whose beginning matches the regexp
`page-delimiter'."
  (interactive "p")
  (or count (setq count 1))
  (while (and (> count 0) (not (eobp)))
    (if (and (looking-at page-delimiter)
             (> (match-end 0) (point)))
        ;; If we're standing at the page delimiter, then just skip to
        ;; the end of it.  (But only if it's not a zero-length
        ;; delimiter, because then we wouldn't have forward progress.)
        (goto-char (match-end 0))
      ;; In case the page-delimiter matches the null string,
      ;; don't find a match without moving.
      (when (bolp)
        (forward-char 1))
      (unless (re-search-forward page-delimiter nil t)
        (goto-char (point-max))))
    (setq count (1- count)))
  (while (and (< count 0) (not (bobp)))
    ;; In case the page-delimiter matches the null string,
    ;; don't find a match without moving.
    (and (save-excursion (re-search-backward page-delimiter nil t))
	 (= (match-end 0) (point))
	 (goto-char (match-beginning 0)))
    (unless (bobp)
      (forward-char -1)
      (if (re-search-backward page-delimiter nil t)
	  ;; We found one--move to the end of it.
	  (goto-char (match-end 0))
	;; We found nothing--go to beg of buffer.
	(goto-char (point-min))))
    (setq count (1+ count))))

(defun backward-page (&optional count)
  "Move backward to page boundary.  With arg, repeat, or go fwd if negative.
A page boundary is any line whose beginning matches the regexp
`page-delimiter'."
  (interactive "p")
  (or count (setq count 1))
  (forward-page (- count)))

(defun mark-page (&optional arg)
  "Put mark at end of page, point at beginning.
A numeric arg specifies to move forward or backward by that many pages,
thus marking a page other than the one point was originally in."
  (interactive "P")
  (setq arg (if arg (prefix-numeric-value arg) 0))
  (if (> arg 0)
      (forward-page arg)
    (if (< arg 0)
        (forward-page (1- arg))))
  (forward-page)
  (push-mark nil t t)
  (forward-page -1))

(defun narrow-to-page (&optional arg)
  "Make text outside current page invisible.
A numeric arg specifies to move forward or backward by that many pages,
thus showing a page other than the one point was originally in."
  (interactive "P")
  (setq arg (if arg (prefix-numeric-value arg) 0))
  (save-excursion
    (widen)
    (if (> arg 0)
	(forward-page arg)
      (if (< arg 0)
	  (let ((adjust 0)
		(opoint (point)))
	    ;; If we are not now at the beginning of a page,
	    ;; move back one extra time, to get to the start of this page.
	    (save-excursion
	      (beginning-of-line)
	      (or (and (looking-at page-delimiter)
		       (eq (match-end 0) opoint))
		  (setq adjust 1)))
	    (forward-page (- arg adjust)))))
    ;; Find the end of the page.
    (set-match-data nil)
    (forward-page)
    ;; If we stopped due to end of buffer, stay there.
    ;; If we stopped after a page delimiter, put end of restriction
    ;; at the beginning of that line.
    ;; Before checking the match that was found,
    ;; verify that forward-page actually set the match data.
    (if (and (match-beginning 0)
	     (save-excursion
	       (goto-char (match-beginning 0)) ; was (beginning-of-line)
	       (looking-at page-delimiter)))
	(goto-char (match-beginning 0))) ; was (beginning-of-line)
    (narrow-to-region (point)
		      (progn
			;; Find the top of the page.
			(forward-page -1)
			;; If we found beginning of buffer, stay there.
			;; If extra text follows page delimiter on same line,
			;; include it.
			;; Otherwise, show text starting with following line.
			(if (and (eolp) (not (bobp)))
			    (forward-line 1))
			(point)))))
(put 'narrow-to-page 'disabled t)

;; GNU faces.el: the built-in `default' face's doc string.
(put 'default 'face-documentation "Basic default face.")

(defun page--count-lines-page ()
  "Return a list of line counts on the current page.
The list is on the form (TOTAL BEFORE AFTER), where TOTAL is the
total number of lines on the current page, while BEFORE and AFTER
are the number of lines on the current page before and after
point, respectively."
  (save-excursion
    (let ((opoint (point)))
      (forward-page)
      (beginning-of-line)
      (unless (looking-at page-delimiter)
        (end-of-line))
      (let ((end (point)))
        (backward-page)
        (list (count-lines (point) end)
              (count-lines (point) opoint)
              (count-lines opoint end))))))

(defun count-lines-page ()
  "Report number of lines on current page, and how many are before or after point."
  (interactive)
  (let ((counts (page--count-lines-page)))
    (message (ngettext "Page has %d line (%d + %d)"
                      "Page has %d lines (%d + %d)" (car counts))
             (car counts) (nth 1 counts) (nth 2 counts))))

(defun page--what-page ()
  "Return a list of the page and line number of point.
The line number is relative to the start of the page."
  (save-restriction
    (widen)
    (save-excursion
      (let ((count 1)
            (adjust (if (or (bolp) (looking-back page-delimiter nil)) 1 0))
            (opoint (point)))
        (goto-char (point-min))
        (while (re-search-forward page-delimiter opoint t)
          (when (= (match-beginning 0) (match-end 0))
            (forward-char))
          (setq count (1+ count)))
        (list count (+ adjust (count-lines (point) opoint)))))))

(defun what-page ()
  "Display the page number, and the line number within that page."
  (interactive)
  (apply #'message (cons "Page %d, line %d" (page--what-page))))

(defun center-line (&optional nlines)
  "Center the line point is on (approximate: no-op when unsupported)."
  (interactive "P")
  nlines)

;; `move-to-window-line' is a real subr now (see builtins/misc.rs).

;; ---------- help commands ----------

(defun help-buffer ()
  "Return the *Help* buffer."
  (get-buffer-create "*Help*"))

(defun pop-to-buffer (buffer &optional _action _norecord)
  "Select BUFFER in some window, preferring the current one."
  (interactive "bPop to buffer: ")
  (let ((b (get-buffer-create (if (stringp buffer) buffer
                                (buffer-name buffer)))))
    (set-window-buffer (selected-window) b)
    (set-buffer b)
    b))

(defun switch-to-buffer (buffer)
  "Select BUFFER in the current window."
  (interactive "BSwitch to buffer: ")
  (let ((b (get-buffer-create (if (stringp buffer) buffer
                                (buffer-name buffer)))))
    (set-window-buffer (selected-window) b)
    (set-buffer b)
    b))

(defun list-buffers (&optional files-only)
  "Display a list of existing buffers."
  (interactive "P")
  (let ((b (get-buffer-create "*Buffer List*")))
    (with-current-buffer b
      (erase-buffer)
      (dolist (buf (buffer-list))
        (unless (and files-only (not (buffer-local-value 'buffer-file-name buf)))
          (insert (format "%-24s %s\n"
                          (buffer-name buf)
                          (or (buffer-local-value 'buffer-file-name buf) ""))))))
    (pop-to-buffer b)))

(defun describe-function (function)
  "Display the documentation of FUNCTION."
  (interactive "aDescribe function: ")
  (let ((b (help-buffer)))
    (with-current-buffer b
      (erase-buffer)
      (insert (symbol-name function) " is "
              (cond ((commandp function) "an interactive command")
                    ((subrp function) "a built-in function")
                    (t "a function"))
              ".\n\n"
              (or (documentation function) "Not documented.")
              "\n"))
    (pop-to-buffer b)))

(defun describe-variable (variable)
  "Display the documentation and value of VARIABLE."
  (interactive "vDescribe variable: ")
  (let ((b (help-buffer)))
    (with-current-buffer b
      (erase-buffer)
      (insert (symbol-name variable) " is "
              (if (boundp variable)
                  (concat "bound; its value is\n"
                          (prin1-to-string (symbol-value variable)))
                "void")
              "\n"))
    (pop-to-buffer b)))

(defun describe-key (key)
  "Display the command bound to KEY."
  (interactive "kDescribe key: ")
  (let* ((b (help-buffer))
         (def (key-binding key)))
    (with-current-buffer b
      (erase-buffer)
      (insert (key-description key) " runs the command "
              (prin1-to-string def) "\n"))
    (pop-to-buffer b)
    nil))

(defun describe-bindings (&optional prefix buffer)
  "Display a list of key bindings."
  (interactive "P")
  (let ((b (help-buffer)))
    (with-current-buffer b
      (erase-buffer)
      (insert "key             binding\n---             -------\n")
      (dolist (cell (cdr (current-global-map)))
        (when (consp cell)
          (let ((k (car cell)))
            (insert (cond ((eq k t) "default")
                          ;; Range binding (FROM . TO).
                          ((consp k)
                           (concat (single-key-description (car k))
                                   ".."
                                   (single-key-description (cdr k))))
                          (t (single-key-description k)))
                    "\t\t"
                    (prin1-to-string (cdr cell)) "\n")))))
    (pop-to-buffer b)
    nil))

(defun describe-mode (&optional buffer)
  "Display the current buffer's mode."
  (interactive)
  (message "Mode: %s" (or major-mode "Fundamental")))

(defun apropos-command (pattern)
  "Show commands whose names match PATTERN."
  (interactive "sApropos command: ")
  (let ((b (help-buffer)))
    (with-current-buffer b
      (erase-buffer)
      (dolist (s (apropos-internal pattern t))
        (when (commandp s)
          (insert (symbol-name s) "\n"))))
    (pop-to-buffer b)))

(defun eval-last-sexp (&optional eval-last-sexp-arg-internal)
  "Evaluate the sexp before point."
  (interactive "P")
  (save-excursion
    (backward-sexp)
    (let ((v (eval (read (current-buffer)))))
      (message "%s" v)
      v)))

(defun save-some-buffers (&optional arg)
  "Save some modified file-visiting buffers."
  (interactive "P")
  (mapc (lambda (b)
          (with-current-buffer b
            (when (and buffer-file-name (buffer-modified-p))
              (save-buffer))))
        (buffer-list))
  t)

(defun save-buffers-kill-emacs (&optional arg)
  "Offer to save each buffer, then kill Emacs."
  (interactive "P")
  (save-some-buffers arg)
  (kill-emacs))

(defun isearch-forward (regexp-p string)
  "Search forward; prompts for a string (line-input fallback)."
  (interactive "P\nsI-search: ")
  (when (> (length string) 0)
    (if regexp-p
        (re-search-forward string nil t)
      (search-forward string nil t)))
  string)

(defun isearch-backward (regexp-p string)
  "Search backward; prompts for a string (line-input fallback)."
  (interactive "P\nsI-search backward: ")
  (when (> (length string) 0)
    (if regexp-p
        (re-search-backward string nil t)
      (search-backward string nil t)))
  string)

(defun isearch-forward-regexp (string)
  "Incremental regexp search forward (line-input fallback)."
  (interactive "sI-search regexp: ")
  (when (> (length string) 0)
    (re-search-forward string nil t))
  string)

(defun isearch-backward-regexp (string)
  "Incremental regexp search backward (line-input fallback)."
  (interactive "sI-search backward regexp: ")
  (when (> (length string) 0)
    (re-search-backward string nil t))
  string)

(defun query-replace (from-string to-string &optional delimited)
  "Replace occurrences of FROM-STRING with TO-STRING, asking."
  (interactive "sQuery replace: \nsQuery replace %s with: ")
  (let ((n 0))
    (while (search-forward from-string nil t)
      (when (y-or-n-p (format "Replace with %s? " to-string))
        (replace-match to-string)
        (setq n (1+ n))))
    (message "Replaced %d occurrence(s)" n)
    n))

(defun suspend-frame ()
  "Suspend the current frame (alias for suspend-emacs)."
  (interactive)
  (suspend-emacs))

(defun iconify-or-deiconify-frame ()
  "Iconify the selected frame (no-op on tty)."
  (interactive)
  nil)

;; ---------- command-loop variables ----------

(defvar this-command nil
  "The command now being executed.")
(defvar last-command nil
  "The last command executed.")
(defvar current-prefix-arg nil
  "The value of the prefix argument for this command.")
(defvar transient-mark-mode nil
  "Non-nil if Transient Mark mode is enabled.")
(defvar major-mode 'fundamental-mode
  "The major mode of the current buffer.")
(defvar buffer-read-only nil
  "Non-nil if the current buffer is read-only.")
(defvar command-args nil
  "Arguments supplied to the current command interactively.")
(defvar buffer-file-name nil
  "Name of file visited in the current buffer.")
(defvar left-margin 0
  "Column for the default `indent-line-function' to indent to.
Linefeed-indented lines are indented to this column.")

(defvar fill-prefix nil
  "Text for `fill-region' to put at the beginning of each line, or nil.")

(defvar use-hard-newlines nil
  "Non-nil means to distinguish between soft and hard newlines.")

(defvar paragraph-ignore-fill-prefix nil
  "Non-nil means the paragraph commands are not affected by `fill-prefix'.")

(defvar comment-column 32
  "Column to indent right-margin comments to.")
(defvar comment-start nil
  "String to insert to start a new comment, or nil if none.")
(defvar tab-stop-list nil
  "List of tab stop positions used by `tab-to-tab-stop'.
Elements should be integers or markers.  A nil value means tab stops are
spaced `tab-width' columns apart.")
(defvar indent-line-function 'indent-relative-first-indent-point
  "Function to be used to indent the current line.")

;; ---------- indentation commands ----------

(defun indent-to-column (col &optional minimum)
  "Indent to column COL, or to MINIMUM if already past COL."
  (interactive "NIndent to column: ")
  (indent-to col minimum))

(defun indent-rigidly-left (start end &optional count)
  "Indent all lines between START and END leftward by COUNT spaces."
  (interactive "r\nP")
  (indent-rigidly start end (- (or count 4))))

(defun indent-rigidly-right (start end &optional count)
  "Indent all lines between START and END rightward by COUNT spaces."
  (interactive "r\nP")
  (indent-rigidly start end (or count 4)))

(defun indent-code-rigidly (start end arg &optional nochange-regexp)
  "Indent the region between START and END rigidly by ARG columns."
  (interactive "r\nP")
  (indent-rigidly start end (prefix-numeric-value arg)))

(defun indent-relative (&optional unindented-ok first-only)
  "Space out to under next indent point in previous nonblank line."
  (interactive "P")
  (let ((stops (save-excursion
                 (beginning-of-line)
                 (when (re-search-backward "^[^\n]" nil t)
                   (end-of-line)
                   (let ((eol (point)) (pos nil) (in-run nil))
                     (beginning-of-line)
                     (while (< (point) eol)
                       (let ((c (char-after)))
                         (cond ((memq c '(?\s ?\t))
                                (setq in-run t))
                               (in-run
                                (push (current-column) pos)
                                (setq in-run nil))))
                       (forward-char 1))
                     (nreverse pos))))))
    (let* ((col (current-column))
           (beyond (delq nil (mapcar (lambda (c) (and (> c col) c)) stops)))
           (target (or (car beyond)
                       (and stops (if first-only (car stops) (car stops))))))
      (cond (target (indent-to target))
            (unindented-ok (indent-to 0))
            (t (tab-to-tab-stop))))))

(defun indent-relative-first-indent-point ()
  "Indent to the first indent stop of the previous nonblank line."
  (interactive)
  (indent-relative nil t))

(defun indent-relative-maybe ()
  "Indent like `indent-relative', defaulting to unindented."
  (interactive)
  (indent-relative t))

(defun indent-according-to-mode ()
  "Indent line in proper way for current major mode."
  (interactive)
  (funcall indent-line-function))

(defun indent-region (start end &optional column)
  "Indent each nonblank line in the region using `indent-line-function'."
  (interactive "r")
  (save-excursion
    (goto-char start)
    (setq end (copy-marker end))
    (while (< (point) end)
      (or (and (bolp) (eolp))
          (if column
              (indent-to-column column)
            (indent-according-to-mode)))
      (forward-line 1))))

(defun indent-sexp (&optional endpos)
  "Indent each line of the list starting just after point."
  (interactive "P")
  (let ((e (or endpos (save-excursion (forward-sexp 1) (point)))))
    (indent-region (point) e)))

(defun current-left-margin ()
  "Return the left margin to use for this line.
This is the value of the buffer-local variable `left-margin' plus the value
of the `left-margin' text-property at the start of the line."
  (save-excursion
    (back-to-indentation)
    (max 0
	 (+ left-margin (or (get-text-property
			     (if (and (eobp) (not (bobp)))
				 (1- (point)) (point))
			     'left-margin) 0)))))

(defun move-to-left-margin (&optional n force)
  "Move to the left margin of the current line.
With optional argument, move forward N-1 lines first.
The column moved to is the one given by the `current-left-margin' function.
If the line's indentation appears to be wrong, and this command is called
interactively or with optional argument FORCE, it will be fixed."
  (interactive (list (prefix-numeric-value current-prefix-arg) t))
  (beginning-of-line n)
  (skip-chars-forward " \t")
  (if (minibufferp (current-buffer))
      (if (save-excursion (beginning-of-line) (bobp))
	  (goto-char (minibuffer-prompt-end))
	(beginning-of-line))
    (let ((lm (current-left-margin))
	  (cc (current-column)))
      (cond ((> cc lm)
	     (if (> (move-to-column lm force) lm)
		 ;; If lm is in a tab and we are not forcing, move before tab
		 (backward-char 1)))
	    ((and force (< cc lm))
	     (indent-to-left-margin))))))

(defun indent-to-left-margin ()
  "Indent current line to the column given by `current-left-margin'."
  (save-excursion (indent-line-to (current-left-margin)))
  ;; If we are within the indentation, move past it.
  (when (save-excursion
	  (skip-chars-backward " \t")
	  (bolp))
    (skip-chars-forward " \t")))

(defun delete-to-left-margin (&optional from to)
  "Delete left margin indentation of each line between FROM and TO."
  (interactive "r")
  (let ((from (or from (point)))
        (to (or to (point))))
    (save-excursion
      (goto-char from)
      (while (< (point) to)
        (beginning-of-line)
        (let ((col (current-indentation)))
          (when (> col left-margin)
            (let ((beg (point)))
              (move-to-column left-margin t)
              (delete-region beg (point)))))
        (forward-line 1)))))

(defun center-region (from to &optional nlates)
  "Center each nonblank line between FROM and TO."
  (interactive "r\nP")
  (save-excursion
    (goto-char from)
    (while (< (point) to)
      (unless (and (bolp) (eolp))
        (center-line nlates))
      (forward-line 1))))

(defun indent-for-comment (&optional insert)
  "Indent this line's comment to `comment-column', or insert a comment."
  (interactive)
  (end-of-line)
  (or (eq (preceding-char) ?\s)
      (insert " "))
  (let ((comment-start (or comment-start ";")))
    (delete-horizontal-space)
    (indent-to comment-column 1)
    (insert comment-start)))

(defun insert-parentheses (&optional arg)
  "Enclose following ARG sexps in parentheses."
  (interactive "P")
  (or arg (setq arg 0))
  (insert ?\()
  (save-excursion
    (or (eq arg 0)
        (forward-sexp arg))
    (insert ?\))))

(defun beginning-of-line-text (&optional n)
  "Move to the beginning of the text on this line.
This is like `beginning-of-line', but skips past a comment prefix."
  (interactive "^p")
  (beginning-of-line n)
  (skip-chars-forward " \t"))

(defun fixup-whitespace ()
  "Fixup white space between objects around point.
Leave one space or none, according to the context."
  (interactive "*")
  (save-excursion
    (delete-horizontal-space)
    (if (or (looking-at "^\\|\\s)")
            (save-excursion (forward-char -1)
                            (looking-at "\\\|$\\|\\s(\\|\\s'")))
        nil
      (insert ?\s))))

;; ---------- named-function-key commands ----------
(defun left-char (&optional n)
  "Move point N characters to the left (to the right if N is negative)."
  (interactive "^p")
  (forward-char (- (prefix-numeric-value n))))

(defun right-char (&optional n)
  "Move point N characters to the right (to the left if N is negative)."
  (interactive "^p")
  (forward-char (prefix-numeric-value n)))

(defun delete-forward-char (&optional n killflag)
  "Delete the following N characters (previous if N is negative)."
  (interactive "p\nP")
  (delete-char (prefix-numeric-value n) killflag))

(defun overwrite-mode (&optional arg)
  "Placeholder for Emacs compatibility; no-op in this editor."
  (interactive "P")
  nil)

(defun help-command (key &optional flag)
  "Placeholder help dispatcher."
  (interactive "KHelp: \np")
  nil)

(defun menu-bar-open (&optional frame)
  "Placeholder menu-bar opener."
  (interactive "i\nF")
  nil)

(defun mouse-set-point (event &optional promote-to-region)
  "Move point to the position clicked on with the mouse."
  (interactive "e\np")
  nil)

(defun mouse-set-region (click)
  "Set the region to the interval dragged over."
  (interactive "e")
  nil)

(defun mwheel-scroll (event &optional arg)
  "Scroll up or down according to the EVENT."
  (interactive "e\nP")
  nil)

;; ---------- default global-map bindings ----------

;; Ported from GNU Emacs's default global-map (map-keymap dump).
;; Prefix symbols (Control-X-prefix, ESC-prefix, ...) carry their
;; keymaps in the function cell via the fset forms at the end.
;; `keymap--builtin-maps' collects every map built via make-keymap
;; below so the order-normalization pass can tell them from literal
;; (keymap ...) menu data (which is already in GNU order).
(defvar keymap--builtin-maps nil)
(let ((m (current-global-map)))
(push m keymap--builtin-maps)
(define-key m [0] 'set-mark-command)
(define-key m [1] 'move-beginning-of-line)
(define-key m [2] 'backward-char)
(define-key m [3] 'mode-specific-command-prefix)
(define-key m [4] 'delete-char)
(define-key m [5] 'move-end-of-line)
(define-key m [6] 'forward-char)
(define-key m [7] 'keyboard-quit)
(define-key m [8] 'help-command)
(define-key m [9] 'indent-for-tab-command)
(define-key m [10] 'electric-newline-and-maybe-indent)
(define-key m [11] 'kill-line)
(define-key m [12] 'recenter-top-bottom)
(define-key m [13] 'newline)
(define-key m [14] 'next-line)
(define-key m [15] 'open-line)
(define-key m [16] 'previous-line)
(define-key m [17] 'quoted-insert)
(define-key m [18] 'isearch-backward)
(define-key m [19] 'isearch-forward)
(define-key m [20] 'transpose-chars)
(define-key m [21] 'universal-argument)
(define-key m [22] 'scroll-up-command)
(define-key m [23] 'kill-region)
(define-key m [24] 'Control-X-prefix)
(define-key m [25] 'yank)
(define-key m [26] 'suspend-frame)
(define-key m [27] 'ESC-prefix)
(define-key m [28] 'toggle-input-method)
(define-key m [29] 'abort-recursive-edit)
(define-key m [31] 'undo)
(define-key m [(32 . 126)] 'self-insert-command)
(define-key m [127] 'delete-backward-char)
(define-key m [(128 . 4194303)] 'self-insert-command)
(define-key m [C-wheel-up] 'mouse-wheel-text-scale)
(define-key m [C-wheel-down] 'mouse-wheel-text-scale)
(define-key m [C-mouse-5] 'mouse-wheel-text-scale)
(define-key m [C-mouse-4] 'mouse-wheel-text-scale)
(define-key m [C-M-wheel-up] 'mouse-wheel-global-text-scale)
(define-key m [C-M-wheel-down] 'mouse-wheel-global-text-scale)
(define-key m [C-M-mouse-5] 'mouse-wheel-global-text-scale)
(define-key m [C-M-mouse-4] 'mouse-wheel-global-text-scale)
(define-key m [M-wheel-right] 'mwheel-scroll)
(define-key m [M-wheel-left] 'mwheel-scroll)
(define-key m [M-wheel-up] 'mwheel-scroll)
(define-key m [M-wheel-down] 'mwheel-scroll)
(define-key m [M-mouse-7] 'mwheel-scroll)
(define-key m [M-mouse-6] 'mwheel-scroll)
(define-key m [M-mouse-5] 'mwheel-scroll)
(define-key m [M-mouse-4] 'mwheel-scroll)
(define-key m [S-wheel-right] 'mwheel-scroll)
(define-key m [S-wheel-left] 'mwheel-scroll)
(define-key m [S-wheel-up] 'mwheel-scroll)
(define-key m [S-wheel-down] 'mwheel-scroll)
(define-key m [S-mouse-7] 'mwheel-scroll)
(define-key m [S-mouse-6] 'mwheel-scroll)
(define-key m [S-mouse-5] 'mwheel-scroll)
(define-key m [S-mouse-4] 'mwheel-scroll)
(define-key m [wheel-right] 'mwheel-scroll)
(define-key m [wheel-left] 'mwheel-scroll)
(define-key m [wheel-up] 'mwheel-scroll)
(define-key m [wheel-down] 'mwheel-scroll)
(define-key m [mouse-7] 'mwheel-scroll)
(define-key m [mouse-6] 'mwheel-scroll)
(define-key m [mouse-5] 'mwheel-scroll)
(define-key m [right-fringe wheel-right] 'mwheel-scroll)
(define-key m [right-fringe wheel-left] 'mwheel-scroll)
(define-key m [right-fringe wheel-up] 'mwheel-scroll)
(define-key m [right-fringe wheel-down] 'mwheel-scroll)
(define-key m [right-fringe mouse-7] 'mwheel-scroll)
(define-key m [right-fringe mouse-6] 'mwheel-scroll)
(define-key m [right-fringe mouse-5] 'mwheel-scroll)
(define-key m [right-fringe mouse-4] 'mwheel-scroll)
(define-key m [left-fringe wheel-right] 'mwheel-scroll)
(define-key m [left-fringe wheel-left] 'mwheel-scroll)
(define-key m [left-fringe wheel-up] 'mwheel-scroll)
(define-key m [left-fringe wheel-down] 'mwheel-scroll)
(define-key m [left-fringe mouse-7] 'mwheel-scroll)
(define-key m [left-fringe mouse-6] 'mwheel-scroll)
(define-key m [left-fringe mouse-5] 'mwheel-scroll)
(define-key m [left-fringe mouse-4] 'mwheel-scroll)
(define-key m [right-margin wheel-right] 'mwheel-scroll)
(define-key m [right-margin wheel-left] 'mwheel-scroll)
(define-key m [right-margin wheel-up] 'mwheel-scroll)
(define-key m [right-margin wheel-down] 'mwheel-scroll)
(define-key m [right-margin mouse-7] 'mwheel-scroll)
(define-key m [right-margin mouse-6] 'mwheel-scroll)
(define-key m [right-margin mouse-5] 'mwheel-scroll)
(define-key m [right-margin mouse-4] 'mwheel-scroll)
(define-key m [left-margin wheel-right] 'mwheel-scroll)
(define-key m [left-margin wheel-left] 'mwheel-scroll)
(define-key m [left-margin wheel-up] 'mwheel-scroll)
(define-key m [left-margin wheel-down] 'mwheel-scroll)
(define-key m [left-margin mouse-7] 'mwheel-scroll)
(define-key m [left-margin mouse-6] 'mwheel-scroll)
(define-key m [left-margin mouse-5] 'mwheel-scroll)
(define-key m [left-margin mouse-4] 'mwheel-scroll)
(define-key m [mouse-4] 'mwheel-scroll)
(define-key m [drag-n-drop] 'ns-drag-n-drop)
(define-key m [ns-show-prefs] 'customize)
(define-key m [ns-toggle-toolbar] 'ns-toggle-toolbar)
(define-key m [ns-new-frame] 'make-frame)
(define-key m [ns-spi-service-call] 'ns-spi-service-call)
(define-key m [ns-open-file-line] 'ns-open-file-select-line)
(define-key m [ns-open-temp-file] '[ns-open-file])
(define-key m [ns-open-file] 'ns-find-file)
(define-key m [ns-power-off] 'save-buffers-kill-emacs)
(define-key m [S-mouse-1] 'mouse-save-then-kill)
(define-key m [kp-next] 'scroll-up-command)
(define-key m [kp-prior] 'scroll-down-command)
(define-key m [kp-end] 'end-of-buffer)
(define-key m [kp-home] 'beginning-of-buffer)
(define-key m [s-left] 'move-beginning-of-line)
(define-key m [s-right] 'move-end-of-line)
(define-key m [75497504] 'ns-do-show-character-palette)
(define-key m [s-kp-bar] 'shell-command-on-region)
(define-key m [8388732] 'shell-command-on-region)
(define-key m [8388656] 'text-scale-adjust)
(define-key m [8388669] 'text-scale-adjust)
(define-key m [8388651] 'text-scale-adjust)
(define-key m [8388730] 'undo)
(define-key m [8388729] 'ns-paste-secondary)
(define-key m [8388728] 'kill-region)
(define-key m [8388727] 'delete-frame)
(define-key m [8388726] 'yank)
(define-key m [8388725] 'revert-buffer)
(define-key m [8388724] 'menu-set-font)
(define-key m [8388723] 'save-buffer)
(define-key m [8388721] 'save-buffers-kill-emacs)
(define-key m [8388720] 'ns-print-buffer)
(define-key m [8388719] 'ns-open-file-using-panel)
(define-key m [8388718] 'make-frame)
(define-key m [8388717] 'iconify-frame)
(define-key m [8388716] 'goto-line)
(define-key m [8388715] 'kill-current-buffer)
(define-key m [8388714] 'exchange-point-and-mark)
(define-key m [8388680] 'ns-do-hide-others)
(define-key m [8388712] 'ns-do-hide-emacs)
(define-key m [8388711] 'isearch-repeat-forward)
(define-key m [8388678] 'isearch-backward)
(define-key m [8388710] 'isearch-forward)
(define-key m [8388709] 'isearch-yank-kill)
(define-key m [8388708] 'isearch-repeat-backward)
(define-key m [8388707] 'ns-copy-including-secondary)
(define-key m [8388705] 'mark-whole-buffer)
(define-key m [8388691] 'ns-write-file-using-panel)
(define-key m [8388685] 'manual-entry)
(define-key m [8388684] 'shell-command)
(define-key m [8388677] 'edit-abbrevs)
(define-key m [8388676] 'dired)
(define-key m [8388675] 'ns-popup-color-panel)
(define-key m [8388646] 'kill-current-buffer)
(define-key m [8388702] 'kill-some-buffers)
(define-key m [8388671] 'info)
(define-key m [8388666] 'ispell)
(define-key m [8388653] 'text-scale-adjust)
(define-key m [8388734] 'ns-prev-frame)
(define-key m [8388704] 'other-frame)
(define-key m [8388647] 'next-window-any-frame)
(define-key m [8388652] 'customize)
(define-key m [tool-bar] '(menu-item "tool bar" ignore :filter tool-bar-make-keymap))
(define-key m [tab-bar] '(menu-item "tab bar" (keymap) :filter tab-bar-make-keymap))
(define-key m [C-f10] 'buffer-menu-open)
(define-key m [f10] 'menu-bar-open)
(define-key m [menu-bar mouse-1] 'menu-bar-open-mouse)
(define-key m [menu-bar help-menu] '("Help" keymap (emacs-tutorial menu-item "Emacs Tutorial" help-with-tutorial :help "Learn how to use Emacs") (emacs-tutorial-language-specific menu-item "Emacs Tutorial (choose language)..." help-with-tutorial-spec-language :help "Learn how to use Emacs (choose a language)") (emacs-faq menu-item "Emacs FAQ" view-emacs-FAQ :help "Frequently asked (and answered) questions about Emacs") (emacs-news menu-item "Emacs News" view-emacs-news :help "New features of this version") (emacs-known-problems menu-item "Emacs Known Problems" view-emacs-problems :help "Read about known problems with Emacs") (emacs-manual-bug menu-item "How to Report a Bug" info-emacs-bug :help "Read about how to report an Emacs bug") (send-emacs-bug-report menu-item "Send Bug Report..." report-emacs-bug :help "Send e-mail to Emacs maintainers") (emacs-psychotherapist menu-item "Emacs Psychotherapist" doctor :help "Our doctor will help you feel better") (sep1 "--") (search-documentation menu-item "Search Documentation" (keymap (emacs-terminology menu-item "Emacs Terminology" search-emacs-glossary :help "Display the Glossary section of the Emacs manual") (lookup-subject-in-emacs-manual menu-item "Look Up Subject in User Manual..." emacs-index-search :help "Find description of a subject in Emacs User manual") (lookup-subject-in-elisp-manual menu-item "Look Up Subject in Elisp Manual..." elisp-index-search :help "Find description of a subject in Emacs Lisp manual") (lookup-key-in-manual menu-item "Look Up Key in User Manual..." Info-goto-emacs-key-command-node :help "Display manual section that describes a key") (lookup-command-in-manual menu-item "Look Up Command in User Manual..." Info-goto-emacs-command-node :help "Display manual section that describes a command") (lookup-symbol-in-manual menu-item "Look Up Symbol in Manual..." info-lookup-symbol :help "Display manual section that describes a symbol") (sep1 "--") (find-commands-by-name menu-item "Find Commands by Name..." apropos-command :help "Find commands whose names match a regexp") (find-options-by-name menu-item "Find Options by Name..." apropos-user-option :help "Find user options whose names match a regexp") (find-option-by-value menu-item "Find Options by Value..." apropos-value :help "Find variables whose values match a regexp") (find-any-object-by-name menu-item "Find Any Object by Name..." apropos :help "Find symbols of any kind whose names match a regexp") (search-documentation-strings menu-item "Search Documentation Strings..." apropos-documentation :help "Find functions and variables whose doc strings match a regexp") "Search Documentation")) (describe menu-item "Describe" (keymap (describe-mode menu-item "Describe Buffer Modes" describe-mode :help "Describe this buffer's major and minor mode") (describe-key-1 menu-item "Describe Key or Mouse Operation..." describe-key :help "Display documentation of command bound to a key, a click, or a menu-item") (shortdoc-display-group menu-item "Function Group Overview..." shortdoc-display-group :help "Display a function overview for a specific topic") (describe-command menu-item "Describe Command..." describe-command :help "Display documentation of command") (describe-function menu-item "Describe Function..." describe-function :help "Display documentation of function/command") (describe-variable menu-item "Describe Variable..." describe-variable :help "Display documentation of variable/option") (describe-face menu-item "Describe Face..." describe-face :help "Display the properties of a face") (describe-package menu-item "Describe Package..." describe-package :help "Display documentation of a Lisp package") (describe-current-display-table menu-item "Describe Display Table" describe-current-display-table :help "Describe the current display table") (list-recent-keystrokes menu-item "Show Recent Inputs" view-lossage :help "Display last few input events and the commands they ran") (list-keybindings menu-item "List Key Bindings" describe-bindings :help "Display all current key bindings (keyboard shortcuts)") (separator-desc-mule "--") (describe-language-environment menu-item "Describe Language Environment" (keymap (Default menu-item "Default" describe-specified-language-support) "Describe Language Environment" (Chinese "Chinese" . describe-chinese-environment-map) (Cyrillic "Cyrillic" . describe-cyrillic-environment-map) (Indian "Indian" . describe-indian-environment-map) (Sinhala "Sinhala" . describe-specified-language-support) (English "English" . describe-specified-language-support) (ASCII "ASCII" . describe-specified-language-support) (Ethiopic "Ethiopic" . describe-specified-language-support) (European "European" . describe-european-environment-map) (Turkish "Turkish" . describe-specified-language-support) (Greek "Greek" . describe-specified-language-support) (Hebrew "Hebrew" . describe-specified-language-support) (Windows-1255 "Windows-1255" . describe-specified-language-support) (Japanese "Japanese" . describe-specified-language-support) (Korean "Korean" . describe-specified-language-support) (Lao "Lao" . describe-specified-language-support) (TaiViet "TaiViet" . describe-specified-language-support) (Thai "Thai" . describe-specified-language-support) (Northern\ Thai "Northern Thai" . describe-specified-language-support) (Tibetan "Tibetan" . describe-specified-language-support) (Vietnamese "Vietnamese" . describe-specified-language-support) (IPA "IPA" . describe-specified-language-support) (Arabic "Arabic" . describe-specified-language-support) (Persian "Persian" . describe-specified-language-support) (Syriac "Syriac" . describe-specified-language-support) (Misc "Misc" . describe-misc-environment-map) (UTF-8 "UTF-8" . describe-specified-language-support) (Khmer "Khmer" . describe-specified-language-support) (Burmese "Burmese" . describe-specified-language-support) (Cham "Cham" . describe-specified-language-support) (Philippine "Philippine" . describe-philippine-environment-map) (Indonesian "Indonesian" . describe-indonesian-environment-map))) (describe-input-method menu-item "Describe Input Method..." describe-input-method :help "Keyboard layout for specific input method") (describe-coding-system menu-item "Describe Coding System..." describe-coding-system) (describe-coding-system-briefly menu-item "Describe Coding System (Briefly)" describe-current-coding-system-briefly) (mule-diag menu-item "Show All of Mule Status" mule-diag :help "Display multilingual environment settings") "Describe")) (emacs-manual menu-item "Read the Emacs Manual" info-emacs-manual :help "Full documentation of Emacs features") (more-manuals menu-item "More Manuals" (keymap (emacs-lisp-intro menu-item "Introduction to Emacs Lisp" menu-bar-read-lispintro :help "Read the Introduction to Emacs Lisp Programming") (emacs-lisp-reference menu-item "Emacs Lisp Reference" menu-bar-read-lispref :help "Read the Emacs Lisp Reference manual") (other-manuals menu-item "All Other Manuals (Info)" Info-directory :help "Read any of the installed manuals") (lookup-subject-in-all-manuals menu-item "Lookup Subject in all Manuals..." info-apropos :help "Find description of a subject in all installed manuals") (order-emacs-manuals menu-item "Ordering Manuals" view-order-manuals :help "How to order manuals from the Free Software Foundation") (sep2 "--") (man menu-item "Read Man Page..." manual-entry :help "Man-page docs for external commands and libraries") "More Manuals")) (find-emacs-packages menu-item "Search Built-in Packages" finder-by-keyword :help "Find built-in packages and features by keyword") (external-packages menu-item "Finding Extra Packages" view-external-packages :help "How to get more Lisp packages for use in Emacs") (sep2 "--") (getting-new-versions menu-item "Getting New Versions" describe-distribution :help "How to get the latest version of Emacs") (describe-copying menu-item "Copying Conditions" describe-copying :help "Show the Emacs license (GPL)") (describe-no-warranty menu-item "(Non)Warranty" describe-no-warranty :help "Explain that Emacs has NO WARRANTY") (sep4 "--") (about-emacs menu-item "About Emacs" about-emacs :help "Display version number, copyright info, and basic help") (about-gnu-project menu-item "About GNU" describe-gnu-project :help "About the GNU System, GNU Project, and GNU/Linux") "Help"))
(define-key m [menu-bar file] '("File" keymap (new-file menu-item "Visit New File..." find-file :enable (menu-bar-non-minibuffer-window-p) :help "Specify a new file's name, to edit the file") (open-file menu-item "Open File..." menu-find-file-existing :enable (menu-bar-non-minibuffer-window-p) :help "Read an existing file into an Emacs buffer") (project-open-file menu-item "Open File In Project..." project-find-file :enable (menu-bar-non-minibuffer-window-p) :help "Read existing file that belongs to current project into an Emacs buffer") (open-directory menu-item "Open Directory..." dired-from-menubar :enable (menu-bar-non-minibuffer-window-p) :help "Read a directory, to operate on its files") (project-dired menu-item "Open Project Directory" project-dired :enable (menu-bar-non-minibuffer-window-p) :help "Read the root directory of the current project, to operate on its files") (insert-file menu-item "Insert File..." insert-file :enable (menu-bar-non-minibuffer-window-p) :help "Insert another file into current buffer") (kill-buffer menu-item "Close" kill-this-buffer :enable (kill-this-buffer-enabled-p) :help "Discard (kill) current buffer") (separator-save "--") (save-buffer menu-item "Save" save-buffer :enable (and (buffer-modified-p) (buffer-file-name) (menu-bar-non-minibuffer-window-p)) :help "Save current buffer to its file") (write-file menu-item "Save As..." write-file :enable (and (menu-bar-menu-frame-live-and-visible-p) (menu-bar-non-minibuffer-window-p)) :help "Write current buffer to another file") (revert-buffer menu-item "Revert Buffer" revert-buffer :enable (or (not (eq revert-buffer-function 'revert-buffer--default)) (not (eq revert-buffer-insert-file-contents-function 'revert-buffer-insert-file-contents--default-function)) (and buffer-file-number (or (buffer-modified-p) (not (verify-visited-file-modtime (current-buffer))) (not (eq (not buffer-read-only) (file-writable-p buffer-file-name)))))) :help "Re-read current buffer from its file") (recover-session menu-item "Recover Crashed Session" recover-session :enable (and auto-save-list-file-prefix (file-directory-p (file-name-directory auto-save-list-file-prefix)) (directory-files (file-name-directory auto-save-list-file-prefix) nil (concat "\\`" (regexp-quote (file-name-nondirectory auto-save-list-file-prefix))) t)) :help "Recover edits from a crashed session") (separator-window "--") (new-window-below menu-item "New Window Below" split-window-below :enable (and (menu-bar-menu-frame-live-and-visible-p) (menu-bar-non-minibuffer-window-p)) :help "Make new window below selected one") (new-window-on-right menu-item "New Window on Right" split-window-right :enable (and (menu-bar-menu-frame-live-and-visible-p) (menu-bar-non-minibuffer-window-p)) :help "Make new window on right of selected one") (one-window menu-item "Remove Other Windows" delete-other-windows :enable (not (one-window-p t nil)) :help "Make selected window fill whole frame") (separator-frame "--") (make-frame menu-item "New Frame" make-frame-command :visible (fboundp 'make-frame-command) :help "Open a new frame") (make-frame-on-display menu-item "New Frame on Display Server..." make-frame-on-display :visible (fboundp 'make-frame-on-display) :help "Open a new frame on a display server") (make-frame-on-monitor menu-item "New Frame on Monitor..." make-frame-on-monitor :visible (fboundp 'make-frame-on-monitor) :help "Open a new frame on another monitor") (delete-this-frame menu-item "Delete Frame" delete-frame :visible (fboundp 'delete-frame) :enable (delete-frame-enabled-p) :help "Delete currently selected frame") (undelete-last-deleted-frame menu-item "Undelete Frame" undelete-frame :enable (and undelete-frame-mode (car undelete-frame--deleted-frames)) :help "Undelete the most recently deleted frame") (undelete-frame-mode menu-item "Allow Undeleting Frames" undelete-frame-mode :help "Allow frames to be restored after deletion" :button (:toggle . undelete-frame-mode)) (separator-tab "--") (make-tab menu-item "New Tab" tab-new :visible (fboundp 'tab-new) :help "Open a new tab") (close-tab menu-item "Close Tab" tab-close :visible (fboundp 'tab-close) :help "Close currently selected tab") (separator-print "--") (print menu-item "Print" (keymap (print-buffer menu-item "Print Buffer" print-buffer :enable (menu-bar-menu-frame-live-and-visible-p) :help "Print current buffer with page headings") (print-region menu-item "Print Region" print-region :enable mark-active :help "Print region between mark and current position") (ps-print-buffer-faces menu-item "PostScript Print Buffer" ps-print-buffer-with-faces :enable (menu-bar-menu-frame-live-and-visible-p) :help "Pretty-print current buffer to PostScript printer") (ps-print-region-faces menu-item "PostScript Print Region" ps-print-region-with-faces :enable mark-active :help "Pretty-print marked region to PostScript printer") (ps-print-buffer menu-item "PostScript Print Buffer (B+W)" ps-print-buffer :enable (menu-bar-menu-frame-live-and-visible-p) :help "Pretty-print current buffer in black and white to PostScript printer") (ps-print-region menu-item "PostScript Print Region (B+W)" ps-print-region :enable mark-active :help "Pretty-print marked region in black and white to PostScript printer") "Print")) (separator-exit "--") (exit-emacs menu-item "Quit" save-buffers-kill-terminal :help "Save unsaved buffers, then exit") "File"))
(define-key m [menu-bar edit] '("Edit" keymap (undo menu-item "Undo" undo :enable (and (not buffer-read-only) (not (eq t buffer-undo-list)) (if (eq last-command 'undo) (listp pending-undo-list) (consp buffer-undo-list))) :help "Undo last edits") (undo-redo menu-item "Redo" undo-redo :enable (and (not buffer-read-only) (undo--last-change-was-undo-p buffer-undo-list)) :help "Redo last undone edits") (separator-undo "--") (cut menu-item "Cut" kill-region :enable (and mark-active (not buffer-read-only)) :help "Cut (kill) text in region between mark and current position" :keys nil) (copy menu-item "Copy" ns-copy-including-secondary :enable mark-active :help "Copy text in region between mark and current position" :keys nil) (paste menu-item "Paste" yank :enable (funcall 'nil) :help "Paste (yank) text most recently cut/copied" :keys nil) (select-paste menu-item "Select and Paste" yank-menu :enable (and (cdr yank-menu) (not buffer-read-only)) :help "Choose a string from the kill ring and paste it") (clear menu-item "Clear" delete-active-region :enable (and mark-active (not buffer-read-only)) :help "Delete the text in region between mark and current position") (mark-whole-buffer menu-item "Select All" mark-whole-buffer :help "Mark the whole buffer for a subsequent cut/copy") (separator-search "--") (search menu-item "Search" (keymap (search-forward menu-item "String Forward..." nonincremental-search-forward :help "Search forward for a string") (search-backward menu-item "String Backwards..." nonincremental-search-backward :help "Search backwards for a string") (re-search-forward menu-item "Regexp Forward..." nonincremental-re-search-forward :help "Search forward for a regular expression") (re-search-backward menu-item "Regexp Backwards..." nonincremental-re-search-backward :help "Search backwards for a regular expression") (separator-repeat-search "--") (repeat-search-fwd menu-item "Repeat Forward" nonincremental-repeat-search-forward :enable (or (and (eq menu-bar-last-search-type 'string) search-ring) (and (eq menu-bar-last-search-type 'regexp) regexp-search-ring)) :help "Repeat last search forward") (repeat-search-back menu-item "Repeat Backwards" nonincremental-repeat-search-backward :enable (or (and (eq menu-bar-last-search-type 'string) search-ring) (and (eq menu-bar-last-search-type 'regexp) regexp-search-ring)) :help "Repeat last search backwards") (separator-tag-search "--") (project-search menu-item "Search in Project Files..." project-find-regexp :help "Search for a regexp in files belonging to current project") (tags-srch menu-item "Search Tagged Files..." tags-search :help "Search for a regexp in all tagged files") (tags-continue menu-item "Continue Tags Search" fileloop-continue :enable (and (featurep 'fileloop) (not (eq fileloop--operate-function 'ignore))) :help "Continue last tags search operation") "Search")) (i-search menu-item "Incremental Search" (keymap (isearch-forward menu-item "Forward String..." isearch-forward :help "Search forward for a string as you type it") (isearch-backward menu-item "Backward String..." isearch-backward :help "Search backwards for a string as you type it") (isearch-forward-regexp menu-item "Forward Regexp..." isearch-forward-regexp :help "Search forward for a regular expression as you type it") (isearch-backward-regexp menu-item "Backward Regexp..." isearch-backward-regexp :help "Search backwards for a regular expression as you type it") (isearch-forward-word menu-item "Forward Word..." isearch-forward-word :help "Search forward for a word as you type it") (isearch-forward-symbol menu-item "Forward Symbol..." isearch-forward-symbol :help "Search forward for a symbol as you type it") (isearch-forward-symbol-at-point menu-item "Forward Symbol at Point..." isearch-forward-symbol-at-point :help "Search forward for a symbol found at point") "Incremental Search")) (replace menu-item "Replace" (keymap (query-replace menu-item "Replace String..." query-replace :enable (not buffer-read-only) :help "Replace string interactively, ask about each occurrence") (query-replace-regexp menu-item "Replace Regexp..." query-replace-regexp :enable (not buffer-read-only) :help "Replace regular expression interactively, ask about each occurrence") (separator-replace-tags "--") (project-replace menu-item "Replace in Project Files..." project-query-replace-regexp :help "Interactively replace a regexp in files belonging to current project") (tags-repl menu-item "Replace in Tagged Files..." tags-query-replace :help "Interactively replace a regexp in all tagged files") (tags-repl-continue menu-item "Continue Replace" fileloop-continue :enable (and (featurep 'fileloop) (not (eq fileloop--operate-function 'ignore))) :help "Continue last tags replace operation") "Replace")) (goto menu-item "Go To" (keymap (go-to-line menu-item "Goto Line..." goto-line :help "Read a line number and go to that line") (go-to-pos menu-item "Goto Buffer Position..." goto-char :help "Read a number N and go to buffer position N") (beg-of-buf menu-item "Goto Beginning of Buffer" beginning-of-buffer) (end-of-buf menu-item "Goto End of Buffer" end-of-buffer) (separator-xref "--") (xref-find-def menu-item "Find Definition..." xref-find-definitions :help "Find definition of function or variable") (xref-find-otherw menu-item "Find Definition in Other Window..." xref-find-definitions-other-window :help "Find function/variable definition in another window") (xref-apropos menu-item "Find Apropos..." xref-find-apropos :help "Find function/variables whose names match regexp") (xref-pop menu-item "Back" xref-go-back :visible (and (featurep 'xref) (not (xref-marker-stack-empty-p))) :help "Back to the position of the last search") (xref-forward menu-item "Forward" xref-go-forward :visible (and (featurep 'xref) (not (xref-forward-history-empty-p))) :help "Forward to the position gone Back from") (separator-tag-file menu-item "--" nil :visible (menu-bar-goto-uses-etags-p)) (set-tags-name menu-item "Set Tags File Name..." visit-tags-table :visible (menu-bar-goto-uses-etags-p) :help "Tell navigation commands which tag table file to use") "Go To")) (bookmark menu-item "Bookmarks" menu-bar-bookmark-map) (separator-bookmark "--") (fill menu-item "Fill" fill-region :enable (and mark-active (not buffer-read-only)) :help "Fill text in region to fit between left and right margin") (spell menu-item "Spell" ispell-menu-map) (execute-extended-command menu-item "Execute Command" execute-extended-command :enable t :help "Read a command name, its arguments, then call it.") "Edit"))
(define-key m [menu-bar options] '("Options" keymap (transient-mark-mode menu-item "Highlight Active Region" transient-mark-mode :enable (not cua-mode) :help "Make text in active region stand out in color (Transient Mark mode)" :button (:toggle and (default-boundp 'transient-mark-mode) (default-value 'transient-mark-mode))) (highlight-paren-mode menu-item "Highlight Matching Parentheses" show-paren-mode :help "Highlight matching/mismatched parentheses at cursor (Show Paren mode)" :button (:toggle and (default-boundp 'show-paren-mode) (default-value 'show-paren-mode))) (highlight-separator "--") (line-wrapping menu-item "Line Wrapping in This Buffer" (keymap (window-wrap menu-item "Wrap at Window Edge" menu-bar--wrap-long-lines-window-edge :help "Wrap long lines at window edge" :button (:radio and (null truncate-lines) (not (truncated-partial-width-window-p)) (not word-wrap)) :visible (menu-bar-menu-frame-live-and-visible-p) :enable (not (truncated-partial-width-window-p))) (truncate menu-item "Truncate Long Lines" menu-bar--toggle-truncate-long-lines :help "Truncate long lines at window edge" :button (:radio or truncate-lines (truncated-partial-width-window-p)) :visible (menu-bar-menu-frame-live-and-visible-p) :enable (not (truncated-partial-width-window-p))) (word-wrap menu-item "Word Wrap (Visual Line mode)" menu-bar--visual-line-mode-enable :help "Wrap long lines at word boundaries" :button (:radio and (null truncate-lines) (not (truncated-partial-width-window-p)) word-wrap) :visible (menu-bar-menu-frame-live-and-visible-p)) (visual-wrap menu-item "Visual Wrap Prefix mode" visual-wrap-prefix-mode :help "Display continuation lines with visual context-dependent prefix" :visible (menu-bar-menu-frame-live-and-visible-p) :button (:toggle bound-and-true-p visual-wrap-prefix-mode) :enable t) "Line Wrapping")) (search-options menu-item "Default Search Options" (keymap (case-fold-search menu-item "Ignore Case" toggle-case-fold-search :help "Ignore letter-case in search commands" :button (:toggle and (default-boundp 'case-fold-search) (default-value 'case-fold-search))) (custom-separator "--") (regular-search menu-item "Literal Search" nil :help "Disable special search modes" :button (:radio not search-default-mode)) (regexp-search menu-item "Regular Expression" nil :help "Enable regular-expression search" :button (:radio eq search-default-mode t)) (word-search-regexp menu-item "Whole Words" nil :help "Enable whole word search" :button (:radio eq search-default-mode #'word-search-regexp)) (isearch-symbol-regexp menu-item "Whole Symbols" nil :help "Enable whole symbol search" :button (:radio eq search-default-mode #'isearch-symbol-regexp)) (char-fold-to-regexp menu-item "Fold Characters" nil :help "Enable character folding search" :button (:radio eq search-default-mode #'char-fold-to-regexp)) "Search Options")) (cua-emulation-mode menu-item "CUA Mode (without C-x/C-c/C-v)" cua-mode :visible (and (boundp 'cua-enable-cua-keys) (not cua-enable-cua-keys)) :help "Enable CUA Mode without rebinding C-x/C-c/C-v keys" :button (:toggle and (default-boundp 'cua-mode) (default-value 'cua-mode))) (cua-mode menu-item "Cut/Paste with C-x/C-c/C-v (CUA Mode)" cua-mode :visible (or (not (boundp 'cua-enable-cua-keys)) cua-enable-cua-keys) :help "Use C-z/C-x/C-c/C-v keys for undo/cut/copy/paste" :button (:toggle and (default-boundp 'cua-mode) (default-value 'cua-mode))) (edit-options-separator "--") (uniquify menu-item "Use Directory Names in Buffer Names" toggle-uniquify-buffer-names :help "Uniquify buffer names by adding parent directory names" :button (:toggle and (default-boundp 'uniquify-buffer-name-style) (default-value 'uniquify-buffer-name-style))) (save-place menu-item "Save Place in Files between Sessions" toggle-save-place-globally :help "Visit files of previous session when restarting Emacs" :button (:toggle and (default-boundp 'save-place-mode) (default-value 'save-place-mode))) (save-desktop menu-item "Save State between Sessions" toggle-save-desktop-globally :help "Visit desktop of previous session when restarting Emacs" :button (:toggle and (default-boundp 'desktop-save-mode) (default-value 'desktop-save-mode))) (cursor-separator "--") (blink-cursor-mode menu-item "Blink Cursor" blink-cursor-mode :help "Whether the cursor blinks (Blink Cursor mode)" :button (:toggle and (default-boundp 'blink-cursor-mode) (default-value 'blink-cursor-mode))) (debugger-separator "--") (debug-on-error menu-item "Enter Debugger on Error" toggle-debug-on-error :help "Enter Lisp debugger when an error is signaled" :button (:toggle and (default-boundp 'debug-on-error) (default-value 'debug-on-error))) (debug-on-quit menu-item "Enter Debugger on Quit/C-g" toggle-debug-on-quit :help "Enter Lisp debugger when C-g is pressed" :button (:toggle and (default-boundp 'debug-on-quit) (default-value 'debug-on-quit))) (mule-separator "--") (mule menu-item "Multilingual Environment" (keymap (set-language-environment menu-item "Set Language Environment" (keymap (Default menu-item "Default" setup-specified-language-environment) "Set Language Environment" (Chinese "Chinese" . setup-chinese-environment-map) (Cyrillic "Cyrillic" . setup-cyrillic-environment-map) (Indian "Indian" . setup-indian-environment-map) (Sinhala "Sinhala" . setup-specified-language-environment) (English "English" . setup-specified-language-environment) (ASCII "ASCII" . setup-specified-language-environment) (Ethiopic "Ethiopic" . setup-specified-language-environment) (European "European" . setup-european-environment-map) (Turkish "Turkish" . setup-specified-language-environment) (Greek "Greek" . setup-specified-language-environment) (Hebrew "Hebrew" . setup-specified-language-environment) (Windows-1255 "Windows-1255" . setup-specified-language-environment) (Japanese "Japanese" . setup-specified-language-environment) (Korean "Korean" . setup-specified-language-environment) (Lao "Lao" . setup-specified-language-environment) (TaiViet "TaiViet" . setup-specified-language-environment) (Thai "Thai" . setup-specified-language-environment) (Northern\ Thai "Northern Thai" . setup-specified-language-environment) (Tibetan "Tibetan" . setup-specified-language-environment) (Vietnamese "Vietnamese" . setup-specified-language-environment) (IPA "IPA" . setup-specified-language-environment) (Arabic "Arabic" . setup-specified-language-environment) (Persian "Persian" . setup-specified-language-environment) (Syriac "Syriac" . setup-specified-language-environment) (Misc "Misc" . setup-misc-environment-map) (UTF-8 "UTF-8" . setup-specified-language-environment) (Khmer "Khmer" . setup-specified-language-environment) (Burmese "Burmese" . setup-specified-language-environment) (Cham "Cham" . setup-specified-language-environment) (Philippine "Philippine" . setup-philippine-environment-map) (Indonesian "Indonesian" . setup-indonesian-environment-map))) (separator-mule "--") (toggle-input-method menu-item "Toggle Input Method" toggle-input-method) (set-input-method menu-item "Select Input Method..." set-input-method) (activate-transient-input-method menu-item "Transient Input Method" activate-transient-input-method) (separator-input-method "--") (set-various-coding-system menu-item "Set Coding Systems" (keymap (universal-coding-system-argument menu-item "For Next Command" universal-coding-system-argument :help "Coding system to be used by next command") (separator-1 "--") (set-buffer-file-coding-system menu-item "For Saving This Buffer" set-buffer-file-coding-system :help "How to encode this buffer when saved") (revert-buffer-with-coding-system menu-item "For Reverting This File Now" revert-buffer-with-coding-system :enable buffer-file-name :help "Revisit this file immediately using specified coding system") (set-file-name-coding-system menu-item "For File Name" set-file-name-coding-system :help "How to decode/encode file names") (separator-2 "--") (set-keyboard-coding-system menu-item "For Keyboard" set-keyboard-coding-system :help "How to decode keyboard input") (set-terminal-coding-system menu-item "For Terminal" set-terminal-coding-system :enable (null (memq initial-window-system '(x w32 ns haiku pgtk android))) :help "How to encode terminal output") (separator-3 "--") (set-selection-coding-system menu-item "For X Selections/Clipboard" set-selection-coding-system :visible (display-selections-p) :help "How to en/decode data to/from selection/clipboard") (set-next-selection-coding-system menu-item "For Next X Selection" set-next-selection-coding-system :visible (display-selections-p) :help "How to en/decode next selection/clipboard operation") (set-buffer-process-coding-system menu-item "For I/O with Subprocess" set-buffer-process-coding-system :visible (fboundp 'make-process) :enable (get-buffer-process (current-buffer)) :help "How to en/decode I/O from/to subprocess connected to this buffer") "Set Coding System")) (view-hello-file menu-item "Show Multilingual Sample Text" view-hello-file :enable (file-readable-p (expand-file-name "HELLO" data-directory)) :help "Demonstrate various character sets") (separator-coding-system "--") (describe-language-environment menu-item "Describe Language Environment" (keymap (Default menu-item "Default" describe-specified-language-support) "Describe Language Environment" (Chinese "Chinese" . describe-chinese-environment-map) (Cyrillic "Cyrillic" . describe-cyrillic-environment-map) (Indian "Indian" . describe-indian-environment-map) (Sinhala "Sinhala" . describe-specified-language-support) (English "English" . describe-specified-language-support) (ASCII "ASCII" . describe-specified-language-support) (Ethiopic "Ethiopic" . describe-specified-language-support) (European "European" . describe-european-environment-map) (Turkish "Turkish" . describe-specified-language-support) (Greek "Greek" . describe-specified-language-support) (Hebrew "Hebrew" . describe-specified-language-support) (Windows-1255 "Windows-1255" . describe-specified-language-support) (Japanese "Japanese" . describe-specified-language-support) (Korean "Korean" . describe-specified-language-support) (Lao "Lao" . describe-specified-language-support) (TaiViet "TaiViet" . describe-specified-language-support) (Thai "Thai" . describe-specified-language-support) (Northern\ Thai "Northern Thai" . describe-specified-language-support) (Tibetan "Tibetan" . describe-specified-language-support) (Vietnamese "Vietnamese" . describe-specified-language-support) (IPA "IPA" . describe-specified-language-support) (Arabic "Arabic" . describe-specified-language-support) (Persian "Persian" . describe-specified-language-support) (Syriac "Syriac" . describe-specified-language-support) (Misc "Misc" . describe-misc-environment-map) (UTF-8 "UTF-8" . describe-specified-language-support) (Khmer "Khmer" . describe-specified-language-support) (Burmese "Burmese" . describe-specified-language-support) (Cham "Cham" . describe-specified-language-support) (Philippine "Philippine" . describe-philippine-environment-map) (Indonesian "Indonesian" . describe-indonesian-environment-map)) :help "Show multilingual settings for a specific language") (describe-input-method menu-item "Describe Input Method..." describe-input-method :help "Keyboard layout for a specific input method") (describe-coding-system menu-item "Describe Coding System..." describe-coding-system) (list-character-sets menu-item "List Character Sets" list-character-sets :help "Show table of available character sets") (mule-diag menu-item "Show All Multilingual Settings" mule-diag :help "Display multilingual environment settings") "Mule (Multilingual Environment)")) (showhide-separator "--") (showhide menu-item "Show/Hide" (keymap (showhide-tool-bar menu-item "Tool Bar" toggle-tool-bar-mode-from-frame :help "Turn tool bar on/off" :visible (display-graphic-p) :button (:toggle menu-bar-positive-p (frame-parameter (menu-bar-frame-for-menubar) 'tool-bar-lines))) (showhide-tab-bar menu-item "Tab Bar" toggle-tab-bar-mode-from-frame :help "Turn tab bar on/off" :button (:toggle menu-bar-positive-p (frame-parameter (menu-bar-frame-for-menubar) 'tab-bar-lines))) (menu-bar-mode menu-item "Menu Bar" toggle-menu-bar-mode-from-frame :help "Turn menu bar on/off" :button (:toggle menu-bar-positive-p (frame-parameter (menu-bar-frame-for-menubar) 'menu-bar-lines))) (showhide-context-menu menu-item "Context Menus" context-menu-mode :help "Turn mouse-3 context menus on/off" :button (:toggle . context-menu-mode)) (showhide-tooltip-mode menu-item "Tooltips" tooltip-mode :help "Turn tooltips on/off" :visible (and (display-graphic-p) (fboundp 'x-show-tip)) :button (:toggle . tooltip-mode)) (showhide-scroll-bar menu-item "Scroll Bar" (keymap (none menu-item "No Vertical Scroll Bar" menu-bar-no-scroll-bar :help "Turn off vertical scroll bar" :visible (display-graphic-p) :button (:radio eq scroll-bar-mode nil)) (left menu-item "On the Left" menu-bar-left-scroll-bar :help "Scroll bar on the left side" :visible (display-graphic-p) :button (:radio and scroll-bar-mode (eq (frame-parameter nil 'vertical-scroll-bars) 'left))) (right menu-item "On the Right" menu-bar-right-scroll-bar :help "Scroll bar on the right side" :visible (display-graphic-p) :button (:radio and scroll-bar-mode (eq (frame-parameter nil 'vertical-scroll-bars) 'right))) (scrollbar-separator "--") (horizontal menu-item "Horizontal" horizontal-scroll-bar-mode :help "Horizontal scroll bar" :button (:toggle and (default-boundp 'horizontal-scroll-bar-mode) (default-value 'horizontal-scroll-bar-mode))) "Scroll Bar") :visible (display-graphic-p)) (showhide-fringe menu-item "Fringe" (keymap (none menu-item "None" menu-bar-showhide-fringe-menu-customize-disable :help "Turn off fringe" :visible (display-graphic-p) :button (:radio eq fringe-mode 0)) (left menu-item "On the Left" menu-bar-showhide-fringe-menu-customize-left :help "Fringe only on the left side" :visible (display-graphic-p) :button (:radio equal fringe-mode '(nil . 0))) (right menu-item "On the Right" menu-bar-showhide-fringe-menu-customize-right :help "Fringe only on the right side" :visible (display-graphic-p) :button (:radio equal fringe-mode '(0))) (default menu-item "Default" menu-bar-showhide-fringe-menu-customize-reset :help "Default width fringe on both left and right side" :visible (display-graphic-p) :button (:radio eq fringe-mode nil)) (customize menu-item "Customize Fringe" menu-bar-showhide-fringe-menu-customize :help "Detailed customization of fringe" :visible (display-graphic-p)) (indicate-empty-lines menu-item "Empty Line Indicators" toggle-indicate-empty-lines :help "Indicate trailing empty lines in fringe, globally" :button (:toggle and (default-boundp 'indicate-empty-lines) (default-value 'indicate-empty-lines))) (showhide-fringe-ind menu-item "Buffer Boundaries" (keymap (none menu-item "No Indicators" menu-bar-showhide-fringe-ind-none :help "Hide all buffer boundary indicators and arrows" :visible (display-graphic-p) :button (:radio eq indicate-buffer-boundaries nil)) (left menu-item "In Left Fringe" menu-bar-showhide-fringe-ind-left :help "Show buffer boundaries and arrows in left fringe" :visible (display-graphic-p) :button (:radio eq indicate-buffer-boundaries 'left)) (right menu-item "In Right Fringe" menu-bar-showhide-fringe-ind-right :help "Show buffer boundaries and arrows in right fringe" :visible (display-graphic-p) :button (:radio eq indicate-buffer-boundaries 'right)) (box menu-item "Opposite, No Arrows" menu-bar-showhide-fringe-ind-box :help "Show top/bottom indicators in opposite fringes, no arrows" :visible (display-graphic-p) :button (:radio equal indicate-buffer-boundaries '((top . left) (bottom . right)))) (mixed menu-item "Opposite, Arrows Right" menu-bar-showhide-fringe-ind-mixed :help "Show top/bottom indicators in opposite fringes, arrows in right" :visible (display-graphic-p) :button (:radio equal indicate-buffer-boundaries '((t . right) (top . left)))) (customize menu-item "Other (Customize)" menu-bar-showhide-fringe-ind-customize :help "Additional choices available through Custom buffer" :visible (display-graphic-p) :button (:radio not (member indicate-buffer-boundaries '(nil left right ((top . left) (bottom . right)) ((t . right) (top . left)))))) "Buffer boundaries") :visible (display-graphic-p) :help "Indicate buffer boundaries in fringe") "Fringe") :visible (display-graphic-p)) (showhide-window-divider menu-item "Window Divider" (keymap (no-divider menu-item "None" menu-bar-no-window-divider :help "Do not display window dividers" :visible (memq (window-system) '(x w32)) :button (:radio and (not (window-divider-width-valid-p (cdr (assq 'bottom-divider-width (frame-parameters))))) (not (window-divider-width-valid-p (cdr (assq 'right-divider-width (frame-parameters))))))) (bottom-only menu-item "Bottom Only" menu-bar-bottom-window-divider :help "Display window divider on the bottom of each window only" :visible (memq (window-system) '(x w32)) :button (:radio and (window-divider-width-valid-p (cdr (assq 'bottom-divider-width (frame-parameters)))) (not (window-divider-width-valid-p (cdr (assq 'right-divider-width (frame-parameters))))))) (right-only menu-item "Right Only" menu-bar-right-window-divider :help "Display window divider on the right of each window only" :visible (memq (window-system) '(x w32)) :button (:radio and (not (window-divider-width-valid-p (cdr (assq 'bottom-divider-width (frame-parameters))))) (window-divider-width-valid-p (cdr (assq 'right-divider-width (frame-parameters)))))) (bottom-and-right menu-item "Bottom and Right" menu-bar-bottom-and-right-window-divider :help "Display window divider on the bottom and right of each window" :visible (memq (window-system) '(x w32)) :button (:radio and (window-divider-width-valid-p (cdr (assq 'bottom-divider-width (frame-parameters)))) (window-divider-width-valid-p (cdr (assq 'right-divider-width (frame-parameters)))))) (customize menu-item "Customize" menu-bar-window-divider-customize :help "Customize window dividers" :visible (memq (window-system) '(x w32))) "Window Divider") :visible (memq (window-system) '(x w32))) (showhide-tab-line-mode menu-item "Window Tab Line" global-tab-line-mode :help "Turn window-local tab-lines on/off" :visible (fboundp 'global-tab-line-mode) :button (:toggle . global-tab-line-mode)) (showhide-outline-minor-mode menu-item "Outlines" outline-minor-mode :help "Turn outline-minor-mode on/off" :visible (seq-some #'local-variable-p '(outline-search-function outline-regexp outline-level)) :button (:toggle bound-and-true-p outline-minor-mode)) (showhide-speedbar menu-item "Speedbar" speedbar-frame-mode :help "Display a Speedbar quick-navigation frame" :button (:toggle and (boundp 'speedbar-frame) (frame-live-p (symbol-value 'speedbar-frame)) (frame-visible-p (symbol-value 'speedbar-frame)))) (datetime-separator "--") (showhide-date-time menu-item "Time, Load and Mail" display-time-mode :help "Display time, system load averages and mail status in mode line" :button (:toggle and (default-boundp 'display-time-mode) (default-value 'display-time-mode))) (showhide-battery menu-item "Battery Status" display-battery-mode :help "Display battery status information in mode line" :button (:toggle and (default-boundp 'display-battery-mode) (default-value 'display-battery-mode))) (linecolumn-separator "--") (size-indication-mode menu-item "Size Indication" size-indication-mode :help "Show the size of the buffer in the mode line" :button (:toggle and (default-boundp 'size-indication-mode) (default-value 'size-indication-mode))) (line-number-mode menu-item "Line Numbers in Mode Line" line-number-mode :help "Show the current line number in the mode line" :button (:toggle and (default-boundp 'line-number-mode) (default-value 'line-number-mode))) (column-number-mode menu-item "Column Numbers in Mode Line" column-number-mode :help "Show the current column number in the mode line" :button (:toggle and (default-boundp 'column-number-mode) (default-value 'column-number-mode))) (display-line-numbers menu-item "Line Numbers for All Lines" (keymap (global menu-item "Global Line Numbers Mode" global-display-line-numbers-mode :help "Set line numbers globally" :button (:toggle and (default-boundp 'global-display-line-numbers-mode) (default-value 'global-display-line-numbers-mode))) (none menu-item "No Line Numbers" menu-bar--display-line-numbers-mode-none :help "Disable line numbers" :button (:radio null display-line-numbers) :visible (menu-bar-menu-frame-live-and-visible-p)) (absolute menu-item "Absolute Line Numbers" menu-bar--display-line-numbers-mode-absolute :help "Enable absolute line numbers" :button (:radio eq display-line-numbers t) :visible (menu-bar-menu-frame-live-and-visible-p)) (relative menu-item "Relative Line Numbers" menu-bar--display-line-numbers-mode-relative :help "Enable relative line numbers" :button (:radio eq display-line-numbers 'relative) :visible (menu-bar-menu-frame-live-and-visible-p)) (visual menu-item "Visual Line Numbers" menu-bar--display-line-numbers-mode-visual :help "Enable visual line numbers" :button (:radio eq display-line-numbers 'visual) :visible (menu-bar-menu-frame-live-and-visible-p)) "Line Numbers")) "Show/Hide")) (menu-set-font menu-item "Set Default Font..." menu-set-font :visible (display-multi-font-p) :help "Select a default font") (custom-separator "--") (save menu-item "Save Options" menu-bar-options-save :help "Save options set from the menu above") (package menu-item "Manage Emacs Packages" package-list-packages :help "Install or uninstall additional Emacs packages") (customize menu-item "Customize Emacs" (keymap (customize-themes menu-item "Custom Themes" customize-themes :help "Choose a pre-defined customization theme") (customize menu-item "Top-level Emacs Customization Group" customize :help "Top-level groups of customizable options, and their descriptions") (customize-browse menu-item "Browse Customization Groups" customize-browse :help "Tree-like browser of all the groups of customizable options") (separator-3 "--") (customize-saved menu-item "Saved Options" customize-saved :help "Customize previously saved options") (customize-changed menu-item "New Options..." customize-changed :help "Options and faces added or changed in recent Emacs versions") (separator-2 "--") (customize-option menu-item "Specific Option..." customize-option :help "Customize value of specific option") (customize-face menu-item "Specific Face..." customize-face :help "Customize attributes of specific face") (customize-group menu-item "Specific Group..." customize-group :help "Customize settings of specific group") (separator-1 "--") (customize-apropos menu-item "All Settings Matching..." customize-apropos :help "Browse customizable settings matching a regexp or word list") (customize-apropos-options menu-item "Options Matching..." customize-apropos-options :help "Browse options matching a regexp or word list") (customize-apropos-faces menu-item "Faces Matching..." customize-apropos-faces :help "Browse faces matching a regexp or word list") "Customize")) "Options"))
(define-key m [menu-bar buffer] '("Buffers" keymap "Buffers" [("*scratch*  ") ("*Messages*  *")] (command-separator "--") (next-buffer menu-item "Next Buffer" next-buffer :help "Switch to the \"next\" buffer in a cyclic order") (previous-buffer menu-item "Previous Buffer" previous-buffer :help "Switch to the \"previous\" buffer in a cyclic order") (select-named-buffer menu-item "Select Named Buffer..." switch-to-buffer :help "Prompt for a buffer name, and select that buffer in the current window") (list-all-buffers menu-item "List All Buffers" list-buffers :help "Pop up a window listing all Emacs buffers") (select-buffer-in-project menu-item "Select Buffer In Project..." project-switch-to-buffer :help "Prompt for a buffer belonging to current project, and switch to it") (list-buffers-in-project menu-item "List Buffers In Project..." project-list-buffers :help "Pop up a window listing all Emacs buffers belonging to current project")))
(define-key m [menu-bar tools] '("Tools" keymap (grep menu-item "Search Files (Grep)..." grep :help "Search files for strings or regexps (with Grep)") (rgrep menu-item "Recursive Grep..." rgrep :help "Interactively ask for parameters and search recursively") (shell-commands menu-item "Shell Commands" (keymap (shell menu-item "Shell Command..." shell-command :help "Invoke a shell command and catch its output") (shell-on-region menu-item "Shell Command on Region..." shell-command-on-region :enable mark-active :help "Pass marked region to a shell command") (async-shell-command menu-item "Async Shell Command..." async-shell-command :help "Invoke a shell command asynchronously in background") (interactive-shell menu-item "Run Shell" shell :help "Run a subshell interactively") (project-interactive-shell menu-item "Run Shell In Project" project-shell :help "Run a subshell interactively, in the current project's root directory") "Shell Commands")) (compile menu-item "Compile..." compile :help "Invoke compiler or Make in current buffer's directory, view errors") (project-compile menu-item "Compile Project..." project-compile :help "Invoke compiler or Make for current project, view errors") (gdb menu-item "Debugger (GDB)..." gdb :help "Debug a program from within Emacs with GDB") (project menu-item "Project" (keymap (project-open-file menu-item "Open File..." project-find-file :help "Open an existing file that belongs to current project") (project-or-external-find-file menu-item "Open File Including External Roots..." project-or-external-find-file :help "Open existing file that belongs to current project or its external roots") (project-find-dir menu-item "Open Directory..." project-find-dir :help "Open existing directory that belongs to current project") (project-dired menu-item "Open Project Root" project-dired :help "Read the root directory of the current project, to operate on its files") (project-vc-dir menu-item "VC Dir" project-vc-dir :help "Show the VC status of the project repository") (project-customize-dirlocals menu-item "Customize Directory Local Variables" project-customize-dirlocals :help "Customize current project Directory Local Variables.") (project-switch-project menu-item "Switch Project..." project-switch-project :help "Switch to another project and then run a command") (separator-project-programs "--") (project-compile menu-item "Compile..." project-compile :help "Invoke compiler or Make for current project, view errors") (project-shell menu-item "Run Shell" project-shell :help "Run a subshell interactively, in the current project's root directory") (project-eshell menu-item "Run Eshell" project-eshell :help "Run eshell for the current project") (project-shell-command menu-item "Shell Command..." project-shell-command :help "Invoke a shell command in project root and catch its output") (project-async-shell-command menu-item "Async Shell Command..." project-async-shell-command :help "Invoke a shell command in project root asynchronously in background") (separator-project-buffers "--") (project-switch-to-buffer menu-item "Switch To Buffer..." project-switch-to-buffer :help "Prompt for a buffer belonging to current project, and switch to it") (project-list-buffers menu-item "List Buffers" project-list-buffers :help "Pop up a window listing all Emacs buffers belonging to current project") (project-kill-buffers menu-item "Kill Buffers..." project-kill-buffers :help "Kill the buffers belonging to the current project") (separator-project-search "--") (project-find-regexp menu-item "Find Regexp..." project-find-regexp :help "Search for a regexp in files belonging to current project") (project-or-external-find-regexp menu-item "Find Regexp Including External Roots..." project-or-external-find-regexp :help "Search for a regexp in files belonging to current project or external files") (project-query-replace-regexp menu-item "Query Replace Regexp..." project-query-replace-regexp :help "Interactively replace a regexp in files belonging to current project") (project-execute-extended-command menu-item "Execute Extended Command..." project-execute-extended-command :help "Execute an extended command in project root directory") "Project")) (eglot menu-item "Language Server Support (Eglot)" eglot :help "Start language server suitable for this buffer's major-mode") (ede menu-item "Project Support (EDE)" global-ede-mode :help "Toggle the Emacs Development Environment (Global EDE mode)" :button (:toggle bound-and-true-p global-ede-mode)) (semantic menu-item "Source Code Parsers (Semantic)" semantic-mode :help "Toggle automatic parsing in source code buffers (Semantic mode)" :button (:toggle bound-and-true-p semantic-mode)) (separator-prog "--") (spell menu-item "Spell Checking" ispell-menu-map) (separator-spell "--") (compare menu-item "Compare (Ediff)" menu-bar-ediff-menu) (ediff-merge menu-item "Merge" menu-bar-ediff-merge-menu) (epatch menu-item "Apply Patch" menu-bar-epatch-menu) (separator-compare "--") (vc menu-item "Version Control" vc-menu-map :filter vc-menu-map-filter) (separator-vc "--") (gnus menu-item "Read Net News" gnus :help "Read network news groups") (rmail menu-item "Read Mail" menu-bar-read-mail :visible (and read-mail-command (not (eq read-mail-command 'ignore))) :help "Read your mail") (compose-mail menu-item "Compose New Mail" compose-mail :visible (and mail-user-agent (not (eq mail-user-agent 'ignore))) :help "Start writing a new mail message") (directory-search menu-item "Directory Servers" eudc-tools-menu) (browse-web menu-item "Browse the Web..." browse-web) (separator-net "--") (calendar menu-item "Calendar" calendar :help "Invoke the Emacs built-in calendar") (calc menu-item "Programmable Calculator" calc :help "Invoke the Emacs built-in full scientific calculator") (simple-calculator menu-item "Simple Calculator" calculator :help "Invoke the Emacs built-in quick calculator") (separator-encryption-decryption "--") (encryption-decryption menu-item "Encryption/Decryption" (keymap (decrypt-file menu-item "Decrypt File..." epa-decrypt-file :help "Decrypt a file") (encrypt-file menu-item "Encrypt File..." epa-encrypt-file :help "Encrypt a file") (verify-file menu-item "Verify File..." epa-verify-file :help "Verify digital signature of a file") (sign-file menu-item "Sign File..." epa-sign-file :help "Create digital signature of a file") (separator-file "--") (decrypt-region menu-item "Decrypt Region" epa-decrypt-region :help "Decrypt the current region") (encrypt-region menu-item "Encrypt Region" epa-encrypt-region :help "Encrypt the current region") (verify-region menu-item "Verify Region" epa-verify-region :help "Verify digital signature of the current region") (sign-region menu-item "Sign Region" epa-sign-region :help "Create digital signature of the current region") (separator-keys "--") (list-keys menu-item "List Keys" epa-list-keys :help "Browse your public keyring") (import-keys menu-item "Import Keys from File..." epa-import-keys :help "Import public keys from a file") (import-keys-region menu-item "Import Keys from Region" epa-import-keys-region :help "Import public keys from the current region") (export-keys menu-item "Export Keys" epa-export-keys :help "Export public keys to a file") (insert-keys menu-item "Insert Keys" epa-insert-keys :help "Insert public keys after the current point") "Encryption/Decryption")) (separator-games "--") (games menu-item "Games" (keymap (5x5 menu-item "5x5" 5x5 :help "Fill in all the squares on a 5x5 board") (adventure menu-item "Adventure" dunnet :help "Dunnet, a text Adventure game for Emacs") (black-box menu-item "Blackbox" blackbox :help "Find balls in a black box by shooting rays") (bubbles menu-item "Bubbles" bubbles :help "Remove all bubbles using the fewest moves") (gomoku menu-item "Gomoku" gomoku :help "Mark 5 contiguous squares (like tic-tac-toe)") (hanoi menu-item "Towers of Hanoi" hanoi :help "Watch Towers-of-Hanoi puzzle solved by Emacs") (life menu-item "Life" life :help "Watch how John Conway's cellular automaton evolves") (mult menu-item "Multiplication Puzzle" mpuz :help "Exercise brain with multiplication") (pong menu-item "Pong" pong :help "Bounce the ball to your opponent") (snake menu-item "Snake" snake :help "Move snake around avoiding collisions") (solitaire menu-item "Solitaire" solitaire :help "Get rid of all the stones") (tetris menu-item "Tetris" tetris :help "Falling blocks game") (zone menu-item "Zone Out" zone :help "Play tricks with Emacs display when Emacs is idle") "Games")) "Tools"))
(define-key m [bottom-left-corner mouse-1] 'ignore)
(define-key m [bottom-left-corner down-mouse-1] 'mouse-drag-bottom-left-corner)
(define-key m [bottom-edge mouse-1] 'ignore)
(define-key m [bottom-edge down-mouse-1] 'mouse-drag-bottom-edge)
(define-key m [bottom-right-corner mouse-1] 'ignore)
(define-key m [bottom-right-corner down-mouse-1] 'mouse-drag-bottom-right-corner)
(define-key m [right-edge mouse-1] 'ignore)
(define-key m [right-edge down-mouse-1] 'mouse-drag-right-edge)
(define-key m [top-right-corner mouse-1] 'ignore)
(define-key m [top-right-corner down-mouse-1] 'mouse-drag-top-right-corner)
(define-key m [top-edge mouse-1] 'ignore)
(define-key m [top-edge down-mouse-1] 'mouse-drag-top-edge)
(define-key m [top-left-corner mouse-1] 'ignore)
(define-key m [top-left-corner down-mouse-1] 'mouse-drag-top-left-corner)
(define-key m [left-edge mouse-1] 'ignore)
(define-key m [left-edge down-mouse-1] 'mouse-drag-left-edge)
(define-key m [bottom-divider C-mouse-2] 'mouse-split-window-horizontally)
(define-key m [bottom-divider mouse-1] 'ignore)
(define-key m [bottom-divider down-mouse-1] 'mouse-drag-mode-line)
(define-key m [right-divider C-mouse-2] 'mouse-split-window-vertically)
(define-key m [right-divider mouse-1] 'ignore)
(define-key m [right-divider down-mouse-1] 'mouse-drag-vertical-line)
(define-key m [vertical-line C-mouse-2] 'mouse-split-window-vertically)
(define-key m [vertical-line mouse-1] 'mouse-select-window)
(define-key m [vertical-line down-mouse-1] 'mouse-drag-vertical-line)
(define-key m [horizontal-scroll-bar wheel-right] 'mwheel-scroll)
(define-key m [horizontal-scroll-bar wheel-left] 'mwheel-scroll)
(define-key m [horizontal-scroll-bar wheel-up] 'mwheel-scroll)
(define-key m [horizontal-scroll-bar wheel-down] 'mwheel-scroll)
(define-key m [horizontal-scroll-bar mouse-7] 'mwheel-scroll)
(define-key m [horizontal-scroll-bar mouse-6] 'mwheel-scroll)
(define-key m [horizontal-scroll-bar mouse-5] 'mwheel-scroll)
(define-key m [horizontal-scroll-bar mouse-4] 'mwheel-scroll)
(define-key m [horizontal-scroll-bar drag-mouse-1] 'nil)
(define-key m [horizontal-scroll-bar down-mouse-1] 'scroll-bar-toolkit-horizontal-scroll)
(define-key m [horizontal-scroll-bar mouse-1] 'nil)
(define-key m [horizontal-scroll-bar C-mouse-2] 'mouse-split-window-horizontally)
(define-key m [vertical-scroll-bar wheel-right] 'mwheel-scroll)
(define-key m [vertical-scroll-bar wheel-left] 'mwheel-scroll)
(define-key m [vertical-scroll-bar wheel-up] 'mwheel-scroll)
(define-key m [vertical-scroll-bar wheel-down] 'mwheel-scroll)
(define-key m [vertical-scroll-bar mouse-7] 'mwheel-scroll)
(define-key m [vertical-scroll-bar mouse-6] 'mwheel-scroll)
(define-key m [vertical-scroll-bar mouse-5] 'mwheel-scroll)
(define-key m [vertical-scroll-bar mouse-4] 'mwheel-scroll)
(define-key m [vertical-scroll-bar drag-mouse-1] 'nil)
(define-key m [vertical-scroll-bar down-mouse-1] 'scroll-bar-toolkit-scroll)
(define-key m [vertical-scroll-bar mouse-1] 'nil)
(define-key m [vertical-scroll-bar C-mouse-2] 'mouse-split-window-vertically)
(define-key m [mode-line wheel-right] 'mwheel-scroll)
(define-key m [mode-line wheel-left] 'mwheel-scroll)
(define-key m [mode-line wheel-up] 'mwheel-scroll)
(define-key m [mode-line wheel-down] 'mwheel-scroll)
(define-key m [mode-line mouse-7] 'mwheel-scroll)
(define-key m [mode-line mouse-6] 'mwheel-scroll)
(define-key m [mode-line mouse-5] 'mwheel-scroll)
(define-key m [mode-line mouse-4] 'mwheel-scroll)
(define-key m [mode-line C-mouse-2] 'mouse-split-window-horizontally)
(define-key m [mode-line mouse-3] 'mouse-delete-window)
(define-key m [mode-line mouse-2] 'mouse-delete-other-windows)
(define-key m [mode-line mouse-1] 'mouse-select-window)
(define-key m [mode-line down-mouse-1] 'mouse-drag-mode-line)
(define-key m [tab-line mouse-1] 'mouse-select-window)
(define-key m [tab-line down-mouse-1] 'mouse-drag-tab-line)
(define-key m [header-line wheel-right] 'mwheel-scroll)
(define-key m [header-line wheel-left] 'mwheel-scroll)
(define-key m [header-line wheel-up] 'mwheel-scroll)
(define-key m [header-line wheel-down] 'mwheel-scroll)
(define-key m [header-line mouse-7] 'mwheel-scroll)
(define-key m [header-line mouse-6] 'mwheel-scroll)
(define-key m [header-line mouse-5] 'mwheel-scroll)
(define-key m [header-line mouse-4] 'mwheel-scroll)
(define-key m [header-line mouse-1] 'mouse-select-window)
(define-key m [header-line down-mouse-1] 'mouse-drag-header-line)
(define-key m [C-down-mouse-3] '(menu-item "Menu Bar" ignore :filter nil))
(define-key m [S-down-mouse-1] 'nil)
(define-key m [C-down-mouse-1] 'mouse-buffer-menu)
(define-key m [mouse-3] 'mouse-save-then-kill)
(define-key m [mouse-2] 'mouse-yank-primary)
(define-key m [drag-mouse-1] 'mouse-set-region)
(define-key m [mouse-1] 'mouse-set-point)
(define-key m [down-mouse-1] 'mouse-drag-region)
(define-key m [C-M-mouse-1] 'mouse-set-point)
(define-key m [C-M-drag-mouse-1] 'ignore)
(define-key m [C-M-down-mouse-1] 'mouse-drag-region-rectangle)
(define-key m [M-mouse-2] 'mouse-yank-secondary)
(define-key m [M-mouse-3] 'mouse-secondary-save-then-kill)
(define-key m [M-down-mouse-1] 'mouse-drag-secondary)
(define-key m [M-drag-mouse-1] 'mouse-set-secondary)
(define-key m [M-mouse-1] 'mouse-start-secondary)
(define-key m [S-f10] 'context-menu-open)
(define-key m [M-f10] 'toggle-frame-maximized)
(define-key m [f11] 'toggle-frame-fullscreen)
(define-key m [compose-last-chars] 'compose-last-chars)
(define-key m [f1] 'help-command)
(define-key m [help] 'help-command)
(define-key m [f2] '2C-command)
(define-key m [f4] 'kmacro-end-or-call-macro)
(define-key m [f3] 'kmacro-start-macro-or-insert-counter)
(define-key m [C-down-mouse-2] 'facemenu-menu)
(define-key m [pinch] 'text-scale-pinch)
(define-key m [text-conversion] 'analyze-text-conversion)
(define-key m [C-M-backspace] 'backward-kill-sexp)
(define-key m [C-M-delete] 'backward-kill-sexp)
(define-key m [C-M-end] 'end-of-defun)
(define-key m [C-M-home] 'beginning-of-defun)
(define-key m [C-M-down] 'down-list)
(define-key m [C-M-up] 'backward-up-list)
(define-key m [C-M-right] 'forward-sexp)
(define-key m [C-M-left] 'backward-sexp)
(define-key m [S-delete] 'kill-region)
(define-key m [C-backspace] 'backward-kill-word)
(define-key m [C-delete] 'kill-word)
(define-key m [C-left] 'left-word)
(define-key m [C-right] 'right-word)
(define-key m [M-left] 'left-word)
(define-key m [M-right] 'right-word)
(define-key m [mouse-movement] 'ignore-preserving-kill-region)
(define-key m [touch-end] 'ignore)
(define-key m [deletechar] 'delete-forward-char)
(define-key m [deleteline] 'kill-line)
(define-key m [insertline] 'open-line)
(define-key m [open] 'find-file)
(define-key m [again] 'repeat-complex-command)
(define-key m [redo] 'repeat-complex-command)
(define-key m [undo] 'undo)
(define-key m [cut] 'kill-region)
(define-key m [paste] 'yank)
(define-key m [copy] 'kill-ring-save)
(define-key m [S-insertchar] 'yank)
(define-key m [C-insertchar] 'kill-ring-save)
(define-key m [insertchar] 'overwrite-mode)
(define-key m [S-insert] 'yank)
(define-key m [C-insert] 'kill-ring-save)
(define-key m [insert] 'overwrite-mode)
(define-key m [execute] 'execute-extended-command)
(define-key m [M-begin] 'beginning-of-buffer-other-window)
(define-key m [begin] 'beginning-of-buffer)
(define-key m [M-end] 'end-of-buffer-other-window)
(define-key m [C-end] 'end-of-buffer)
(define-key m [end] 'end-of-buffer)
(define-key m [M-prior] 'scroll-other-window-down)
(define-key m [M-next] 'scroll-other-window)
(define-key m [C-next] 'scroll-left)
(define-key m [C-prior] 'scroll-right)
(define-key m [C-down] 'forward-paragraph)
(define-key m [C-up] 'backward-paragraph)
(define-key m [next] 'scroll-up-command)
(define-key m [prior] 'scroll-down-command)
(define-key m [down] 'next-line)
(define-key m [right] 'right-char)
(define-key m [up] 'previous-line)
(define-key m [left] 'left-char)
(define-key m [M-home] 'beginning-of-buffer-other-window)
(define-key m [C-home] 'beginning-of-buffer)
(define-key m [home] 'beginning-of-buffer)
(define-key m [C-S-backspace] 'kill-whole-line)
(define-key m [Scroll_Lock] 'scroll-lock-mode)
(define-key m [find] 'search-forward)
(define-key m [menu] 'execute-extended-command)
(define-key m [67108896] 'set-mark-command)
(define-key m [67108909] 'negative-argument)
(define-key m [67108921] 'digit-argument)
(define-key m [67108920] 'digit-argument)
(define-key m [67108919] 'digit-argument)
(define-key m [67108918] 'digit-argument)
(define-key m [67108917] 'digit-argument)
(define-key m [67108916] 'digit-argument)
(define-key m [67108915] 'digit-argument)
(define-key m [67108914] 'digit-argument)
(define-key m [67108913] 'digit-argument)
(define-key m [67108912] 'digit-argument)
(define-key m [XF86Back] 'previous-buffer)
(define-key m [XF86Forward] 'next-buffer)
(define-key m [67108927] 'undo-redo)
(define-key m [67108911] 'undo)
(define-key m [make-frame-visible] 'ignore)
(define-key m [iconify-frame] 'ignore)
(define-key m [delete-frame] 'handle-delete-frame)
(define-key m [select-window] 'handle-select-window)
(define-key m [switch-frame] 'handle-switch-frame)
)

(fset 'mode-specific-command-prefix
  (let ((s (make-sparse-keymap)))
  (define-key s [94] (let ((s (make-sparse-keymap)))

  s))
  s))
(fset 'help-command
  (let ((s (make-sparse-keymap)))
  (define-key s [113] 'help-quit)
  (define-key s [120] 'describe-command)
  (define-key s [119] 'where-is)
  (define-key s [118] 'describe-variable)
  (define-key s [116] 'help-with-tutorial)
  (define-key s [115] 'describe-syntax)
  (define-key s [82] 'info-display-manual)
  (define-key s [114] 'info-emacs-manual)
  (define-key s [80] 'describe-package)
  (define-key s [112] 'finder-by-keyword)
  (define-key s [110] 'view-emacs-news)
  (define-key s [111] 'describe-symbol)
  (define-key s [109] 'describe-mode)
  (define-key s [108] 'view-lossage)
  (define-key s [107] 'describe-key)
  (define-key s [52] (let ((s (make-sparse-keymap)))
  (define-key s [115] 'help-find-source)
  (define-key s [105] 'info-other-window)
  s))
  (define-key s [105] 'info)
  (define-key s [117] 'apropos-user-option)
  (define-key s [104] 'view-hello-file)
  (define-key s [103] 'describe-gnu-project)
  (define-key s [102] 'describe-function)
  (define-key s [101] 'view-echo-area-messages)
  (define-key s [100] 'apropos-documentation)
  (define-key s [99] 'describe-key-briefly)
  (define-key s [98] 'describe-bindings)
  (define-key s [97] 'apropos-command)
  (define-key s [83] 'info-lookup-symbol)
  (define-key s [76] 'describe-language-environment)
  (define-key s [75] 'Info-goto-emacs-key-command-node)
  (define-key s [73] 'describe-input-method)
  (define-key s [70] 'Info-goto-emacs-command-node)
  (define-key s [67] 'describe-coding-system)
  (define-key s [28] 'describe-input-method)
  (define-key s [23] 'describe-no-warranty)
  (define-key s [20] 'view-emacs-todo)
  (define-key s [19] 'search-forward-help-for-help)
  (define-key s [17] 'help-quick-toggle)
  (define-key s [16] 'view-emacs-problems)
  (define-key s [15] 'describe-distribution)
  (define-key s [14] 'view-emacs-news)
  (define-key s [13] 'view-order-manuals)
  (define-key s [6] 'view-emacs-FAQ)
  (define-key s [5] 'view-external-packages)
  (define-key s [4] 'view-emacs-debugging)
  (define-key s [3] 'describe-copying)
  (define-key s [1] 'about-emacs)
  (define-key s [63] 'help-for-help)
  (define-key s [46] 'display-local-help)
  (define-key s [f1] 'help-for-help)
  (define-key s [help] 'help-for-help)
  (define-key s [8] 'help-for-help)
  s))
(fset 'kmacro-keymap
  '(autoload "kmacro" "Keymap for keyboard macro commands." t keymap))
(fset 'ctl-x-4-prefix
  (let ((s (make-sparse-keymap)))
  (define-key s [46] 'xref-find-definitions-other-window)
  (define-key s [112] 'project-other-window-command)
  (define-key s [100] 'dired-other-window)
  (define-key s [15] 'display-buffer)
  (define-key s [98] 'switch-to-buffer-other-window)
  (define-key s [6] 'find-file-other-window)
  (define-key s [114] 'find-file-read-only-other-window)
  (define-key s [102] 'find-file-other-window)
  (define-key s [52] 'other-window-prefix)
  (define-key s [49] 'same-window-prefix)
  (define-key s [48] 'kill-buffer-and-window)
  (define-key s [99] 'clone-indirect-buffer-other-window)
  (define-key s [97] 'add-change-log-entry-other-window)
  (define-key s [10] 'dired-jump-other-window)
  (define-key s [109] 'compose-mail-other-window)
  s))
(fset 'ctl-x-5-prefix
  (let ((s (make-sparse-keymap)))
  (define-key s [117] 'undelete-frame)
  (define-key s [99] 'clone-frame)
  (define-key s [53] 'other-frame-prefix)
  (define-key s [111] 'other-frame)
  (define-key s [48] 'delete-frame)
  (define-key s [49] 'delete-other-frames)
  (define-key s [50] 'make-frame-command)
  (define-key s [46] 'xref-find-definitions-other-frame)
  (define-key s [112] 'project-other-frame-command)
  (define-key s [100] 'dired-other-frame)
  (define-key s [15] 'display-buffer-other-frame)
  (define-key s [114] 'find-file-read-only-other-frame)
  (define-key s [6] 'find-file-other-frame)
  (define-key s [102] 'find-file-other-frame)
  (define-key s [98] 'switch-to-buffer-other-frame)
  (define-key s [109] 'compose-mail-other-frame)
  s))
(fset '2C-command
  '(keymap (115 . 2C-split) (98 . 2C-associate-buffer)
           (f2 . 2C-two-columns) (50 . 2C-two-columns)))
(fset 'vc-prefix-map
  (let ((s (make-sparse-keymap)))
  (define-key s [119] (let ((s (make-sparse-keymap)))
  (define-key s [65] 'vc-apply-root-to-other-working-tree)
  (define-key s [97] 'vc-apply-to-other-working-tree)
  (define-key s [82] 'vc-move-working-tree)
  (define-key s [120] 'vc-delete-working-tree)
  (define-key s [115] 'vc-working-tree-switch-project)
  (define-key s [107] 'vc-kill-other-working-tree-buffers)
  (define-key s [119] 'vc-switch-working-tree)
  (define-key s [99] 'vc-add-working-tree)
  s))
  (define-key s [33] 'vc-edit-next-command)
  (define-key s [120] 'vc-delete-file)
  (define-key s [82] 'vc-rename-file)
  (define-key s [126] 'vc-revision-other-window)
  (define-key s [68] 'vc-root-diff)
  (define-key s [61] 'vc-diff)
  (define-key s [80] 'vc-push)
  (define-key s [43] 'vc-update)
  (define-key s [118] 'vc-next-action)
  (define-key s [64] 'vc-revert)
  (define-key s [117] 'vc-revert)
  (define-key s [115] 'vc-create-tag)
  (define-key s [114] 'vc-retrieve-tag)
  (define-key s [109] 'vc-merge)
  (define-key s [69] (let ((s (make-sparse-keymap)))
  (define-key s [68] 'vc-root-diff-outgoing-and-edited)
  (define-key s [61] 'vc-diff-outgoing-and-edited)
  (define-key s [76] 'vc-root-log-outgoing)
  s))
  (define-key s [84] (let ((s (make-sparse-keymap)))
  (define-key s [82] (let ((s (make-sparse-keymap)))
  (define-key s [68] 'vc-root-diff-remote-unintegrated)
  (define-key s [61] 'vc-diff-remote-unintegrated)
  (define-key s [76] 'vc-root-log-remote-unintegrated)
  (define-key s [108] 'vc-log-remote-unintegrated)
  s))
  (define-key s [68] 'vc-root-diff-unintegrated)
  (define-key s [61] 'vc-diff-unintegrated)
  (define-key s [76] 'vc-root-log-unintegrated)
  (define-key s [108] 'vc-log-unintegrated)
  s))
  (define-key s [77] (let ((s (make-sparse-keymap)))
  (define-key s [68] 'vc-diff-mergebase)
  (define-key s [76] 'vc-log-mergebase)
  s))
  (define-key s [79] 'vc-root-log-outgoing)
  (define-key s [73] 'vc-root-log-incoming)
  (define-key s [76] 'vc-print-root-log)
  (define-key s [108] 'vc-print-log)
  (define-key s [105] 'vc-register)
  (define-key s [104] 'vc-region-history)
  (define-key s [71] 'vc-ignore)
  (define-key s [103] 'vc-annotate)
  (define-key s [100] 'vc-dir)
  (define-key s [98] (let ((s (make-sparse-keymap)))
  (define-key s [115] 'vc-switch-branch)
  (define-key s [76] 'vc-print-root-branch-log)
  (define-key s [108] 'vc-print-fileset-branch-log)
  (define-key s [99] 'vc-create-branch)
  s))
  (define-key s [97] 'vc-update-change-log)
  s))
(fset 'Control-X-prefix
  (let ((s (make-keymap)))
  (define-key s [0] 'pop-global-mark)
  (define-key s [2] 'list-buffers)
  (define-key s [3] 'save-buffers-kill-terminal)
  (define-key s [4] 'list-directory)
  (define-key s [5] 'eval-last-sexp)
  (define-key s [6] 'find-file)
  (define-key s [9] 'indent-rigidly)
  (define-key s [10] 'dired-jump)
  (define-key s [11] 'kmacro-keymap)
  (define-key s [12] 'downcase-region)
  (define-key s [13] (let ((s (make-sparse-keymap)))
  (define-key s [108] 'set-language-environment)
  (define-key s [99] 'universal-coding-system-argument)
  (define-key s [28] 'set-input-method)
  (define-key s [88] 'set-next-selection-coding-system)
  (define-key s [120] 'set-selection-coding-system)
  (define-key s [112] 'set-buffer-process-coding-system)
  (define-key s [107] 'set-keyboard-coding-system)
  (define-key s [116] 'set-terminal-coding-system)
  (define-key s [70] 'set-file-name-coding-system)
  (define-key s [114] 'revert-buffer-with-coding-system)
  (define-key s [102] 'set-buffer-file-coding-system)
  s))
  (define-key s [14] 'set-goal-column)
  (define-key s [15] 'delete-blank-lines)
  (define-key s [16] 'mark-page)
  (define-key s [17] 'read-only-mode)
  (define-key s [18] 'find-file-read-only)
  (define-key s [19] 'save-buffer)
  (define-key s [20] 'transpose-lines)
  (define-key s [21] 'upcase-region)
  (define-key s [22] 'find-alternate-file)
  (define-key s [23] 'write-file)
  (define-key s [24] 'exchange-point-and-mark)
  (define-key s [26] 'suspend-frame)
  (define-key s [27] (let ((s (make-sparse-keymap)))
  (define-key s [67108912] 'global-text-scale-adjust)
  (define-key s [67108909] 'global-text-scale-adjust)
  (define-key s [67108925] 'global-text-scale-adjust)
  (define-key s [67108907] 'global-text-scale-adjust)
  (define-key s [58] 'repeat-complex-command)
  (define-key s [27] 'repeat-complex-command)
  s))
  (define-key s [32] 'rectangle-mark-mode)
  (define-key s [36] 'set-selective-display)
  (define-key s [39] 'expand-abbrev)
  (define-key s [40] 'kmacro-start-macro)
  (define-key s [41] 'kmacro-end-macro)
  (define-key s [42] 'calc-dispatch)
  (define-key s [43] 'balance-windows)
  (define-key s [45] 'shrink-window-if-larger-than-buffer)
  (define-key s [46] 'set-fill-prefix)
  (define-key s [48] 'delete-window)
  (define-key s [49] 'delete-other-windows)
  (define-key s [50] 'split-window-below)
  (define-key s [51] 'split-window-right)
  (define-key s [52] 'ctl-x-4-prefix)
  (define-key s [53] 'ctl-x-5-prefix)
  (define-key s [54] '2C-command)
  (define-key s [56] (let ((s (make-sparse-keymap)))
  (define-key s [101] (let ((s (make-sparse-keymap)))
  (define-key s [48] 'emoji-zoom-reset)
  (define-key s [45] 'emoji-zoom-decrease)
  (define-key s [43] 'emoji-zoom-increase)
  (define-key s [108] 'emoji-list)
  (define-key s [114] 'emoji-recent)
  (define-key s [100] 'emoji-describe)
  (define-key s [115] 'emoji-search)
  (define-key s [105] 'emoji-insert)
  (define-key s [101] 'emoji-insert)
  s))
  (define-key s [13] 'insert-char)
  s))
  (define-key s [59] 'comment-set-column)
  (define-key s [60] 'scroll-left)
  (define-key s [61] 'what-cursor-position)
  (define-key s [62] 'scroll-right)
  (define-key s [79] 'other-window-backward)
  (define-key s [91] 'backward-page)
  (define-key s [92] 'activate-transient-input-method)
  (define-key s [93] 'forward-page)
  (define-key s [94] 'enlarge-window)
  (define-key s [96] 'next-error)
  (define-key s [97] (let ((s (make-sparse-keymap)))
  (define-key s [110] 'expand-jump-to-next-slot)
  (define-key s [112] 'expand-jump-to-previous-slot)
  (define-key s [39] 'expand-abbrev)
  (define-key s [101] 'expand-abbrev)
  (define-key s [45] 'inverse-add-global-abbrev)
  (define-key s [105] (let ((s (make-sparse-keymap)))
  (define-key s [108] 'inverse-add-mode-abbrev)
  (define-key s [103] 'inverse-add-global-abbrev)
  s))
  (define-key s [43] 'add-mode-abbrev)
  (define-key s [103] 'add-global-abbrev)
  (define-key s [1] 'add-mode-abbrev)
  (define-key s [108] 'add-mode-abbrev)
  s))
  (define-key s [98] 'switch-to-buffer)
  (define-key s [100] 'dired)
  (define-key s [101] 'kmacro-end-and-call-macro)
  (define-key s [102] 'set-fill-column)
  (define-key s [104] 'mark-whole-buffer)
  (define-key s [105] 'insert-file)
  (define-key s [107] 'kill-buffer)
  (define-key s [108] 'count-lines-page)
  (define-key s [109] 'compose-mail)
  (define-key s [110] (let ((s (make-sparse-keymap)))
  (define-key s [112] 'narrow-to-page)
  (define-key s [100] 'narrow-to-defun)
  (define-key s [103] 'goto-line-relative)
  (define-key s [119] 'widen)
  (define-key s [110] 'narrow-to-region)
  s))
  (define-key s [111] 'other-window)
  (define-key s [112] (let ((s (make-sparse-keymap)))
  (define-key s [24] (let ((s (make-sparse-keymap)))
  (define-key s [115] 'project-save-some-buffers)
  s))
  (define-key s [2] 'project-list-buffers)
  (define-key s [111] 'project-any-command)
  (define-key s [120] 'project-execute-extended-command)
  (define-key s [114] 'project-query-replace-regexp)
  (define-key s [71] 'project-or-external-find-regexp)
  (define-key s [103] 'project-find-regexp)
  (define-key s [112] 'project-switch-project)
  (define-key s [107] 'project-kill-buffers)
  (define-key s [101] 'project-eshell)
  (define-key s [99] 'project-compile)
  (define-key s [118] 'project-vc-dir)
  (define-key s [68] 'project-dired)
  (define-key s [100] 'project-find-dir)
  (define-key s [115] 'project-shell)
  (define-key s [98] 'project-switch-to-buffer)
  (define-key s [70] 'project-or-external-find-file)
  (define-key s [102] 'project-find-file)
  (define-key s [38] 'project-async-shell-command)
  (define-key s [33] 'project-shell-command)
  s))
  (define-key s [113] 'kbd-macro-query)
  (define-key s [114] (let ((s (make-sparse-keymap)))
  (define-key s [108] 'bookmark-bmenu-list)
  (define-key s [77] 'bookmark-set-no-overwrite)
  (define-key s [109] 'bookmark-set)
  (define-key s [98] 'bookmark-jump)
  (define-key s [66] 'buffer-to-register)
  (define-key s [70] 'file-to-register)
  (define-key s [102] 'frameset-to-register)
  (define-key s [119] 'window-configuration-to-register)
  (define-key s [43] 'increment-register)
  (define-key s [110] 'number-to-register)
  (define-key s [114] 'copy-rectangle-to-register)
  (define-key s [103] 'insert-register)
  (define-key s [105] 'insert-register)
  (define-key s [120] 'copy-to-register)
  (define-key s [115] 'copy-to-register)
  (define-key s [106] 'jump-to-register)
  (define-key s [32] 'point-to-register)
  (define-key s [67108896] 'point-to-register)
  (define-key s [0] 'point-to-register)
  (define-key s [27] (let ((s (make-sparse-keymap)))
  (define-key s [119] 'copy-rectangle-as-kill)
  s))
  (define-key s [78] 'rectangle-number-lines)
  (define-key s [116] 'string-rectangle)
  (define-key s [111] 'open-rectangle)
  (define-key s [121] 'yank-rectangle)
  (define-key s [100] 'delete-rectangle)
  (define-key s [107] 'kill-rectangle)
  (define-key s [99] 'clear-rectangle)
  s))
  (define-key s [115] 'save-some-buffers)
  (define-key s [116] (let ((s (make-sparse-keymap)))
  (define-key s [116] 'other-tab-prefix)
  (define-key s [18] 'find-file-read-only-other-tab)
  (define-key s [6] 'find-file-other-tab)
  (define-key s [102] 'find-file-other-tab)
  (define-key s [98] 'switch-to-buffer-other-tab)
  (define-key s [13] 'tab-switch)
  (define-key s [94] (let ((s (make-sparse-keymap)))
  (define-key s [102] 'tab-detach)
  s))
  (define-key s [114] 'tab-rename)
  (define-key s [71] 'tab-group)
  (define-key s [77] 'tab-move-to)
  (define-key s [109] 'tab-move)
  (define-key s [79] 'tab-previous)
  (define-key s [111] 'tab-next)
  (define-key s [117] 'tab-undo)
  (define-key s [48] 'tab-close)
  (define-key s [49] 'tab-close-other)
  (define-key s [50] 'tab-new)
  (define-key s [78] 'tab-new-to)
  (define-key s [110] 'tab-duplicate)
  (define-key s [112] 'project-other-tab-command)
  (define-key s [100] 'dired-other-tab)
  s))
  (define-key s [117] 'undo)
  (define-key s [118] 'vc-prefix-map)
  (define-key s [119] (let ((s (make-sparse-keymap)))
  (define-key s [102] (let ((s (make-sparse-keymap)))
  (define-key s [down] 'window-layout-flip-topdown)
  (define-key s [up] 'window-layout-flip-topdown)
  (define-key s [right] 'window-layout-flip-leftright)
  (define-key s [left] 'window-layout-flip-leftright)
  s))
  (define-key s [114] (let ((s (make-sparse-keymap)))
  (define-key s [right] 'window-layout-rotate-clockwise)
  (define-key s [left] 'window-layout-rotate-anticlockwise)
  s))
  (define-key s [116] 'window-layout-transpose)
  (define-key s [111] (let ((s (make-sparse-keymap)))
  (define-key s [right] 'rotate-windows)
  (define-key s [left] 'rotate-windows-back)
  s))
  (define-key s [113] 'quit-window)
  (define-key s [48] 'delete-windows-on)
  (define-key s [45] 'fit-window-to-buffer)
  (define-key s [94] (let ((s (make-sparse-keymap)))
  (define-key s [116] 'tab-window-detach)
  (define-key s [102] 'tear-off-window)
  s))
  (define-key s [100] 'toggle-window-dedicated)
  (define-key s [115] 'window-toggle-side-windows)
  (define-key s [51] 'split-root-window-right)
  (define-key s [50] 'split-root-window-below)
  s))
  (define-key s [120] (let ((s (make-sparse-keymap)))
  (define-key s [64] 'tramp-revert-buffer-with-sudo)
  (define-key s [116] 'toggle-truncate-lines)
  (define-key s [105] 'insert-buffer)
  (define-key s [110] 'clone-buffer)
  (define-key s [117] 'rename-uniquely)
  (define-key s [114] 'rename-buffer)
  (define-key s [103] 'revert-buffer-quick)
  (define-key s [102] 'font-lock-update)
  s))
  (define-key s [122] 'repeat)
  (define-key s [123] 'shrink-window-horizontally)
  (define-key s [125] 'enlarge-window-horizontally)
  (define-key s [127] 'backward-kill-sentence)
  (define-key s [67108912] 'text-scale-adjust)
  (define-key s [67108925] 'text-scale-adjust)
  (define-key s [67108909] 'text-scale-adjust)
  (define-key s [67108907] 'text-scale-adjust)
  (define-key s [67108923] 'comment-line)
  (define-key s [67108896] 'pop-global-mark)
  (define-key s [C-left] 'previous-buffer)
  (define-key s [left] 'previous-buffer)
  (define-key s [C-right] 'next-buffer)
  (define-key s [right] 'next-buffer)
  s))
(fset 'ESC-prefix
  (let ((s (make-keymap)))
  (define-key s [0] 'mark-sexp)
  (define-key s [1] 'beginning-of-defun)
  (define-key s [2] 'backward-sexp)
  (define-key s [3] 'exit-recursive-edit)
  (define-key s [4] 'down-list)
  (define-key s [5] 'end-of-defun)
  (define-key s [6] 'forward-sexp)
  (define-key s [8] 'mark-defun)
  (define-key s [9] 'complete-symbol)
  (define-key s [10] 'default-indent-new-line)
  (define-key s [11] 'kill-sexp)
  (define-key s [12] 'reposition-window)
  (define-key s [14] 'forward-list)
  (define-key s [15] 'split-line)
  (define-key s [16] 'backward-list)
  (define-key s [18] 'isearch-backward-regexp)
  (define-key s [19] 'isearch-forward-regexp)
  (define-key s [20] 'transpose-sexps)
  (define-key s [21] 'backward-up-list)
  (define-key s [22] 'scroll-other-window)
  (define-key s [23] 'append-next-kill)
  (define-key s [27] (let ((s (make-sparse-keymap)))
  (define-key s [58] 'eval-expression)
  (define-key s [27] 'keyboard-escape-quit)
  s))
  (define-key s [28] 'indent-region)
  (define-key s [31] 'undo-redo)
  (define-key s [32] 'cycle-spacing)
  (define-key s [33] 'shell-command)
  (define-key s [36] 'ispell-word)
  (define-key s [37] 'query-replace)
  (define-key s [38] 'async-shell-command)
  (define-key s [39] 'abbrev-prefix-mark)
  (define-key s [40] 'insert-parentheses)
  (define-key s [41] 'move-past-close-and-reindent)
  (define-key s [44] 'xref-go-back)
  (define-key s [45] 'negative-argument)
  (define-key s [46] 'xref-find-definitions)
  (define-key s [47] 'dabbrev-expand)
  (define-key s [(48 . 57)] 'digit-argument)
  (define-key s [58] 'eval-expression)
  (define-key s [59] 'comment-dwim)
  (define-key s [60] 'beginning-of-buffer)
  (define-key s [61] 'count-words-region)
  (define-key s [62] 'end-of-buffer)
  (define-key s [63] 'xref-find-references)
  (define-key s [64] 'mark-word)
  (define-key s [88] 'execute-extended-command-for-buffer)
  (define-key s [92] 'delete-horizontal-space)
  (define-key s [94] 'delete-indentation)
  (define-key s [96] 'tmm-menubar)
  (define-key s [97] 'backward-sentence)
  (define-key s [98] 'backward-word)
  (define-key s [99] 'capitalize-word)
  (define-key s [100] 'kill-word)
  (define-key s [101] 'forward-sentence)
  (define-key s [102] 'forward-word)
  (define-key s [103] (let ((s (make-sparse-keymap)))
  (define-key s [105] 'imenu)
  (define-key s [9] 'move-to-column)
  (define-key s [112] 'previous-error)
  (define-key s [110] 'next-error)
  (define-key s [27] (let ((s (make-sparse-keymap)))
  (define-key s [112] 'previous-error)
  (define-key s [110] 'next-error)
  (define-key s [103] 'goto-line)
  s))
  (define-key s [103] 'goto-line)
  (define-key s [99] 'goto-char)
  s))
  (define-key s [104] 'mark-paragraph)
  (define-key s [105] 'tab-to-tab-stop)
  (define-key s [106] 'default-indent-new-line)
  (define-key s [107] 'kill-sentence)
  (define-key s [108] 'downcase-word)
  (define-key s [109] 'back-to-indentation)
  (define-key s [113] 'fill-paragraph)
  (define-key s [114] 'move-to-window-line-top-bottom)
  (define-key s [115] (let ((s (make-sparse-keymap)))
  (define-key s [46] 'isearch-forward-symbol-at-point)
  (define-key s [95] 'isearch-forward-symbol)
  (define-key s [119] 'isearch-forward-word)
  (define-key s [104] (let ((s (make-sparse-keymap)))
  (define-key s [119] 'hi-lock-write-interactive-patterns)
  (define-key s [102] 'hi-lock-find-patterns)
  (define-key s [117] 'unhighlight-regexp)
  (define-key s [46] 'highlight-symbol-at-point)
  (define-key s [108] 'highlight-lines-matching-regexp)
  (define-key s [112] 'highlight-phrase)
  (define-key s [114] 'highlight-regexp)
  s))
  (define-key s [27] (let ((s (make-sparse-keymap)))
  (define-key s [46] 'isearch-forward-thing-at-point)
  (define-key s [119] 'eww-search-words)
  s))
  (define-key s [111] 'occur)
  s))
  (define-key s [116] 'transpose-words)
  (define-key s [117] 'upcase-word)
  (define-key s [118] 'scroll-down-command)
  (define-key s [119] 'kill-ring-save)
  (define-key s [120] 'execute-extended-command)
  (define-key s [121] 'yank-pop)
  (define-key s [122] 'zap-to-char)
  (define-key s [123] 'backward-paragraph)
  (define-key s [124] 'shell-command-on-region)
  (define-key s [125] 'forward-paragraph)
  (define-key s [126] 'not-modified)
  (define-key s [127] 'backward-kill-word)
  (define-key s [8388712] 'ns-do-hide-others)
  (define-key s [8388678] 'isearch-backward-regexp)
  (define-key s [8388710] 'isearch-forward-regexp)
  (define-key s [67108901] 'query-replace-regexp)
  (define-key s [f10] 'toggle-frame-maximized)
  (define-key s [67108910] 'xref-find-apropos)
  (define-key s [67108908] 'xref-go-forward)
  (define-key s [67108911] 'dabbrev-completion)
  (define-key s [33554444] 'recenter-other-window)
  (define-key s [C-backspace] 'backward-kill-sexp)
  (define-key s [C-delete] 'backward-kill-sexp)
  (define-key s [67108896] 'mark-sexp)
  (define-key s [C-end] 'end-of-defun)
  (define-key s [C-home] 'beginning-of-defun)
  (define-key s [C-down] 'down-list)
  (define-key s [C-up] 'backward-up-list)
  (define-key s [C-right] 'forward-sexp)
  (define-key s [C-left] 'backward-sexp)
  (define-key s [left] 'backward-word)
  (define-key s [right] 'forward-word)
  (define-key s [begin] 'beginning-of-buffer-other-window)
  (define-key s [end] 'end-of-buffer-other-window)
  (define-key s [33554454] 'scroll-other-window-down)
  (define-key s [prior] 'scroll-other-window-down)
  (define-key s [next] 'scroll-other-window)
  (define-key s [home] 'beginning-of-buffer-other-window)
  (define-key s [67108909] 'negative-argument)
  (define-key s [67108921] 'digit-argument)
  (define-key s [67108920] 'digit-argument)
  (define-key s [67108919] 'digit-argument)
  (define-key s [67108918] 'digit-argument)
  (define-key s [67108917] 'digit-argument)
  (define-key s [67108916] 'digit-argument)
  (define-key s [67108915] 'digit-argument)
  (define-key s [67108914] 'digit-argument)
  (define-key s [67108913] 'digit-argument)
  (define-key s [67108912] 'digit-argument)
  s))
(fset 'facemenu-menu
  '(autoload "facemenu" nil nil keymap))
;; Menu-item defs whose symbols carry keymaps (yank-menu et al).
(fset 'yank-menu
  '(keymap "Select Yank" nil))

(fset 'menu-bar-bookmark-map
  '(keymap (jump menu-item "Jump to Bookmark..." bookmark-jump :help "Jump to a bookmark (a point in some file)") (set menu-item "Set Bookmark..." bookmark-set :help "Set a bookmark named inside a file.") (insert menu-item "Insert Contents..." bookmark-insert :help "Insert the text of the file pointed to by a bookmark") (locate menu-item "Insert Location..." bookmark-locate :help "Insert the name of the file associated with a bookmark") (rename menu-item "Rename Bookmark..." bookmark-rename :help "Change the name of a bookmark") (delete-all menu-item "Delete all Bookmarks..." bookmark-delete-all :help "Delete all bookmarks from the bookmark list") (delete menu-item "Delete Bookmark..." bookmark-delete :help "Delete a bookmark from the bookmark list") (edit menu-item "Edit Bookmark List" bookmark-bmenu-list :help "Display a list of existing bookmarks") (save menu-item "Save Bookmarks" bookmark-save :help "Save currently defined bookmarks") (write menu-item "Save Bookmarks As..." bookmark-write :help "Write bookmarks to a file (reading the file name with the minibuffer)") (load menu-item "Load a Bookmark File..." bookmark-load :help "Load bookmarks from a bookmark file)") "Bookmark functions"))

(fset 'ispell-menu-map
  '(keymap (ispell-buffer menu-item "Spell-Check Buffer" ispell-buffer :help "Check spelling of selected buffer") (ispell-message menu-item "Spell-Check Message" ispell-message :visible (eq major-mode 'mail-mode) :help "Skip headers and included message text") (ispell-region menu-item "Spell-Check Region" ispell-region :enable mark-active :help "Spell-check text in marked region") (ispell-comments-and-strings menu-item "Spell-Check Comments" ispell-comments-and-strings :help "Spell-check only comments and strings") (ispell-word menu-item "Spell-Check Word" ispell-word :help "Spell-check word at cursor") (ispell-continue menu-item "Continue Spell-Checking" ispell-continue :enable (and (boundp 'ispell-region-end) (marker-position ispell-region-end) (equal (marker-buffer ispell-region-end) (current-buffer))) :help "Continue spell checking last region") (ispell-complete-word-interior-frag menu-item "Complete Word Fragment" ispell-complete-word-interior-frag :help "Complete word fragment at cursor") (ispell-complete-word menu-item "Complete Word" ispell-complete-word :help "Complete word at cursor using dictionary") (flyspell-mode menu-item "Automatic spell checking (Flyspell)" flyspell-mode :help "Check spelling while you edit the text" :button (:toggle bound-and-true-p flyspell-mode)) (ispell-help menu-item "Help" nil :help "Show standard Ispell keybindings and commands") (ispell-customize menu-item "Customize..." nil :help "Customize spell checking options") (ispell-pdict-save menu-item "Save Dictionary" nil :help "Save personal dictionary") (ispell-kill-ispell menu-item "Kill Process" nil :enable (and (boundp 'ispell-process) ispell-process (eq (ispell-process-status) 'run)) :help "Terminate Ispell subprocess") (ispell-change-dictionary menu-item "Change Dictionary..." ispell-change-dictionary :help "Supply explicit dictionary file name") "Spell"))

(fset 'menu-bar-ediff-menu
  '(keymap (ediff-files menu-item "Two Files..." ediff-files :help "Compare two files simultaneously") (ediff-buffers menu-item "Two Buffers..." ediff-buffers :help "Compare two buffers simultaneously") (ediff-files3 menu-item "Three Files..." ediff-files3 :help "Compare three files simultaneously") (ediff-buffers3 menu-item "Three Buffers..." ediff-buffers3 :help "Compare three buffers simultaneously") (separator-ediff-files "--") (ediff-directories menu-item "Two Directories..." ediff-directories :help "Compare files common to two directories simultaneously") (ediff-directories3 menu-item "Three Directories..." ediff-directories3 :help "Compare files common to three directories simultaneously") (separator-ediff-directories "--") (ediff-revision menu-item "File with Revision..." ediff-revision :help "Compare file with its older versions") (ediff-dir-revision menu-item "Directory Revisions..." ediff-directory-revisions :help "Compare directory files with their older versions") (separator-ediff-regions "--") (ediff-regions-wordwise menu-item "Regions Word-by-word..." ediff-regions-wordwise :help "Compare regions word-wise") (ediff-regions-linewise menu-item "Regions Line-by-line..." ediff-regions-linewise :help "Compare regions line-wise") (separator-ediff-windows "--") (ediff-windows-wordwise menu-item "Windows Word-by-word..." ediff-windows-wordwise :help "Compare windows word-wise") (ediff-windows-linewise menu-item "Windows Line-by-line..." ediff-windows-linewise :help "Compare windows line-wise") (window menu-item "This Window and Next Window" compare-windows :help "Compare the current window and the next window") (separator-ediff-misc "--") (ediff-misc menu-item "Ediff Miscellanea" menu-bar-ediff-misc-menu) "Compare"))

(fset 'menu-bar-ediff-misc-menu
  '(keymap (ediff-doc menu-item "Ediff Manual" ediff-documentation :help "Bring up the Ediff manual") (ediff-cust menu-item "Customize Ediff" ediff-customize :help "Change some of the parameters that govern the behavior of Ediff") (eregistry menu-item "List Ediff Sessions" ediff-show-registry :help "List all active Ediff sessions; it is a convenient way to find and resume such a session") (emultiframe menu-item "Use separate control buffer frame" ediff-toggle-multiframe :help "Switch between the single-frame presentation mode and the multi-frame mode" :button (:toggle eq (bound-and-true-p ediff-window-setup-function) #'ediff-setup-windows-multiframe)) "Ediff Miscellanea"))

(fset 'menu-bar-ediff-merge-menu
  '(keymap (ediff-merge-files menu-item "Files..." ediff-merge-files :help "Merge files (without using ancestor information)") (ediff-merge-files-with-ancestor menu-item "Files with Ancestor..." ediff-merge-files-with-ancestor :help "Merge files by comparing them with a common ancestor") (ediff-merge-buffers menu-item "Buffers..." ediff-merge-buffers :help "Merge buffers (without using ancestor information)") (ediff-merge-buffers-with-ancestor menu-item "Buffers with Ancestor..." ediff-merge-buffers-with-ancestor :help "Merge buffers by comparing their contents with a common ancestor") (separator-ediff-merge-dirs "--") (ediff-merge-directories menu-item "Directories..." ediff-merge-directories :help "Merge files common to a pair of directories") (ediff-merge-directories-with-ancestor menu-item "Directories with Ancestor..." ediff-merge-directories-with-ancestor :help "Merge files common to a pair of directories by comparing the files with common ancestors") (separator-ediff-merge "--") (ediff-merge-revisions menu-item "Revisions..." ediff-merge-revisions :help "Merge versions of the same file (without using ancestor information)") (ediff-merge-revisions-with-ancestor menu-item "Revisions with Ancestor..." ediff-merge-revisions-with-ancestor :help "Merge versions of the same file by comparing them with a common ancestor") (ediff-merge-dir-revisions menu-item "Directory Revisions..." ediff-merge-directory-revisions :help "Merge versions of the files in the same directory (without using ancestor information)") (ediff-merge-dir-revisions-with-ancestor menu-item "Directory Revisions with Ancestor..." ediff-merge-directory-revisions-with-ancestor :help "Merge versions of the files in the same directory by comparing the files with common ancestors") "Merge"))

(fset 'menu-bar-epatch-menu
  '(keymap (ediff-patch-file menu-item "To a File..." ediff-patch-file :help "Apply a patch to a file") (ediff-patch-buffer menu-item "To a Buffer..." ediff-patch-buffer :help "Apply a patch to the contents of a buffer") "Apply Patch"))

(fset 'vc-menu-map
  '(keymap (vc-dir-root menu-item "VC Dir" vc-dir-root :help "Show the VC status of the repository") (vc-ignore menu-item "Ignore File..." vc-ignore :help "Ignore a file under current version control system") (vc-register menu-item "Register" vc-register :help "Register file set into a version control system") (vc-next-action menu-item "Check In/Out" vc-next-action :help "Do the next logical version control operation on the current fileset") (vc-update menu-item "Update to Latest Version" vc-update :help "Update the current fileset's files to their tip revisions") (vc-push menu-item "Push Changes" vc-push :help "Push the current branch's changes") (vc-revert menu-item "Revert to Base Version" vc-revert :help "Revert working copies of the selected file set to their repository contents") (vc-insert-header menu-item "Insert Header" vc-insert-headers :help "Insert headers into a file for use with a version control system.") (separator3 "--") (vc-print-root-log menu-item "Show Top of the Tree History " vc-print-root-log :help "List the change log for the current tree in a window") (vc-print-log menu-item "Show History" vc-print-log :help "List the change log of the current file set in a window") (vc-log-in menu-item "Show Incoming Log" vc-root-log-incoming :help "Show a log of changes that will be received with a pull operation") (vc-log-out menu-item "Show Outgoing Log" vc-root-log-outgoing :help "Show a log of changes that will be sent with a push operation") (vc-update-change-log menu-item "Update ChangeLog" vc-update-change-log :help "Find change log file and add entries from recent version control logs") (vc-root-diff menu-item "Compare Tree with Base Version" vc-root-diff :help "Compare current tree with the base version") (vc-diff menu-item "Compare with Base Version" vc-diff :help "Compare file set with the base version") (vc-revision-other-window menu-item "Show Other Version" vc-revision-other-window :help "Visit another version of the current file in another window") (vc-rename-file menu-item "Rename File" vc-rename-file :help "Rename file") (vc-annotate menu-item "Annotate" vc-annotate :help "Display the edit history of the current file using colors") (separator2 "--") (vc-create-branch menu-item "Create Branch..." vc-create-branch :help "Make a new branch") (vc-switch-branch menu-item "Switch Branch..." vc-switch-branch :help "Switch to another branch") (vc-print-root-branch-log menu-item "Show Top of the Tree Branch History..." vc-print-root-branch-log :help "List the change log for another branch") (vc-print-fileset-branch-log menu-item "Show Branch History..." vc-print-fileset-branch-log :help "List the change log for another branch") (vc-create-tag menu-item "Create Tag" vc-create-tag :help "Create version tag") (vc-retrieve-tag menu-item "Retrieve Tag" vc-retrieve-tag :help "Retrieve tagged version or branch") (separator1 "--") (vc-cherry-pick menu-item "Cherry-Pick Revision" vc-cherry-pick :help "Copy the changes from a single revision to this branch") (vc-revert-or-delete-revision menu-item "Revert Revision" vc-revert-or-delete-revision :help "Undo the effects of a revision") "Version Control"))

(fset 'eudc-tools-menu
  '(keymap (load menu-item "Load Hotlist of Servers" eudc-load-eudc :help "Load the Emacs Unified Directory Client") (new menu-item "New Server" eudc-set-server :help "Set the directory server to SERVER using PROTOCOL") (separator-eudc-query "--") (query menu-item "Query with Form" eudc-query-form :help "Display a form to query the directory server") (expand-inline menu-item "Expand Inline Query" eudc-expand-inline :help "Query the directory server, and expand the query string before point") (separator-eudc-email "--") (email menu-item "Get Email" eudc-get-email :help "Get the email field of NAME from the directory server") (phone menu-item "Get Phone" eudc-get-phone :help "Get the phone field of name from the directory server") "Directory Servers"))


;; Language-environment menu maps.
(fset 'describe-chinese-environment-map
  '(keymap "Chinese Environment" (Chinese-GB "Chinese-GB" . describe-specified-language-support) (Chinese-BIG5 "Chinese-BIG5" . describe-specified-language-support) (Chinese-CNS "Chinese-CNS" . describe-specified-language-support) (Chinese-EUC-TW "Chinese-EUC-TW" . describe-specified-language-support) (Chinese-GBK "Chinese-GBK" . describe-specified-language-support) (Chinese-GB18030 "Chinese-GB18030" . describe-specified-language-support)))

(fset 'describe-cyrillic-environment-map
  '(keymap "Cyrillic Environment" (Cyrillic-ISO "Cyrillic-ISO" . describe-specified-language-support) (Cyrillic-KOI8 "Cyrillic-KOI8" . describe-specified-language-support) (Russian "Russian" . describe-specified-language-support) (Ukrainian "Ukrainian" . describe-specified-language-support) (Cyrillic-ALT "Cyrillic-ALT" . describe-specified-language-support) (Tajik "Tajik" . describe-specified-language-support) (Bulgarian "Bulgarian" . describe-specified-language-support) (Belarusian "Belarusian" . describe-specified-language-support) (Mongolian-cyrillic "Mongolian-cyrillic" . describe-specified-language-support)))

(fset 'describe-indian-environment-map
  '(keymap "Indian Environment" (Devanagari "Devanagari" . describe-specified-language-support) (Bengali "Bengali" . describe-specified-language-support) (Gurmukhi "Gurmukhi" . describe-specified-language-support) (Gujarati "Gujarati" . describe-specified-language-support) (Odia "Odia" . describe-specified-language-support) (Oriya "Oriya" . describe-specified-language-support) (Tamil "Tamil" . describe-specified-language-support) (Telugu "Telugu" . describe-specified-language-support) (Kannada "Kannada" . describe-specified-language-support) (Malayalam "Malayalam" . describe-specified-language-support) (Brahmi "Brahmi" . describe-specified-language-support) (Kaithi "Kaithi" . describe-specified-language-support) (Tirhuta "Tirhuta" . describe-specified-language-support) (Sharada "Sharada" . describe-specified-language-support) (Siddham "Siddham" . describe-specified-language-support) (Syloti\ Nagri "Syloti Nagri" . describe-specified-language-support) (Modi "Modi" . describe-specified-language-support) (Limbu "Limbu" . describe-specified-language-support) (Grantha "Grantha" . describe-specified-language-support) (Lepcha "Lepcha" . describe-specified-language-support) (Meetei\ Mayek "Meetei Mayek" . describe-specified-language-support) (Wancho "Wancho" . describe-specified-language-support) (Toto "Toto" . describe-specified-language-support) (Kharoshthi "Kharoshthi" . describe-specified-language-support)))

(fset 'describe-european-environment-map
  '(keymap "European Environment" (Latin-1 "Latin-1" . describe-specified-language-support) (Latin-2 "Latin-2" . describe-specified-language-support) (Latin-3 "Latin-3" . describe-specified-language-support) (Latin-4 "Latin-4" . describe-specified-language-support) (Latin-5 "Latin-5" . describe-specified-language-support) (Latin-8 "Latin-8" . describe-specified-language-support) (Latin-9 "Latin-9" . describe-specified-language-support) (Esperanto "Esperanto" . describe-specified-language-support) (Dutch "Dutch" . describe-specified-language-support) (German "German" . describe-specified-language-support) (French "French" . describe-specified-language-support) (Italian "Italian" . describe-specified-language-support) (Slovenian "Slovenian" . describe-specified-language-support) (Spanish "Spanish" . describe-specified-language-support) (Polish "Polish" . describe-specified-language-support) (Welsh "Welsh" . describe-specified-language-support) (Latin-6 "Latin-6" . describe-specified-language-support) (Latin-7 "Latin-7" . describe-specified-language-support) (Lithuanian "Lithuanian" . describe-specified-language-support) (Latvian "Latvian" . describe-specified-language-support) (Swedish "Swedish" . describe-specified-language-support) (Croatian "Croatian" . describe-specified-language-support) (Brazilian\ Portuguese "Brazilian Portuguese" . describe-specified-language-support) (Catalan "Catalan" . describe-specified-language-support) (Czech "Czech" . describe-specified-language-support) (Slovak "Slovak" . describe-specified-language-support) (Romanian "Romanian" . describe-specified-language-support) (Georgian "Georgian" . describe-specified-language-support)))

(fset 'describe-misc-environment-map
  '(keymap "Misc Environment" (Hanifi\ Rohingya "Hanifi Rohingya" . describe-specified-language-support) (Adlam "Adlam" . describe-specified-language-support) (Mende\ Kikakui "Mende Kikakui" . describe-specified-language-support) (Gothic "Gothic" . describe-specified-language-support) (Coptic "Coptic" . describe-specified-language-support) (Mongolian-traditional "Mongolian-traditional" . describe-specified-language-support) (Tifinagh "Tifinagh" . describe-specified-language-support)))

(fset 'describe-philippine-environment-map
  '(keymap "Philippine Environment" (Tagalog "Tagalog" . describe-specified-language-support) (Hanunoo "Hanunoo" . describe-specified-language-support) (Buhid "Buhid" . describe-specified-language-support) (Tagbanwa "Tagbanwa" . describe-specified-language-support)))

(fset 'describe-indonesian-environment-map
  '(keymap "Indonesian Environment" (Balinese "Balinese" . describe-specified-language-support) (Javanese "Javanese" . describe-specified-language-support) (Sundanese "Sundanese" . describe-specified-language-support) (Batak "Batak" . describe-specified-language-support) (Rejang "Rejang" . describe-specified-language-support) (Makasar "Makasar" . describe-specified-language-support) (Buginese "Buginese" . describe-specified-language-support)))

(fset 'setup-chinese-environment-map
  '(keymap "Chinese Environment" (Chinese-GB "Chinese-GB" . setup-specified-language-environment) (Chinese-BIG5 "Chinese-BIG5" . setup-specified-language-environment) (Chinese-CNS "Chinese-CNS" . setup-specified-language-environment) (Chinese-EUC-TW "Chinese-EUC-TW" . setup-specified-language-environment) (Chinese-GBK "Chinese-GBK" . setup-specified-language-environment) (Chinese-GB18030 "Chinese-GB18030" . setup-specified-language-environment)))

(fset 'setup-cyrillic-environment-map
  '(keymap "Cyrillic Environment" (Cyrillic-ISO "Cyrillic-ISO" . setup-specified-language-environment) (Cyrillic-KOI8 "Cyrillic-KOI8" . setup-specified-language-environment) (Russian "Russian" . setup-specified-language-environment) (Ukrainian "Ukrainian" . setup-specified-language-environment) (Cyrillic-ALT "Cyrillic-ALT" . setup-specified-language-environment) (Tajik "Tajik" . setup-specified-language-environment) (Bulgarian "Bulgarian" . setup-specified-language-environment) (Belarusian "Belarusian" . setup-specified-language-environment) (Mongolian-cyrillic "Mongolian-cyrillic" . setup-specified-language-environment)))

(fset 'setup-indian-environment-map
  '(keymap "Indian Environment" (Devanagari "Devanagari" . setup-specified-language-environment) (Bengali "Bengali" . setup-specified-language-environment) (Gurmukhi "Gurmukhi" . setup-specified-language-environment) (Gujarati "Gujarati" . setup-specified-language-environment) (Odia "Odia" . setup-specified-language-environment) (Oriya "Oriya" . setup-specified-language-environment) (Tamil "Tamil" . setup-specified-language-environment) (Telugu "Telugu" . setup-specified-language-environment) (Kannada "Kannada" . setup-specified-language-environment) (Malayalam "Malayalam" . setup-specified-language-environment) (Brahmi "Brahmi" . setup-specified-language-environment) (Kaithi "Kaithi" . setup-specified-language-environment) (Tirhuta "Tirhuta" . setup-specified-language-environment) (Sharada "Sharada" . setup-specified-language-environment) (Siddham "Siddham" . setup-specified-language-environment) (Syloti\ Nagri "Syloti Nagri" . setup-specified-language-environment) (Modi "Modi" . setup-specified-language-environment) (Limbu "Limbu" . setup-specified-language-environment) (Grantha "Grantha" . setup-specified-language-environment) (Lepcha "Lepcha" . setup-specified-language-environment) (Meetei\ Mayek "Meetei Mayek" . setup-specified-language-environment) (Wancho "Wancho" . setup-specified-language-environment) (Toto "Toto" . setup-specified-language-environment) (Kharoshthi "Kharoshthi" . setup-specified-language-environment)))

(fset 'setup-european-environment-map
  '(keymap "European Environment" (Latin-1 "Latin-1" . setup-specified-language-environment) (Latin-2 "Latin-2" . setup-specified-language-environment) (Latin-3 "Latin-3" . setup-specified-language-environment) (Latin-4 "Latin-4" . setup-specified-language-environment) (Latin-5 "Latin-5" . setup-specified-language-environment) (Latin-8 "Latin-8" . setup-specified-language-environment) (Latin-9 "Latin-9" . setup-specified-language-environment) (Esperanto "Esperanto" . setup-specified-language-environment) (Dutch "Dutch" . setup-specified-language-environment) (German "German" . setup-specified-language-environment) (French "French" . setup-specified-language-environment) (Italian "Italian" . setup-specified-language-environment) (Slovenian "Slovenian" . setup-specified-language-environment) (Spanish "Spanish" . setup-specified-language-environment) (Polish "Polish" . setup-specified-language-environment) (Welsh "Welsh" . setup-specified-language-environment) (Latin-6 "Latin-6" . setup-specified-language-environment) (Latin-7 "Latin-7" . setup-specified-language-environment) (Lithuanian "Lithuanian" . setup-specified-language-environment) (Latvian "Latvian" . setup-specified-language-environment) (Swedish "Swedish" . setup-specified-language-environment) (Croatian "Croatian" . setup-specified-language-environment) (Brazilian\ Portuguese "Brazilian Portuguese" . setup-specified-language-environment) (Catalan "Catalan" . setup-specified-language-environment) (Czech "Czech" . setup-specified-language-environment) (Slovak "Slovak" . setup-specified-language-environment) (Romanian "Romanian" . setup-specified-language-environment) (Georgian "Georgian" . setup-specified-language-environment)))

(fset 'setup-misc-environment-map
  '(keymap "Misc Environment" (Hanifi\ Rohingya "Hanifi Rohingya" . setup-specified-language-environment) (Adlam "Adlam" . setup-specified-language-environment) (Mende\ Kikakui "Mende Kikakui" . setup-specified-language-environment) (Gothic "Gothic" . setup-specified-language-environment) (Coptic "Coptic" . setup-specified-language-environment) (Mongolian-traditional "Mongolian-traditional" . setup-specified-language-environment) (Tifinagh "Tifinagh" . setup-specified-language-environment)))

(fset 'setup-philippine-environment-map
  '(keymap "Philippine Environment" (Tagalog "Tagalog" . setup-specified-language-environment) (Hanunoo "Hanunoo" . setup-specified-language-environment) (Buhid "Buhid" . setup-specified-language-environment) (Tagbanwa "Tagbanwa" . setup-specified-language-environment)))

(fset 'setup-indonesian-environment-map
  '(keymap "Indonesian Environment" (Balinese "Balinese" . setup-specified-language-environment) (Javanese "Javanese" . setup-specified-language-environment) (Sundanese "Sundanese" . setup-specified-language-environment) (Batak "Batak" . setup-specified-language-environment) (Rejang "Rejang" . setup-specified-language-environment) (Makasar "Makasar" . setup-specified-language-environment) (Buginese "Buginese" . setup-specified-language-environment)))

;; Restore GNU's stored keymap order: `define-key' prepends new alist
;; bindings, but the dump above was emitted in GNU's `map-keymap'
;; iteration order, so each map's alist was built reversed.  Character
;; bindings live in the char-table element and need no fixup; reverse
;; only the cons bindings (keeping leading non-cons elements — the
;; char-table and any prompt string — and any parent tail in place).
(dolist (e (accessible-keymaps (current-global-map)))
  (let ((m (cdr e)))
    ;; Only maps built via (make-keymap)+define-key need reversing;
    ;; literal (keymap ...) menu data is already in GNU order.
    (when (and (consp m) (eq (car m) 'keymap)
               (memq m keymap--builtin-maps))
      (let ((fixed '())
            (binds '())
            (cur (cdr m))
            (tail nil)
            (seen nil))
        (while (consp cur)
          (if (memq cur seen)
              (error "cyclic keymap alist at prefix %S" (car e))
            (push cur seen))
          (cond ((eq (car cur) 'keymap)
                 (setq tail cur)
                 (setq cur nil))
                ((consp (car cur)) (push (car cur) binds))
                (t (push (car cur) fixed)))
          (when (consp cur) (setq cur (cdr cur))))
        (setcdr m (nconc (nreverse fixed) binds tail))))))

;; ---------- interactive commands ----------

(defun repeat (&optional arg)
  "Re-execute the last command, like Emacs's C-x z."
  (interactive "P")
  (let ((cmd last-command))
    (when (and cmd (not (eq cmd 'repeat)))
      (command-execute cmd))))

(defun find-alternate-file (filename)
  "Visit FILENAME, replacing the current buffer's contents (C-x C-v)."
  (interactive "fFind alternate file: ")
  (kill-buffer (current-buffer))
  (find-file filename))

(defun occur (regexp &optional nlines)
  "Show all lines in the current buffer matching REGEXP in *Occur*."
  (interactive "sList lines matching: \nP")
  (let ((src (current-buffer))
        (hits '()))
    (save-excursion
      (goto-char (point-min))
      (let ((ln 1))
        (while (not (eobp))
          (let ((line (buffer-substring (line-beginning-position)
                                        (line-end-position))))
            (when (string-match regexp line)
              (push (format "%7d:%s" ln line) hits)))
          (forward-line 1)
          (setq ln (1+ ln)))))
    (let ((ob (get-buffer-create "*Occur*"))
          (n (length hits)))
      (with-current-buffer ob
        (erase-buffer)
        (insert (format "%d %s for \"%s\" in buffer: %s\n"
                        n (if (= n 1) "match" "matches") regexp
                        (buffer-name src)))
        (dolist (l (nreverse hits))
          (insert l "\n")))
      (message "Searched 1 buffer; %d %s for \"%s\""
               n (if (= n 1) "match" "matches") regexp)
      (display-buffer ob))))

(defun display-buffer (buffer &optional action)
  "Make BUFFER visible in a window without selecting it."
  (let ((w (or (get-buffer-window buffer) (selected-window))))
    (set-window-buffer w buffer))
  buffer)

;; ---------- subr.el-level utilities ----------
(defalias 'cl-subseq #'seq-subseq)

(defun add-to-list (list-var element &optional append compare-fn)
  "Add ELEMENT to the list value of LIST-VAR if not already present."
  (let ((lst (symbol-value list-var)))
    (if (if compare-fn
            (let ((found nil) (rest lst))
              (while (and rest (not found))
                (if (funcall compare-fn element (car rest))
                    (setq found t))
                (setq rest (cdr rest)))
              found)
          (member element lst))
        lst
      (set list-var
           (if append (append lst (list element)) (cons element lst))))))

(defmacro bound-and-true-p (var)
  "Return the value of symbol VAR if bound and non-nil."
  (list 'and (list 'boundp (list 'quote var)) var))

(defmacro save-match-data (&rest body)
  "Execute BODY, restoring the match data afterwards."
  (list 'let (list (list 'match-data '(match-data)))
        (list 'unwind-protect (cons 'progn body)
              '(set-match-data match-data))))

(defmacro with-local-quit (&rest body)
  "Execute BODY with quits allowed."
  (cons 'let (cons '((inhibit-quit nil)) body)))

(defvar signal-hook-function nil
  "If non-nil, `signal' calls this function (with the same arguments)
before doing anything else.")

(defvar inhibit-variable-watchers nil
  "If non-nil, variable watchers are not called.")

(defmacro handler-bind (handlers &rest body)
  "Execute BODY with condition handlers bound.
Each element of HANDLERS is (CONDITIONS HANDLER) where CONDITIONS is a
condition name or list of names; HANDLER is called with the condition
object (CONDITION-NAME . DATA) when a matching signal is raised
uncaught (at debugger-entry time, in the raising dynamic context)."
  (cons 'handler-bind-1
        (cons (cons 'lambda (cons nil body))
              (apply #'append
                     (mapcar (lambda (b)
                               (list (list 'quote
                                           (let ((c (car b)))
                                             (if (consp c) c (list c))))
                                     (car (cdr b))))
                             handlers)))))

(defmacro condition-case-unless-debug (var bodyform &rest handlers)
  "Like `condition-case' (we have no debugger, so equivalent here)."
  (cons 'condition-case
        (cons var (cons bodyform handlers))))

;; GNU implements these three as Lisp macros (their `symbol-function'
;; is `(macro . ...)', not a subr); eval still dispatches them via the
;; special-form table, so the cells exist for `macroexpand'/introspection.
(defmacro save-mark-and-excursion (&rest body)
  "Like `save-excursion', but also save and restore the mark state."
  (list 'let (list (list 'saved-marker '(save-mark-and-excursion--save)))
        (list 'unwind-protect (cons 'save-excursion body)
              '(save-mark-and-excursion--restore saved-marker))))

(defmacro track-mouse (&rest body)
  "Evaluate BODY with mouse movement events enabled."
  (list 'internal--track-mouse (cons 'lambda (cons nil body))))

;; ---------- backquote (port of GNU emacs-lisp/backquote.el) ----------
;; `backquote-process' returns (TAG . FORM): TAG 0 => constant, 1 =>
;; evaluates to the structure, 2 => produces a list to be spliced in.

(defvar backquote-backquote-symbol (intern "`"))
(defvar backquote-unquote-symbol (intern ","))
(defvar backquote-splice-symbol (intern ",@"))

(defun backquote-list*-function (first &rest list)
  "Like `list' but the last argument is the tail of the new list."
  (if list (cons first (apply #'backquote-list*-function list)) first))

;; GNU defines this via `backquote-list*-macro': one expansion step
;; folds the whole spine into a cons chain (list* semantics).
(defmacro backquote-list* (first &rest list)
  "Like `list' but the last argument is the tail of the new list."
  (let ((r (car (last (cons first list)))))
    (dolist (x (cdr (nreverse (cons first list))))
      (setq r (list 'cons x r)))
    r))

(defun backquote-delay-process (s level)
  "Process a (un|back|splice)quote inside a backquote.
This simply recurses through the body."
  (let ((exp (backquote-listify (list (cons 0 (list 'quote (car s))))
                                (backquote-process (cdr s) level))))
    (cons (if (eq (car-safe exp) 'quote) 0 1) exp)))

(defun backquote-process (s &optional level)
  "Process the body of a backquote.
S is the body.  Returns a cons cell whose cdr is piece of code which
is the macro-expansion of S, and whose car is a small integer whose value
can either indicate that the code is constant (0), or not (1), or returns
a list which should be spliced into its environment (2).
LEVEL is only used internally and indicates the nesting level:
0 (the default) is for the toplevel nested inside a single backquote."
  (unless level (setq level 0))
  (cond
   ((vectorp s)
    (let ((n (backquote-process (append s nil) level)))
      (if (= (car n) 0)
	  (cons 0 s)
	(cons 1 (cond
		 ((not (listp (cdr n)))
		  (list 'vconcat (cdr n)))
		 ((eq (nth 1 n) 'list)
		  (cons 'vector (nthcdr 2 n)))
		 ((eq (nth 1 n) 'append)
		  (cons 'vconcat (nthcdr 2 n)))
		 (t
		  (list 'apply '(function vector) (cdr n))))))))
   ((atom s)
    (cons 0 (if (or (null s) (eq s t) (not (symbolp s)))
		s
	      (list 'quote s))))
   ((eq (car s) backquote-unquote-symbol)
    (if (<= level 0)
        (cond
         ((> (length s) 2)
          (error "Multiple args to , are not supported: %S" s))
         (t (cons (if (eq (car-safe (nth 1 s)) 'quote) 0 1)
                  (nth 1 s))))
      (backquote-delay-process s (1- level))))
   ((eq (car s) backquote-splice-symbol)
    (if (<= level 0)
        (if (> (length s) 2)
            (error "Multiple args to ,@ are not supported: %S" s)
          (cons 2 (nth 1 s)))
      (backquote-delay-process s (1- level))))
   ((eq (car s) backquote-backquote-symbol)
      (backquote-delay-process s (1+ level)))
   (t
    (let ((rest s)
	  item firstlist list lists expression)
      ;; Scan this list-level, setting LISTS to a list of forms,
      ;; each of which produces a list of elements
      ;; that should go in this level.
      ;; The order of LISTS is backwards.
      ;; If there are non-splicing elements (constant or variable)
      ;; at the beginning, put them in FIRSTLIST,
      ;; as a list of tagged values (TAG . FORM).
      ;; If there are any at the end, they go in LIST, likewise.
      (while (and (consp rest)
                  ;; Stop if the cdr is an expression inside a backquote or
                  ;; unquote since this needs to go recursively through
                  ;; backquote-process.
                  (not (or (eq (car rest) backquote-unquote-symbol)
                           (eq (car rest) backquote-backquote-symbol))))
	(setq item (backquote-process (car rest) level))
	(cond
	 ((= (car item) 2)
	  ;; Put the nonspliced items before the first spliced item
	  ;; into FIRSTLIST.
	  (if (null lists)
	      (setq firstlist list
		    list nil))
	  ;; Otherwise, put any preceding nonspliced items into LISTS.
	  (if list
	      (push (backquote-listify list '(0 . nil)) lists))
	  (push (cdr item) lists)
	  (setq list nil))
	 (t
	  (setq list (cons item list))))
	(setq rest (cdr rest)))
      ;; Handle nonsplicing final elements, and the tail of the list
      ;; (which remains in REST).
      (if (or rest list)
	  (push (backquote-listify list (backquote-process rest level))
                lists))
      ;; Turn LISTS into a form that produces the combined list.
      (setq expression
	    (if (or (cdr lists)
		    (eq (car-safe (car lists)) backquote-splice-symbol))
		(cons 'append (nreverse lists))
	      (car lists)))
      ;; Tack on any initial elements.
      (if firstlist
	  (setq expression (backquote-listify firstlist (cons 1 expression))))
      (cons (if (eq (car-safe expression) 'quote) 0 1) expression)))))

;; backquote-listify takes (tag . structure) pairs from backquote-process
;; and decides between append, list, backquote-list*, and cons depending
;; on which tags are in the list.

(defun backquote-listify (list old-tail)
  (let ((heads nil) (tail (cdr old-tail)) (list-tail list) (item nil))
    (if (= (car old-tail) 0)
	(setq tail (eval tail)
	      old-tail nil))
    (while (consp list-tail)
      (setq item (car list-tail))
      (setq list-tail (cdr list-tail))
      (if (or heads old-tail (/= (car item) 0))
	  (setq heads (cons (cdr item) heads))
	(setq tail (cons (eval (cdr item)) tail))))
    (cond
     (tail
      (if (null old-tail)
	  (setq tail (list 'quote tail)))
      (if heads
	  (let ((use-list* (or (cdr heads)
			       (and (consp (car heads))
				    (eq (car (car heads))
					backquote-splice-symbol)))))
	    (cons (if use-list* 'backquote-list* 'cons)
		  (append heads (list tail))))
	tail))
     (t (cons 'list heads)))))

(defmacro backquote (structure)
  "Argument STRUCTURE describes a template to build.
The whole structure acts as if it were quoted except for certain
places where expressions are evaluated and inserted or spliced in."
  (cdr (backquote-process structure)))

;; The reader produces (` STRUCTURE) — GNU binds ` to the same macro.
(fset (intern "`") (symbol-function 'backquote))



(defvar minor-mode-alist nil
  "Alist of (MODE . LIGHTER-STRINGS) for minor modes.")

;; GNU registers these as autoload cells; calling them loads the
;; library from lisp/ (see load-path handling in load.rs).
(autoload 'define-minor-mode "easy-mmode"
  "Define a new minor mode MODE." nil t)
(autoload 'define-globalized-minor-mode "easy-mmode"
  "Define a global minor mode." nil t)
;; loaddefs.el registers this obsolete alias eagerly.
(defalias 'easy-mmode-define-minor-mode 'define-minor-mode)
(defalias 'define-global-minor-mode 'define-globalized-minor-mode)
(autoload 'kbd-macro-query "macros"
  "Query user during kbd macro execution." t)
(autoload 'insert-kbd-macro "macros"
  "Insert in buffer the Lisp definition of kbd macro MACRONAME." t)
(autoload 'kmacro-start-macro "kmacro"
  "Record subsequent keyboard input, defining a keyboard macro." t)
(autoload 'kmacro-end-macro "kmacro"
  "Finish defining a keyboard macro." t)
(autoload 'kmacro-start-macro-or-insert-counter "kmacro" nil t)
(autoload 'kmacro-end-or-call-macro "kmacro" nil t)
(autoload 'kmacro-end-or-call-macro-repeat "kmacro" nil t)
(autoload 'kmacro-name-last-macro "kmacro"
  "Assign a name to the last keyboard macro defined." t)
(autoload 'gv-get "gv"
  "Build the code that applies DO to PLACE.")
(autoload 'gv-letplace "gv"
  "Build the code manipulating the generalized variable PLACE." nil t)
(autoload 'gv-define-expander "gv"
  "Attach HANDLER as the gv-expander of NAME." nil t)
(autoload 'gv-define-setter "gv"
  "Define a setter method for generalized variable NAME." nil t)
(autoload 'gv-define-simple-setter "gv"
  "Define a simple setter method for generalized variable NAME." nil t)
(autoload 'setf "gv"
  "Set each PLACE to the value of its VAL." nil t)
(autoload 'incf "gv"
  "Increment generalized variable PLACE by DELTA (default to 1)." nil t)
(autoload 'decf "gv"
  "Decrement generalized variable PLACE by DELTA (default to 1)." nil t)
(autoload 'gv-ref "gv"
  "Return a reference to PLACE." nil t)
(autoload 'defclass "eieio"
  "Define NAME as a class." nil t)
(autoload 'make-instance "eieio"
  "Create an instance of CLASS." nil nil)

;; thingatpt.el autoloads (GNU loaddefs registers exactly these).
(autoload 'forward-thing "thingatpt"
  "Move forward to the end of the Nth next THING." t)
(autoload 'bounds-of-thing-at-point "thingatpt"
  "Determine the start and end buffer locations for the THING at point.")
(autoload 'thing-at-point "thingatpt"
  "Return the THING at point.")
(autoload 'bounds-of-thing-at-mouse "thingatpt"
  "Determine start and end locations for THING at mouse click given by EVENT.")
(autoload 'thing-at-mouse "thingatpt"
  "Return the THING at mouse click specified by EVENT.")
(autoload 'sexp-at-point "thingatpt"
  "Return the sexp at point, or nil if none is found.")
(autoload 'symbol-at-point "thingatpt"
  "Return the symbol at point, or nil if none is found.")
(autoload 'number-at-point "thingatpt"
  "Return the number at point, or nil if none is found.")
(autoload 'list-at-point "thingatpt"
  "Return the Lisp list at point, or nil if none is found.")

;; GNU aliases (resolve immediately, before the library loads).
(defalias 'kmacro-exec-ring-item 'funcall)
(defalias 'name-last-kbd-macro 'kmacro-name-last-macro)

;; ---------- nadvice place forms ----------

;; `add-function'/`remove-function' are GNU macros over generalized
;; places; PLACE is normalized to code evaluating to (KIND . ARGS)
;; and `cl--add-function'/`cl--remove-function' do the wrap/dispatch.
(defun cl--advice-place-code (place)
  (cond
   ((symbolp place) (list 'list ''var (list 'quote place)))
   ((eq (car-safe place) 'local) (list 'list ''var (nth 1 place)))
   ((eq (car-safe place) 'var) (list 'list ''var (nth 1 place)))
   ((eq (car-safe place) 'function) (list 'list ''function (nth 1 place)))
   ((eq (car-safe place) 'symbol-function)
    (list 'list ''function (nth 1 place)))
   ((eq (car-safe place) 'default-value)
    (list 'list ''var (nth 1 place)))
   ((eq (car-safe place) 'get)
    (list 'list ''get (nth 1 place) (nth 2 place)))
   ;; GNU: a quoted place reaches a `(setf quote)' setter and fails.
   ((eq (car-safe place) 'quote) '(quote (setf-quote)))
   (t (list 'list ''bad (list 'quote place)))))

(defmacro add-function (how place function &optional props)
  "Add FUNCTION to the function stored in the generalized PLACE.
HOW is one of the `advice-add' locations; PROPS is an alist that may
contain `name' and `depth'."
  (list 'cl--add-function how (cl--advice-place-code place)
        function props))

(defmacro remove-function (place function)
  "Remove FUNCTION (or the named advice) from the function in PLACE."
  (list 'cl--remove-function (cl--advice-place-code place) function))

(defmacro define-advice (symbol args &rest body)
  "Define an advice and add it to the function named SYMBOL."
  (or (listp args) (signal 'wrong-type-argument (list 'listp args)))
  (or (<= 2 (length args) 4)
      (signal 'wrong-number-of-arguments (list 2 4 (length args))))
  (let* ((how (nth 0 args))
         (lambda-list (nth 1 args))
         (name (nth 2 args))
         (depth (nth 3 args))
         (props (append (and depth (list (cons 'depth depth)))
                        (and name (list (cons 'name name)))))
         (advice (cond ((null name) (cons 'lambda (cons lambda-list body)))
                       ((or (stringp name) (symbolp name))
                        (intern (format "%s@%s" symbol name)))
                       (t (error "Unrecognized name spec `%S'" name)))))
    (append '(prog1)
            (and (symbolp advice)
                 (list (cons 'defun (cons advice (cons lambda-list body)))))
            (list (list 'advice-add (list 'quote symbol) how
                        (list 'function advice)
                        (and props (list 'quote props)))))))

(defun advice-mapc (fun symbol)
  "Apply FUN to each advice added to SYMBOL.
FUN is called with the advice function and its property alist."
  (advice-function-mapc fun symbol))

;; ---------- mode keymaps ----------




;;; -*- Compatibility support for fill.el / newcomment.el ports -*-

;; ---------- custom.el core (GNU data API) ----------

(defvar custom-enabled-themes nil
  "Themes that are enabled for this Emacs session.")

(defvar custom-known-themes '(user changed)
  "Themes that have been defined with `deftheme'.")

(defvar custom-theme-load-path (list 'custom-theme-directory t)
  "List of directories to search for custom theme files.
Each element is either a directory name (a string); the symbol
`custom-theme-directory' (meaning the value of that variable), or
t (meaning the built-in themes directory).")

(defvar custom-theme-directory nil
  "Directory in which to look for user themes.")

(defvar custom-delayed-init-variables nil
  "List of variables whose initialization is delayed.
See `custom-initialize-delay'.")

(defvar custom-file nil
  "File used for storing customization information.")

(defvar custom-local-buffer nil
  "Non-nil means, in customization, to operate on buffer-local settings.")

(defun custom-add-to-group (group member type)
  "To existing GROUP add a new MEMBER of type TYPE.
If there already is an entry for MEMBER, change its type to TYPE."
  (let* ((mems (get group 'custom-group))
         (elt (assq member mems)))
    (if elt
        (setcar (cdr elt) type)
      (put group 'custom-group (append mems (list (list member type)))))))

(defun custom-add-link (symbol widget)
  "To the custom option SYMBOL add the link WIDGET."
  (unless (member widget (get symbol 'custom-links))
    (put symbol 'custom-links (cons widget (get symbol 'custom-links)))))

(defun custom-add-version (symbol version)
  "To the custom option SYMBOL add the version VERSION."
  (put symbol 'custom-version version))

(defun custom-add-load (symbol load)
  "To the custom option SYMBOL add the dependency LOAD.
LOAD should be either a library file name, or a feature name."
  (unless (member load (get symbol 'custom-loads))
    (put symbol 'custom-loads (cons load (get symbol 'custom-loads)))))

(defun custom-autoload (symbol load)
  "Mark SYMBOL as autoloaded custom option needing LOAD.
See `custom-declare-variable' and `custom-declare-group'."
  (put symbol 'custom-autoload load))

(defun custom-load-symbol (symbol)
  "Load all dependencies for SYMBOL given by `custom-loads'."
  (unless (get symbol 'custom-autoload)
    (let ((loads (get symbol 'custom-loads)))
      (put symbol 'custom-loads nil)
      ;; Mark against recursive loads.
      (put symbol 'custom-autoload 'loads)
      (dolist (load loads)
        (condition-case nil
            (if (symbolp load) (require load) (load load))
          (error nil))))))

(defun custom-handle-keyword (symbol keyword value type)
  "For customization option SYMBOL, handle KEYWORD with VALUE.
TYPE should be `custom-face', `custom-variable' or `custom-group'."
  (unless (listp value)
    (setq value (list value)))
  (cond ((eq keyword :group)
         (custom-add-to-group (car value) symbol type))
        ((eq keyword :link)
         (custom-add-link symbol value))
        ((eq keyword :load)
         (custom-add-load symbol value))
        ((eq keyword :version)
         (custom-add-version symbol (car value)))
        ((eq keyword :set)
         (put symbol 'custom-set (car value)))
        ((eq keyword :get)
         (put symbol 'custom-get (car value)))
        ((eq keyword :set-after)
         (put symbol 'custom-dependencies (nreverse value)))
        ((eq keyword :type)
         (put symbol 'custom-type value))
        ((eq keyword :options)
         (put symbol 'custom-options value))
        ((eq keyword :value)
         (put symbol 'custom-value value))
        ((eq keyword :require)
         (put symbol 'custom-requests
              (append (get symbol 'custom-requests) value)))
        ((eq keyword :risky)
         (put symbol 'risky-local-variable (car value)))
        ((eq keyword :safe)
         (put symbol 'safe-local-variable (car value)))
        ((eq keyword :package-version)
         (put symbol 'custom-package-version value))
        ((eq keyword :tag)
         (put symbol 'custom-tag (car value)))
        ;; Keywords handled elsewhere, or accepted with no plist effect.
        ((memq keyword '(:initialize :local :indent :debug :doc-string
                         :no-autoload :obsolete))
         nil)
        (t (error "Unknown keyword %s" keyword))))

(defun custom-quote (x)
  "Apply `custom-quote' to X: evaluate `(custom-quote Y)' forms within X.
This is obsolete; use `quote' instead."
  (if (consp x)
      (if (eq (car x) 'custom-quote)
          (eval (car (cdr x)))
        (cons (custom-quote (car x)) (custom-quote (cdr x))))
    x))

;;; initialization

(defun custom-initialize-default (symbol exp)
  "Initialize SYMBOL based on EXP.
Set the symbol, using its `:set' function (or `set-default' if it has
none); the variable's default value is what gets set."
  (funcall (or (get symbol 'custom-set) 'set-default) symbol (eval exp)))

(defun custom-initialize-set (symbol exp)
  "Initialize SYMBOL based on EXP.
Set the symbol, using its `:set' function (or `set' if it has none)."
  (funcall (or (get symbol 'custom-set) 'set) symbol (eval exp)))

(defun custom-initialize-reset (symbol exp)
  "Initialize SYMBOL based on EXP.
Set the symbol, using its `:set' function (or `set-default' if it has none)."
  (funcall (or (get symbol 'custom-set) 'set-default) symbol (eval exp)))

(defun custom-initialize-changed (symbol exp)
  "Initialize SYMBOL based on EXP.
Set the symbol, using its `:set' function (or `set' if it has none),
unless it is already bound."
  (unless (default-boundp symbol)
    (funcall (or (get symbol 'custom-set) 'set) symbol (eval exp))))

(defun custom-initialize-delay (symbol exp)
  "Delay initialization of SYMBOL to the next `custom-delayed-init' call.
This is used in files that are preloaded (or for autoloads), so that the
init-code can be evaluated once it is safe (e.g. after theme loading)."
  (unless (memq symbol custom-delayed-init-variables)
    (push symbol custom-delayed-init-variables))
  (put symbol 'custom-delayed-init (list exp)))

;;; declare group

(defun custom-declare-group (group members doc &rest args)
  "Like `defgroup', but GROUP is evaluated as a normal argument."
  (unless (symbolp group)
    (error "Invalid group name `%s'" group))
  (put group 'group-documentation doc)
  (dolist (member members)
    (custom-add-to-group group (car member) (car (cdr member))))
  (let ((rest args))
    (while rest
      (let ((key (car rest)))
        (cond
         ((null (cdr rest))
          (error "Keyword %s is missing an argument" key))
         ((not (symbolp key))
          (error "Junk in args %S" args))
         ((eq key :prefix)
          (put group 'custom-prefix (car (cdr rest))))
         ((eq key :group)
          (custom-add-to-group (car (cdr rest)) group 'custom-group))
         (t
          (custom-handle-keyword group key (car (cdr rest))
                                 'custom-group))))
      (setq rest (cdr (cdr rest)))))
  group)

(defmacro defgroup (group members doc &rest args)
  "Declare GROUP as a customization group containing MEMBERS.
GROUP should be a symbol; MEMBERS a list of (MEMBER TYPE) entries.
DOC is a doc string.  Keyword ARGS are as for `defcustom'."
  (declare (doc-string 3) (indent 1))
  `(custom-declare-group ',group ,members ,doc ,@args))

(defun custom-group-list (group)
  "Return list of groups inside GROUP."
  (let (list)
    (dolist (entry (get group 'custom-group) (nreverse list))
      (when (eq (car (cdr entry)) 'custom-group)
        (push (car entry) list)))))

(defun custom-group-of-mode (mode)
  "Return the custom group for MODE, or nil if it has none."
  (or (get mode 'custom-mode-group)
      (get mode 'custom-group)
      (let ((parent (get mode 'derived-mode-parent)))
        (and parent (custom-group-of-mode parent)))))

;;; declare variable

(defun custom-declare-variable (variable requests doc &rest args)
  "Like `defcustom', but VARIABLE and REQUESTS are evaluated as normal args.
REQUESTS should be an expression to evaluate to compute the value —
`defcustom' passes a form of the shape (funcall (function (lambda ()
INIT)))."
  (unless (symbolp variable)
    (error "Invalid variable name `%s'" variable))
  ;; Record the standard value unless the user already saved one.
  (unless (get variable 'saved-value)
    (put variable 'standard-value (list requests)))
  (put variable 'variable-documentation doc)
  ;; Process keyword arguments; `:initialize' selects the init function
  ;; and is not recorded on the plist.
  (let ((initialize 'custom-initialize-default)
        (rest args))
    (while rest
      (let ((key (car rest)))
        (cond
         ((null (cdr rest))
          (error "Keyword %s is missing an argument" key))
         ((eq key :initialize)
          (setq initialize (car (cdr rest))))
         (t
          (unless (symbolp key)
            (error "Junk in args %S" args))
          (custom-handle-keyword variable key (car (cdr rest))
                                 'custom-variable))))
      (setq rest (cdr (cdr rest))))
    ;; Initialize, unless the variable is already bound.
    (unless (default-boundp variable)
      (funcall initialize variable requests)))
  variable)

(defmacro defcustom (symbol initial doc &rest args)
  "Declare SYMBOL as a customizable variable that defaults to INITIAL.
INITIAL is a Lisp expression evaluated to give the default.
DOC is the variable documentation string.  Keyword ARGS:
`:group', `:type', `:options', `:set', `:get', `:initialize',
`:risky', `:safe', `:require', `:set-after', `:version',
`:package-version', `:link', `:tag', `:load', `:local'."
  (declare (doc-string 3) (indent 1))
  `(custom-declare-variable
    ',symbol
    '(funcall #'(lambda () ,initial))
    ,doc
    ,@args))

(defun custom-variable-p (arg)
  "Return non-nil if ARG is a customizable variable.
A customizable variable is a `defcustom' variable, or an autoloaded
one, or one that has been set through Custom."
  (and (symbolp arg)
       (or (get arg 'standard-value)
           (get arg 'custom-autoload)
           (get arg 'customized-value))))

(defun custom-variable-state (variable)
  "Return the state of VARIABLE as seen by Customize.
The state is `hidden', `standard', `saved', `set' or `themed'."
  (cond ((get variable 'saved-value) 'saved)
        ((get variable 'customized-value) 'set)
        ((get variable 'theme-value) 'themed)
        ((get variable 'standard-value) 'standard)
        (t 'hidden)))

(defun custom-note-var-changed (symbol)
  "Inform Custom that SYMBOL has been set (changed) programmatically.
This records the current value in SYMBOL's `customized-value' property."
  (put symbol 'customized-value
       (list (custom-quote (symbol-value symbol)))))

(defun custom-set-default (variable value)
  "Default :set function for a customizable variable.
VALUE is evaluated; the variable's default is set to the result."
  (set-default variable value))

(defun custom-set-minor-mode (variable value)
  ":set function for minor mode variables.
Normally, this sets the default value of VARIABLE to nil if VALUE
is nil and to t otherwise, but it calls VARIABLE as a function with
argument 1 (enable) or -1 (disable), like minor-mode functions do."
  (funcall variable (if value 1 -1)))

(defun custom-reevaluate-setting (symbol)
  "Re-execute :set function of SYMBOL with the saved/customized value."
  (interactive "vVariable: ")
  (funcall (or (get symbol 'custom-set) 'set) symbol
           (eval (car (or (get symbol 'saved-value)
                          (get symbol 'standard-value))))))

;;; declare face

(defun face-spec-set (face spec &optional _frame)
  "Override the face attributes of FACE according to SPEC.
SPEC is a list of (DISPLAY ATTRS) entries; this implementation
applies the attributes of the `t' entries, falling back to `default'."
  (put face 'face-override-spec spec)
  (let ((default-attrs nil) (applied nil))
    (dolist (entry spec)
      ;; GNU face-spec-choose: ATTRS is (cdr entry); a one-element cdr
      ;; is the old-style (DISPLAY PLIST) form where PLIST is the
      ;; single element.
      (let* ((display (car entry))
             (attrs-cdr (cdr entry))
             (attrs (if (null (cdr attrs-cdr)) (car attrs-cdr) attrs-cdr)))
        (cond
         ((eq display t)
          (apply 'set-face-attribute face nil attrs)
          (setq applied t))
         ((eq display 'default)
          (setq default-attrs attrs)))))
    (when (and (not applied) default-attrs)
      (apply 'set-face-attribute face nil default-attrs))))

(defun custom-declare-face (face spec doc &rest args)
  "Like `defface', but FACE is evaluated as a normal argument."
  (unless (internal-lisp-face-p face)
    (internal-make-lisp-face face))
  (unless (get face 'face-defface-spec)
    (put face 'face-defface-spec spec)
    (when (and spec (not (get face 'saved-face)))
      ;; Respect faces already set by the user.
      (face-spec-set face spec))
    (put face 'face-documentation doc))
  (let ((rest args))
    (while rest
      (let ((key (car rest)))
        (if (null (cdr rest))
            (error "Keyword %s is missing an argument" key)
          (custom-handle-keyword face key (car (cdr rest))
                                 'custom-face)))
      (setq rest (cdr (cdr rest)))))
  face)

(defmacro defface (face spec doc &rest args)
  "Define FACE (a symbol) as a customizable face with SPEC and DOC.
SPEC is a list of (DISPLAY . PLIST) entries; ARGS are custom keywords."
  (declare (doc-string 3))
  `(custom-declare-face ',face ,spec ,doc ,@args))

(defun custom-facep (face)
  "Return non-nil if FACE is a customizable face."
  (and (symbolp face)
       (get face 'face-defface-spec)
       (facep face)))

(defun custom-face-state (face)
  "Return the state of FACE as seen by Customize."
  (cond ((get face 'saved-face) 'saved)
        ((get face 'customized-face) 'set)
        ((get face 'theme-face) 'themed)
        ((get face 'face-defface-spec) 'standard)
        (t 'hidden)))

;;; themes

(defun custom-make-theme-feature (theme)
  "Given a symbol THEME, create a new symbol named THEME-theme.
This is used to convert a symbol into a feature name."
  (intern (concat (symbol-name theme) "-theme")))

(defun custom-declare-theme (theme &optional doc)
  "Like `deftheme', but THEME is evaluated as a normal argument."
  (unless (memq theme custom-known-themes)
    (push theme custom-known-themes))
  (put theme 'theme-feature (custom-make-theme-feature theme))
  (put theme 'theme-documentation doc)
  theme)

(defmacro deftheme (theme &optional doc)
  "Define THEME (a symbol) as a custom theme, and return THEME.
Optional argument DOC is a doc string describing the theme."
  `(custom-declare-theme ',theme ,doc))

(defun custom-theme-p (theme)
  "Return non-nil if THEME is a valid custom theme name."
  (memq theme custom-known-themes))

(defun custom-check-theme (theme)
  "Check THEME for validity.
Raise an error if THEME is not a custom theme; return nil otherwise."
  (unless (custom-theme-p theme)
    (error "Unknown theme `%s'" theme)))

(defun provide-theme (theme)
  "Indicate that this file provides THEME feature."
  (provide (custom-make-theme-feature theme)))

(defun custom-push-theme (prop symbol theme mode value)
  "Add (PROP SYMBOL THEME VALUE) to the THEME's `theme-settings' property.
If THEME is `user', instead record a (THEME VALUE) entry in SYMBOL's
PROP property (a user setting stored on the symbol itself).
MODE `set' or `set-buffer' records a new setting; `reset' removes none
here (removal is done by `custom-theme-reset-variables' et al)."
  (if (eq theme 'user)
      (put symbol prop (cons (list theme value) (get symbol prop)))
    (when (memq mode '(set set-buffer))
      (put theme 'theme-settings
           (cons (list prop symbol theme value)
                 (get theme 'theme-settings))))))

(defun custom-theme-set-variables (theme &rest args)
  "Record a list of variable settings for THEME.
Each argument in ARGS should be a list of the form (VAR EXP [NOW
[REQUEST [COMMENT]]]); only VAR and EXP are recorded."
  (custom-check-theme theme)
  (dolist (entry args)
    (custom-push-theme 'theme-value (car entry) theme 'set
                       (car (cdr entry)))))

(defun custom-theme-set-faces (theme &rest args)
  "Record a list of face settings for THEME.
Each argument in ARGS should be a list of the form (FACE SPEC
[NOW [COMMENT]]); only FACE and SPEC are recorded."
  (custom-check-theme theme)
  (dolist (entry args)
    (custom-push-theme 'theme-face (car entry) theme 'set
                       (car (cdr entry)))))

(defun custom--settings-delete (settings prop symbol)
  "Delete the (PROP SYMBOL ...) entries from a `theme-settings' list."
  (let (out)
    (dolist (e settings (nreverse out))
      (unless (and (eq (car e) prop) (eq (car (cdr e)) symbol))
        (push e out)))))

(defun custom--theme-entry-delete (entries theme)
  "Delete the (THEME ...) entries from a `theme-value'/`theme-face' list."
  (let (out)
    (dolist (e entries (nreverse out))
      (unless (eq (car e) theme)
        (push e out)))))

(defun custom-theme-reset-variables (theme &rest args)
  "Reset the value of the variables to values previously recorded.
Each element of ARGS is a variable name, or a list (VAR VAL)."
  (custom-check-theme theme)
  (dolist (arg args)
    (let ((var (if (consp arg) (car arg) arg)))
      (put theme 'theme-settings
           (custom--settings-delete (get theme 'theme-settings)
                                    'theme-value var))
      (when (memq theme custom-enabled-themes)
        (put var 'theme-value
             (custom--theme-entry-delete (get var 'theme-value) theme))
        (custom-theme-recalc-variable var)))))

(defun custom-theme-reset-faces (theme &rest args)
  "Reset the specs of the faces to values previously recorded.
Each element of ARGS is a face name, or a list (FACE SPEC)."
  (custom-check-theme theme)
  (dolist (arg args)
    (let ((face (if (consp arg) (car arg) arg)))
      (put theme 'theme-settings
           (custom--settings-delete (get theme 'theme-settings)
                                    'theme-face face))
      (when (memq theme custom-enabled-themes)
        (put face 'theme-face
             (custom--theme-entry-delete (get face 'theme-face) theme))
        (custom-theme-recalc-face face)))))

(defun custom-theme-recalc-variable (variable &optional _frame)
  "Set the default value of VARIABLE to a theme setting, if one exists.
This consults VARIABLE's `theme-value' property for settings made by
enabled themes (or `user'), falling back to a `changed' entry, then
`saved-value' and `customized-value'."
  (let ((theme-values (get variable 'theme-value))
        (rest custom-enabled-themes)
        (set (or (get variable 'custom-set) 'set))
        winner)
    (while (and rest (not winner))
      (let ((e (assq (car rest) theme-values)))
        (when e (setq winner e)))
      (setq rest (cdr rest)))
    (unless winner (setq winner (assq 'user theme-values)))
    (cond
     (winner
      (funcall set variable (car (cdr winner))))
     ((assq 'changed theme-values)
      (funcall set variable (car (cdr (assq 'changed theme-values)))))
     ((get variable 'saved-value)
      (funcall set variable (eval (car (get variable 'saved-value)))))
     ((get variable 'customized-value)
      (funcall set variable (car (get variable 'customized-value))))
     ((get variable 'standard-value)
      (funcall set variable (eval (car (get variable 'standard-value))))))))

(defun custom-theme-recalc-face (face &optional _frame)
  "Set the attributes of FACE to a theme spec, if one exists.
This consults FACE's `theme-face' property for specs made by enabled
themes (or `user'), falling back to `face-defface-spec'."
  (let ((theme-faces (get face 'theme-face))
        (rest custom-enabled-themes)
        winner)
    (while (and rest (not winner))
      (let ((e (assq (car rest) theme-faces)))
        (when e (setq winner e)))
      (setq rest (cdr rest)))
    (unless winner (setq winner (assq 'user theme-faces)))
    (cond
     (winner (face-spec-set face (car (cdr winner))))
     ((get face 'face-defface-spec)
      (face-spec-set face (get face 'face-defface-spec))))))

(defun enable-theme (theme)
  "Reenable all variable and face settings defined by THEME.
THEME should be either `user', or a theme defined via `deftheme'."
  (interactive "SEnable custom theme: ")
  (if (memq theme custom-enabled-themes)
      (message "Theme `%s' is already enabled" theme)
    (custom-check-theme theme)
    ;; Record `changed' entries for variables that differ from their
    ;; standard value, then record the theme's settings on the symbols.
    (dolist (setting (get theme 'theme-settings))
      (let ((prop (nth 0 setting)) (symbol (nth 1 setting))
            (val (nth 3 setting)))
        (when (eq prop 'theme-value)
          ;; Record the pre-theme value as `changed' — but only when the
          ;; variable is not already under theme control, and only when
          ;; its current value differs from the standard value.
          (when (and (null (get symbol prop)) (boundp symbol))
            (let ((std (car (get symbol 'standard-value))))
              (unless (and std
                           (equal (eval std) (default-value symbol)))
                (put symbol prop
                     (list (list 'changed (default-value symbol))))))))
        (put symbol prop
             (cons (list theme val) (get symbol prop)))))
    (push theme custom-enabled-themes)
    (dolist (setting (get theme 'theme-settings))
      (let ((prop (nth 0 setting)) (symbol (nth 1 setting)))
        (cond ((eq prop 'theme-value)
               (custom-theme-recalc-variable symbol))
              ((eq prop 'theme-face)
               (custom-theme-recalc-face symbol))))))
  theme)

(defun disable-theme (theme)
  "Disable all variable and face settings defined by THEME."
  (interactive "SDisable custom theme: ")
  (custom-check-theme theme)
  (if (not (memq theme custom-enabled-themes))
      (message "%s theme is not currently enabled" theme)
    (setq custom-enabled-themes (delq theme custom-enabled-themes))
    (dolist (setting (get theme 'theme-settings))
      (let ((prop (nth 0 setting)) (symbol (nth 1 setting)))
        (put symbol prop
             (custom--theme-entry-delete (get symbol prop) theme))
        (cond ((eq prop 'theme-value)
               (custom-theme-recalc-variable symbol))
              ((eq prop 'theme-face)
               (custom-theme-recalc-face symbol))))))
  theme)

(defun custom-theme-load-themes (&optional _update)
  "Load themes specified by the variable `custom-enabled-themes'."
  (dolist (theme custom-enabled-themes)
    (load-theme theme)))

(defun custom-available-themes ()
  "Return a list of available custom themes (symbols like `foo-theme')."
  (let (out)
    (dolist (dir custom-theme-load-path)
      (when (eq dir 'custom-theme-directory)
        (setq dir custom-theme-directory))
      (when (eq dir t)
        (setq dir (bound-and-true-p data-directory)))
      (when (and (stringp dir) (file-directory-p dir))
        (dolist (file (directory-files dir nil "-theme\\.el\\'"))
          ;; "foo-theme.el" names the theme `foo'.
          (let ((theme (intern (substring file 0 (- (length file) 9)))))
            (unless (memq theme out)
              (push theme out))))))
    (nreverse out)))

(defun load-theme (theme &optional _no-confirm no-enable)
  "Load Custom theme THEME from its file and enable it.
The theme file is named THEME-theme.el; it is searched in
`custom-theme-load-path' and `load-path'."
  (interactive "SLoad custom theme: ")
  (unless (custom-theme-p theme)
    ;; Search theme dirs, then the regular load-path.
    (let ((file (concat (symbol-name theme) "-theme"))
          (found nil))
      (dolist (dir custom-theme-load-path)
        (when (eq dir 'custom-theme-directory)
          (setq dir custom-theme-directory))
        (when (eq dir t)
          (setq dir (bound-and-true-p data-directory)))
        (when (and (not found) (stringp dir) (file-directory-p dir))
          (let ((full (expand-file-name (concat file ".el") dir)))
            (when (file-exists-p full)
              (load full nil t)
              (setq found t)))))
      (unless found
        (load file nil t))))
  (if (not (custom-theme-p theme))
      (error "Unable to find theme file for `%s'" theme)
    (unless no-enable
      (enable-theme theme)))
  theme)

;;; set/get user values

(defun custom-set-variables (&rest args)
  "Initialize variables according to user specifications.
Each argument should be a list of the form (VAR VALUE [NOW [REQUEST
[COMMENT]]]).  VALUE is evaluated to give the value; REQUEST is a list
of features to load; COMMENT is a comment string."
  (dolist (entry args)
    (let ((var (nth 0 entry)) (val (nth 1 entry))
          (requests (nth 3 entry)) (comment (nth 4 entry)))
      (when requests
        (put var 'custom-requests
             (append (get var 'custom-requests) requests)))
      ;; Record the setting as a user (pseudo-theme) setting.
      (put var 'saved-value (list val))
      (put var 'saved-variable-comment comment)
      (custom-push-theme 'theme-value var 'user 'set (eval val))
      ;; If VAR is a declared customizable variable, apply now.
      (when (custom-variable-p var)
        (custom-theme-recalc-variable var)))))

(defun custom-set-faces (&rest args)
  "Initialize faces according to user specifications.
Each argument should be a list of the form (FACE SPEC [NOW
[COMMENT]])."
  (dolist (entry args)
    (let ((face (nth 0 entry)) (spec (nth 1 entry))
          (comment (nth 3 entry)))
      (put face 'saved-face spec)
      (put face 'saved-face-comment comment)
      (custom-push-theme 'theme-face face 'user 'set spec)
      (when (custom-facep face)
        (custom-theme-recalc-face face)))))

(defun customize-set-variable (variable value &optional comment)
  "Set the default value of VARIABLE to VALUE, and return VALUE.
VALUE is a Lisp object.
If COMMENT is non-nil, set `customized-variable-comment' to it."
  (interactive "vCustomize set variable: \nxExpression: ")
  (custom-load-symbol variable)
  (put variable 'customized-value (list (custom-quote value)))
  (put variable 'customized-variable-comment comment)
  (custom-push-theme 'theme-value variable 'user 'set value)
  (funcall (or (get variable 'custom-set) 'set) variable value)
  value)

(defun customize-set-value (variable value &optional comment)
  "Set VARIABLE to VALUE.  VALUE is a Lisp object.
If `custom-file' is non-nil, record the setting so it can be saved;
otherwise just set it and warn that the change is temporary."
  (interactive "vCustomize set variable: \nxExpression (value): ")
  (custom-load-symbol variable)
  (if (bound-and-true-p custom-file)
      (progn
        (unless (get variable 'saved-value)
          (put variable 'standard-value
               (list (custom-quote
                      (if (default-boundp variable)
                          (default-value variable) nil)))))
        (put variable 'customized-value (list (custom-quote value)))
        (put variable 'customized-variable-comment comment)
        (custom-push-theme 'theme-value variable 'user 'set value)
        (funcall (or (get variable 'custom-set) 'set) variable value))
    (message "Setting `%s' temporarily since \"emacs -q\" would overwrite customizations"
             variable)
    (funcall (or (get variable 'custom-set) 'set) variable value)))

(defun customize-save-variable (variable value &optional comment)
  "Set the default value of VARIABLE to VALUE, and return VALUE.
Like `customize-set-variable', but records VALUE as `saved-value'."
  (interactive "vCustomize set and save variable: \nxExpression: ")
  (custom-load-symbol variable)
  (put variable 'saved-value (list (custom-quote value)))
  (put variable 'saved-variable-comment comment)
  (custom-push-theme 'theme-value variable 'user 'set value)
  (funcall (or (get variable 'custom-set) 'set) variable value)
  value)

(defun custom-add-frequent-value (variable value)
  "Add VALUE to the list of frequently used values of VARIABLE."
  (let ((options (get variable 'custom-options)))
    (unless (member value options)
      (put variable 'custom-options (cons value options)))))

(defun custom-unlispify-menu-entry (symbol &optional no-suffix)
  "Convert SYMBOL into a menu-friendly version."
  (let ((name (replace-regexp-in-string
               "-" " "
               (replace-regexp-in-string "\\(-mode\\)?\\'" ""
                                         (symbol-name symbol)))))
    (concat (upcase (substring name 0 1)) (substring name 1)
            (cond (no-suffix "")
                  ((boundp symbol) " (variable)")
                  ((facep symbol) " (face)")
                  (t " (group)")))))

(defun custom-variable-tag (variable)
  "Return the `:tag' of VARIABLE or its unlispified name."
  (or (get variable 'custom-tag)
      (format "Customize option: `%s'"
              (custom-unlispify-menu-entry variable))))

(defun custom-face-tag (face)
  "Return the `:tag' of FACE or its unlispified name."
  (or (get face 'custom-tag)
      (format "Customize face: `%s'"
              (custom-unlispify-menu-entry face))))

(defun custom-group-tag (group)
  "Return the `:tag' of GROUP or its unlispified name."
  (or (get group 'custom-tag)
      (format "Customize group: `%s'"
              (custom-unlispify-menu-entry group))))

(defmacro defvar-local (var value &optional docstring)
  (list 'progn
        (list 'defvar var value docstring)
        (list 'make-variable-buffer-local (list 'quote var))))

(defmacro defsubst (name arglist &rest body)
  (cons 'defun (cons name (cons arglist body))))

;; ---------- subr.el / subr-x.el cluster (GNU-dumped) ----------

(defsubst xor (cond1 cond2)
  "Return the non-nil argument if exactly one of COND1, COND2 is non-nil."
  (if cond1 (unless cond2 cond1) cond2))

(defmacro if-let* (varlist then &rest else)
  "Bind each VAR in VARLIST to VAL; eval THEN when all non-nil, else ELSE.
Each binding spec is (VAR VAL), (VAR) (binds nil), or a bare VAR
\(tests VAR's current value)."
  (if (null varlist)
      `(progn ,then)
    (let ((spec (car varlist)))
      (cond
       ((symbolp spec) (setq spec (list spec spec)))
       ((null (cdr spec)) (setq spec (list (car spec) nil))))
      `(let* (,spec)
         (if ,(car spec)
             (if-let* ,(cdr varlist) ,then ,@else)
           ,@(if else `((progn ,@else)) '(nil)))))))

(defmacro when-let* (varlist &rest body)
  "Bind each VAR in VARLIST; when all non-nil, eval BODY."
  `(if-let* ,varlist (progn ,@body)))

;; The non-* variants take a single (VAR VAL) spec or legacy [VAR VAL].
(defmacro if-let (varlist then &rest else)
  "Like `if-let*' for a single binding (obsolete shape)."
  (let ((spec (if (and (consp varlist) (cdr varlist)
                       (symbolp (car varlist)) (not (consp (car varlist))))
                  (list varlist)          ; legacy (var val)
                varlist)))
    `(if-let* ,spec ,then ,@else)))

(defmacro when-let (varlist &rest body)
  "Like `when-let*' for a single binding (obsolete shape)."
  `(if-let ,varlist (progn ,@body)))

(defmacro and-let* (varlist &rest body)
  "Bind (VAR VAL) specs in sequence; eval BODY when all yield non-nil.
A (FORM) spec evaluates FORM for truth without binding; a bare
VAR spec tests VAR's current value."
  (if (null varlist)
      `(progn ,@body)
    (let ((spec (car varlist))
          (rest (cdr varlist)))
      (cond
       ((and (consp spec) (symbolp (car spec)) (cdr spec))
        `(let* ((,(car spec) (and t ,(cadr spec))))
           (and ,(car spec) (and-let* ,rest ,@body))))
       ((consp spec)
        (let ((tmp (gensym)))
          `(let* ((,tmp (and t ,(car spec))))
             (and ,tmp (and-let* ,rest ,@body)))))
       (t
        `(and ,spec (and-let* ,rest ,@body)))))))

(defun cl--thread-expand (x forms last)
  (if (null forms)
      x
    (let ((f (car forms)))
      (cl--thread-expand
       (if (consp f)
           (if last
               `(,(car f) ,@(cdr f) ,x)
             `(,(car f) ,x ,@(cdr f)))
         (list f x))
       (cdr forms) last))))

(defmacro thread-first (&rest forms)
  "Thread X through FORMS as the first argument."
  (cl--thread-expand (car forms) (cdr forms) nil))

(defmacro thread-last (&rest forms)
  "Thread X through FORMS as the last argument."
  (cl--thread-expand (car forms) (cdr forms) t))

(defmacro dlet (spec &rest body)
  "Like `let*' with dynamic binding (ours is already dynamic)."
  `(let* ,spec ,@body))

(defun define-error (name message &optional parent)
  "Define NAME as an error with MESSAGE inheriting from PARENT."
  (let* ((parent (or parent 'error))
         (conds (cons name (or (get parent 'error-conditions)
                               (list 'error)))))
    (put name 'error-conditions conds)
    (put name 'error-message message)))

(defvar after-load-alist nil
  "Alist of (FILE . FORMS) to eval after FILE is loaded.")

(defun eval-after-load (file form)
  "Arrange that FORM is evaluated after FILE is loaded.
String FILEs become a regexp matching the file name at the end of
any path, with optional .so/.dylib/.elc/.el/.gz extension."
  (let ((key (if (stringp file)
                 (concat "\\(\\`\\|/\\)" (regexp-quote file)
                         "\\(\\.so\\|\\.dylib\\|\\.elc\\|\\.el\\)?\\(\\.gz\\)?\\'")
               file)))
    (setq after-load-alist
          (cons (list key `(lambda () ,form)) after-load-alist)))
  nil)

(defmacro with-eval-after-load (file &rest body)
  "Arrange that BODY runs after FILE is loaded."
  `(eval-after-load ,file (function (lambda () ,@body))))

(defun load-library (library)
  "Load the Emacs Lisp library named LIBRARY."
  (interactive "sLoad library: ")
  (load library))

(defun delete-consecutive-dups (list &optional circular)
  "Destructively remove consecutive `equal' duplicates from LIST."
  (when (and circular (consp list) (consp (cdr list))
             (equal (car list) (car (last list))))
    (setcdr (nthcdr (- (length list) 2) list) nil))
  (let ((tail list))
    (while (cdr-safe tail)
      (if (equal (car tail) (cadr tail))
          (setcdr tail (cddr tail))
        (setq tail (cdr tail)))))
  list)

(defun assoc-delete-all (key alist &optional testfn)
  "Delete from ALIST all elements whose car matches KEY via TESTFN."
  (let ((test (or testfn #'equal)))
    (while (and alist (funcall test key (caar alist)))
      (setq alist (cdr alist)))
    (let ((tail alist))
      (while (cdr tail)
        (if (funcall test key (car (cadr tail)))
            (setcdr tail (cddr tail))
          (setq tail (cdr tail)))))
    alist))

(defun string-limit (string &optional length coding-system)
  "Return STRING truncated to LENGTH characters."
  (let ((n (length string)))
    (if (or (null length) (>= length n))
        string
      (substring string 0 (max 0 length)))))

(defun string-clean-whitespace (string)
  "Collapse whitespace runs in STRING to single spaces; trim ends."
  (string-trim
   (if (fboundp 'replace-regexp-in-string)
       (replace-regexp-in-string "[\\s-]+" " " string)
     (string-replace "\n" " " string))))

(defun forward-thing (thing &optional n)
  "Move point forward N THINGs."
  (or n (setq n 1))
  (cond
   ((memq thing '(symbol)) (forward-symbol n))
   ((memq thing '(word)) (forward-word n))
   ((memq thing '(sexp)) (forward-sexp n))
   ((memq thing '(list)) (forward-list n))
   ((memq thing '(line)) (forward-line n))
   ((memq thing '(sentence)) (forward-sentence n))
   ((memq thing '(paragraph)) (forward-paragraph n))
   ((memq thing '(defun)) (beginning-of-defun (- n)))
   ((memq thing '(char)) (forward-char n))
   (t (error "Unknown thing: %s" thing))))

;; files.el (verbatim GNU).
(defun locate-file (filename path &optional suffixes predicate)
  "Search for FILENAME through PATH.
If found, return the absolute file name of FILENAME; otherwise
return nil.
PATH should be a list of directories to look in, like the lists in
`exec-path' or `load-path'.
If SUFFIXES is non-nil, it should be a list of suffixes to append to
file name when searching.  If SUFFIXES is nil, it is equivalent to (\"\").
Use (\"/\") to disable PATH search, but still try the suffixes in SUFFIXES.
If non-nil, PREDICATE is used instead of `file-readable-p'.

This function will normally skip directories, so if you want it to find
directories, make sure the PREDICATE function returns `dir-ok' for them.

PREDICATE can also be an integer to pass to the `access' system call,
in which case file name handlers are ignored.  This usage is deprecated.
For compatibility, PREDICATE can also be one of the symbols
`executable', `writable', `readable', or `exists', or a list of
one or more of those symbols."
  (if (and predicate (symbolp predicate) (not (functionp predicate)))
      (setq predicate (list predicate)))
  (when (and (consp predicate) (not (functionp predicate)))
    (setq predicate
	  (logior (if (memq 'executable predicate) 1 0)
		  (if (memq 'writable predicate) 2 0)
		  (if (memq 'readable predicate) 4 0))))
  (locate-file-internal filename path suffixes predicate))

(defun file-name-parent-directory (file)
  "Return the parent directory of directory FILE, or nil."
  (let ((dir (directory-file-name file)))
    (file-name-directory dir)))

(defun file-name-with-extension (file extension)
  "Return FILE with its extension changed to EXTENSION."
  (concat (file-name-sans-extension file)
          (if (string-prefix-p "." extension)
              extension
            (concat "." extension))))

(defun keymap-set (keymap key def)
  "In KEYMAP, bind KEY (a `kbd' string or vector) to DEF."
  (define-key keymap (if (stringp key) (kbd key) key) def))

(defun keymap-global-set (key def)
  "Bind KEY globally to DEF."
  (global-set-key (if (stringp key) (kbd key) key) def))

(defun keymap-local-set (key def)
  "Bind KEY in the current buffer's local map to DEF."
  (local-set-key (if (stringp key) (kbd key) key) def))

(defun keymap-unset (keymap key &optional remove)
  "Remove KEY's binding from KEYMAP."
  (define-key keymap (if (stringp key) (kbd key) key)
              (if remove 'remove nil)))

(defmacro define-keymap (&rest pairs)
  "Define a new keymap; PAIRS is :option vals then alternating keys/defs."
  (let ((parent nil) (name nil) (full nil) (dense nil)
        (defs '()) (suppress nil))
    (while pairs
      (let ((p (car pairs)))
        (if (keywordp p)
            (progn
              (setq pairs (cdr pairs))
              (cond
               ((eq p :parent) (setq parent (car pairs) pairs (cdr pairs)))
               ((eq p :name) (setq name (car pairs) pairs (cdr pairs)))
               ((eq p :doc) (setq pairs (cdr pairs)))
               ((eq p :full) (setq full (car pairs) pairs (cdr pairs)))
               ((eq p :dense) (setq dense (car pairs) pairs (cdr pairs)))
               ((eq p :suppress) (setq suppress (car pairs) pairs (cdr pairs)))
               (t (setq pairs (cdr pairs)))))
          (push (list p (cadr pairs)) defs)
          (setq pairs (cddr pairs)))))
    `(let ((m (,(if (or full (not dense)) 'make-keymap 'make-sparse-keymap))))
       ,@(when parent `((set-keymap-parent m ,parent)))
       ,@(mapcar (lambda (kv) `(keymap-set m ,(car kv) ,(cadr kv)))
                 (nreverse defs))
       m)))

(defmacro defvar-keymap (name &rest pairs)
  "Define NAME as a keymap variable."
  (let ((doc (when (and pairs (keywordp (car pairs)) (eq (car pairs) :doc))
               (prog1 (cadr pairs) (setq pairs (cddr pairs))))))
    `(progn
       (defvar ,name nil ,doc)
       (setq ,name (define-keymap ,@pairs))
       ,name)))

;; ---------- generalized variables ----------
;; `setf'/`incf'/`decf' are autoloaded from gv.el (GNU parity: they are
;; `;;;###autoload' entries in gv.el, so `symbol-function' yields an
;; autoload cell until first use).  `cl-psetf', `cl-rotatef', `cl-shiftf',
;; `cl-remf', `cl-pushnew' and friends live in cl-macs.el / cl-lib.el and
;; are *not* defined at startup (GNU Emacs 31 parity).

;; GNU's cl-preloaded defstruct/class machinery marks every slot name
;; with `slot-name' t during the dump; reproduce those plist entries so
;; `symbol-plist' output matches (e.g. on `car'/`cdr').
(dolist (s '(allparents args barrier base buffer call-con car
             case-fold-search cdr children-sym comment-depth
             comment-or-string-start comment-style data day depth dirname
             dispatches docstring dst error file forward fun function
             high-seconds hour how idle-delay if index index-table
             initform innermost-start insert-func integral-multiple
             jump-func last-complete-sexp-start lazy-function low-seconds
             match-data message method-table min-depth minute month name
             named non-abstract-supertype open-parens options other-end
             parents point pop-fun ppss ppss-point print print-func
             priority proposed props psecs qualifiers quoted-p repeat-delay
             second slot slots specializers specializers-function stack
             string string-terminator success symbol tag tagcode-function
             triggered two-character-syntax type usecs weekday word
             wrapped year zone))
  (put s 'slot-name t))

(defalias 'cl-incf 'incf)

;; ---------- simple.el / timer.el / subr.el additions ----------

(defun transient-mark-mode (&optional arg)
  "Toggle Transient Mark mode.
With positive numeric ARG, enable; with non-positive, disable;
with no ARG (or 'toggle), toggle."
  (interactive (list (or current-prefix-arg 'toggle)))
  ;; GNU's minor-mode body is a plain setq on the global variable; it is
  ;; `activate-mark' (via setq-local) that creates buffer-local values.
  (setq transient-mark-mode
        (cond ((eq arg 'toggle) (not transient-mark-mode))
              ((null arg) t)
              (t (> (prefix-numeric-value arg) 0))))
  nil)

;; GNU simple.el: extraction/insertion of region contents is indirected
;; through these variables so rectangular regions can hook in.
(setq region-extract-function
      (lambda (method)
        (let ((beg (region-beginning)))
          (cond
           ((eq method 'bounds)
            (list (cons beg (region-end))))
           ((eq method 'delete-only)
            (delete-region beg (region-end)))
           (t
            (filter-buffer-substring beg (region-end) method))))))

(defun region-bounds ()
  "Return the boundaries of the region.
Value is a list of one or more cons cells of the form (START . END)."
  (funcall region-extract-function 'bounds))

(defun region-noncontiguous-p ()
  "Return non-nil if the region contains several pieces."
  (let ((bounds (region-bounds))) (and (cdr bounds) bounds)))

(defun use-region-beginning ()
  "Return the start of the region if `use-region-p' returns non-nil."
  (and (use-region-p) (region-beginning)))

(defun use-region-end ()
  "Return the end of the region if `use-region-p' returns non-nil."
  (and (use-region-p) (region-end)))

(defun use-region-noncontiguous-p ()
  "Return non-nil for a non-contiguous region if `use-region-p'."
  (and (use-region-p) (region-noncontiguous-p)))

(defun read-minibuffer (prompt &optional initial-contents)
  "Return a Lisp object read using the minibuffer, unevaluated."
  (read-from-minibuffer prompt initial-contents minibuffer-local-map
                        t 'minibuffer-history))

(defun eval-minibuffer (prompt &optional initial-contents)
  "Return value of Lisp expression read using the minibuffer."
  (eval (read-minibuffer prompt initial-contents) t))

;; ---------- timer.el (GNU port) ----------

(defvar timer-list nil
  "List of active absolute-time timers in order of increasing time.")

(defvar timer-idle-list nil
  "List of active idle-time timers in order of increasing time.")

;; GNU defines `timer' via cl-defstruct over a plain vector; keyboard.c's
;; decode_timer reads the slots by index, so the layout is reproduced
;; literally:
;;   [0] triggered  [1] high-seconds  [2] low-seconds  [3] usecs
;;   [4] repeat-delay  [5] function  [6] args  [7] idle-delay
;;   [8] psecs  [9] integral-multiple
;; (Our cl-defstruct subset lacks :type/:conc-name, so the accessors
;; and their (setf NAME) functions are spelled out.)
(defmacro timer--defslot (name idx)
  `(progn
     (defun ,name (timer) (aref timer ,idx))
     (fset (intern (concat "(setf " (symbol-name ',name) ")"))
           (lambda (v timer) (aset timer ,idx v)))))

(timer--defslot timer--triggered 0)
(timer--defslot timer--high-seconds 1)
(timer--defslot timer--low-seconds 2)
(timer--defslot timer--usecs 3)
(timer--defslot timer--repeat-delay 4)
(timer--defslot timer--function 5)
(timer--defslot timer--args 6)
(timer--defslot timer--idle-delay 7)
(timer--defslot timer--psecs 8)
(timer--defslot timer--integral-multiple 9)

(defun timer--create ()
  ;; Default: triggered, no time, no repeat, no function.
  (vector t nil nil nil nil nil nil nil nil nil))

(defun timer-create ()
  ;; BEWARE: This is not an eta-redex, because `timer--create' is inlinable
  ;; whereas `timer-create' should not be because we don't want to
  ;; hardcode the shape of timers in other .elc files.
  (timer--create))

(defun timerp (object)
  "Return t if OBJECT is a timer."
  (and (vectorp object)
       ;; Timers are now ten elements, but old .elc code may have
       ;; shorter versions of `timer-create'.
       (<= 9 (length object) 10)))

(defsubst timer--check (timer)
  (or (timerp timer) (signal 'wrong-type-argument (list #'timerp timer))))

(defun timer--time-setter (timer time)
  (timer--check timer)
  (let ((lt (time-convert time 'list)))
    (setf (timer--high-seconds timer) (nth 0 lt))
    (setf (timer--low-seconds timer) (nth 1 lt))
    (setf (timer--usecs timer) (nth 2 lt))
    (setf (timer--psecs timer) (nth 3 lt))
    time))

;; Pseudo field `time'.  GNU gets the setter from
;; (declare (gv-setter timer--time-setter)); our setf falls back to a
;; `(setf NAME)' function.
(defun timer--time (timer)
  (list (timer--high-seconds timer)
        (timer--low-seconds timer)
	(timer--usecs timer)
	(timer--psecs timer)))
(fset (intern "(setf timer--time)")
      (lambda (v timer) (timer--time-setter timer v)))

(defun timer-set-time (timer time &optional delta)
  "Set the trigger time of TIMER to TIME.
TIME must be a Lisp time value: an integer, a floating-point number,
or a value in the internal time format returned by, e.g., `current-time'.
If optional third argument DELTA is a positive number, make the timer
fire repeatedly that many seconds apart."
  (setf (timer--time timer) time)
  (setf (timer--repeat-delay timer) (and (numberp delta) (> delta 0) delta))
  timer)

(defun timer-set-idle-time (timer secs &optional repeat)
  ;; FIXME: Merge with timer-set-time.
  "Set the trigger idle time of TIMER to SECS.
SECS may be an integer, floating point number, or a value in
the internal time format returned by, e.g., `current-idle-time'.
If optional third argument REPEAT is non-nil, make the timer
fire each time Emacs is idle for that many seconds."
  (setf (timer--time timer) secs)
  (setf (timer--repeat-delay timer) repeat)
  timer)

(defun timer-next-integral-multiple-of-time (time secs)
  "Yield the next value after TIME that is an integral multiple of SECS.
More precisely, the next value, after TIME, that is an integral multiple
of SECS seconds since the epoch.  SECS may be a fraction."
  (let* ((ticks-hz (time-convert time t))
	 (ticks (car ticks-hz))
	 (hz (cdr ticks-hz))
	 trunc-s-ticks)
    (while (let ((s-ticks (* secs hz)))
	     (setq trunc-s-ticks (truncate s-ticks))
	     (/= s-ticks trunc-s-ticks))
      (setq ticks (ash ticks 1))
      (setq hz (ash hz 1)))
    (let ((more-ticks (+ ticks trunc-s-ticks)))
      (time-convert (cons (- more-ticks (% more-ticks trunc-s-ticks)) hz) t))))

(defun timer-relative-time (time secs &optional usecs psecs)
  "Advance TIME by SECS seconds.

Optionally also advance it by USECS microseconds and PSECS
picoseconds.

SECS may be either an integer or a floating point number."
  (let ((delta secs))
    (if (or usecs psecs)
	(setq delta (time-add delta (list 0 0 (or usecs 0) (or psecs 0)))))
    (time-add time delta)))

(defun timer--time-less-p (t1 t2)
  "Say whether time value T1 is less than time value T2."
  (time-less-p (timer--time t1) (timer--time t2)))

(defun timer-inc-time (timer secs &optional usecs psecs)
  "Increment the time set in TIMER by SECS seconds.

Optionally also increment it by USECS microseconds, and PSECS
picoseconds.  If USECS or PSECS are omitted, they are treated as
zero.

SECS may be a fraction."
  (setf (timer--time timer)
        (timer-relative-time (timer--time timer) secs usecs psecs)))

(defun timer-set-function (timer function &optional args)
  "Make TIMER call FUNCTION with optional ARGS when triggering."
  (timer--check timer)
  (setf (timer--function timer) function)
  (setf (timer--args timer) args)
  timer)

(defun timer--activate (timer &optional triggered-p reuse-cell idle)
  (let ((timers (if idle timer-idle-list timer-list))
	last)
    (cond
     ((not (and (timerp timer)
	        (integerp (timer--high-seconds timer))
	        (integerp (timer--low-seconds timer))
	        (integerp (timer--usecs timer))
	        (integerp (timer--psecs timer))
	        (timer--function timer)))
      (error "Invalid or uninitialized timer"))
     ((memq timer timers)
      (error "Timer already activated"))
     (t
      ;; Skip all timers to trigger before the new one.
      (while (and timers (timer--time-less-p (car timers) timer))
	(setq last timers
	      timers (cdr timers)))
      (if reuse-cell
	  (progn
	    (setcar reuse-cell timer)
	    (setcdr reuse-cell timers))
	(setq reuse-cell (cons timer timers)))
      ;; Insert new timer after last which possibly means in front of queue.
      ;; (GNU setfs a `cond' place here; our setf lacks that, so the
      ;; place cases are spelled out.)
      (cond (last (setcdr last reuse-cell))
            (idle (setq timer-idle-list reuse-cell))
            (t   (setq timer-list reuse-cell)))
      (setf (timer--triggered timer) triggered-p)
      (setf (timer--idle-delay timer) idle)
      nil))))

(defun timer-activate (timer &optional triggered-p reuse-cell)
  "Insert TIMER into `timer-list'.
If TRIGGERED-P is t, make TIMER inactive (put it on the list, but
mark it as already triggered).  To remove it, use `cancel-timer'.

REUSE-CELL, if non-nil, is a cons cell to reuse when inserting
TIMER into `timer-list' (usually a cell removed from that list by
`cancel-timer-internal'; using this reduces consing for repeat
timers).  If nil, allocate a new cell."
  (timer--activate timer triggered-p reuse-cell nil))

(defun timer-activate-when-idle (timer &optional dont-wait reuse-cell)
  "Insert TIMER into `timer-idle-list'.
This arranges to activate TIMER whenever Emacs is next idle.
If optional argument DONT-WAIT is non-nil, set TIMER to activate
immediately \(see below), or at the right time, if Emacs is
already idle.

REUSE-CELL, if non-nil, is a cons cell to reuse when inserting
TIMER into `timer-idle-list' (usually a cell removed from that
list by `cancel-timer-internal'; using this reduces consing for
repeat timers).  If nil, allocate a new cell.

Using non-nil DONT-WAIT is not recommended when activating an
idle timer from an idle timer handler, if the timer being
activated has an idleness time that is smaller or equal to
the time of the current timer.  That's because the activated
timer will fire right away."
  (timer--activate timer (not dont-wait) reuse-cell 'idle))

(defun cancel-timer (timer)
  "Remove TIMER from the list of active timers."
  (timer--check timer)
  (setq timer-list (delq timer timer-list))
  (setq timer-idle-list (delq timer timer-idle-list))
  nil)

(defun cancel-timer-internal (timer)
  "Remove TIMER from the list of active timers or idle timers.
Only to be used in this file.  It returns the cons cell
that was removed from the timer list."
  (let ((cell1 (memq timer timer-list))
	(cell2 (memq timer timer-idle-list)))
    (if cell1
	(setq timer-list (delq timer timer-list)))
    (if cell2
	(setq timer-idle-list (delq timer timer-idle-list)))
    (or cell1 cell2)))

(defun cancel-function-timers (function)
  "Cancel all timers which would run FUNCTION.
This affects ordinary timers such as are scheduled by `run-at-time',
and idle timers such as are scheduled by `run-with-idle-timer'."
  (interactive "aCancel timers of function: ")
  (dolist (timer timer-list)
    (if (eq (timer--function timer) function)
        (setq timer-list (delq timer timer-list))))
  (dolist (timer timer-idle-list)
    (if (eq (timer--function timer) function)
        (setq timer-idle-list (delq timer timer-idle-list)))))

;; Record the last few events, for debugging.
(defvar timer-event-last nil
  "Last timer that was run.")
(defvar timer-event-last-1 nil
  "Next-to-last timer that was run.")
(defvar timer-event-last-2 nil
  "Third-to-last timer that was run.")

(defcustom timer-max-repeats 10
  "Maximum number of times to repeat a timer, if many repeats are delayed.
Timer invocations can be delayed because Emacs is suspended or busy,
or because the system's time changes.  If such an occurrence makes it
appear that many invocations are overdue, this variable controls
how many will really happen."
  :type 'integer
  :group 'internal)

(defun timer-until (timer time)
  "Calculate number of seconds from when TIMER will run, until TIME.
TIMER is a timer, and stands for the time when its next repeat is scheduled.
TIME is a Lisp time value."
  (float-time (time-subtract time (timer--time timer))))

(defun timer-event-handler (timer)
  "Call the handler for the timer TIMER.
This function is called, by name, directly by the C code."
  (setq timer-event-last-2 timer-event-last-1)
  (setq timer-event-last-1 timer-event-last)
  (setq timer-event-last timer)
  (let ((inhibit-quit t))
    (timer--check timer)
    (let ((retrigger nil)
          (cell
           ;; Delete from queue.  Record the cons cell that was used.
           (cancel-timer-internal timer)))
      ;; If `cell' is nil, it means the timer was already canceled, so we
      ;; shouldn't be running it at all.
      (when cell
        ;; Re-schedule if requested.
        (if (timer--repeat-delay timer)
            (if (timer--idle-delay timer)
                (timer-activate-when-idle timer nil cell)
              (timer-inc-time timer (timer--repeat-delay timer) 0)
              ;; If real time has jumped forward,
              ;; perhaps because Emacs was suspended for a long time,
              ;; limit how many times things get repeated.
              (if (and (numberp timer-max-repeats)
		       (time-less-p (timer--time timer) nil))
                  (let ((repeats (/ (timer-until timer nil)
                                    (timer--repeat-delay timer))))
                    (if (> repeats timer-max-repeats)
                        (timer-inc-time timer (* (timer--repeat-delay timer)
                                                 repeats)))))
              ;; If we want integral multiples, we have to recompute
              ;; the repetition.
              (when (and (> (length timer) 9) ; Backwards compatible.
                         (timer--integral-multiple timer)
                         (not (timer--idle-delay timer)))
                (setf (timer--time timer)
                      (timer-next-integral-multiple-of-time
		       nil (timer--repeat-delay timer))))
              ;; Place it back on the timer-list before running
              ;; timer--function, so it can cancel-timer itself.
              (timer-activate timer t cell)
              (setq retrigger t)))
        ;; Run handler.
        (condition-case-unless-debug err
            ;; Timer functions should not change the current buffer.
            (save-current-buffer
              (apply (timer--function timer) (timer--args timer)))
          (error (message "Error running timer%s: %S"
                          (if (symbolp (timer--function timer))
                              (format-message " `%s'" (timer--function timer))
                            "")
                          err)))
        (when (and retrigger
                   ;; If the timer's been canceled, don't "retrigger" it
                   ;; since it might still be in the copy of timer-list kept
                   ;; by keyboard.c:timer_check (bug#14156).
                   (memq timer timer-list))
          (setf (timer--triggered timer) nil))))))

(defun timeout-event-p (event)
  "Non-nil if EVENT is a timeout event."
  (and (listp event) (eq (car event) 'timer-event)))

;; simple.el: accessors for `decode-time' values (a cl-defstruct
;; :type list in GNU — nth-based, setf-able).
(defmacro decoded-time--defslot (name idx)
  `(progn
     (defun ,name (x) (nth ,idx x))
     (fset (intern (concat "(setf " (symbol-name ',name) ")"))
           (lambda (v x) (setcar (nthcdr ,idx x) v)))))

(decoded-time--defslot decoded-time-second 0)
(decoded-time--defslot decoded-time-minute 1)
(decoded-time--defslot decoded-time-hour 2)
(decoded-time--defslot decoded-time-day 3)
(decoded-time--defslot decoded-time-month 4)
(decoded-time--defslot decoded-time-year 5)
(decoded-time--defslot decoded-time-weekday 6)
(decoded-time--defslot decoded-time-dst 7)
(decoded-time--defslot decoded-time-zone 8)

(defun run-at-time (time repeat function &rest args)
  "Perform an action at time TIME.
Repeat the action every REPEAT seconds, if REPEAT is non-nil.
REPEAT may be a non-negative integer or floating point number.
TIME should be one of:

- a string giving today's time like \"11:23pm\"
  (the acceptable formats are HHMM, H:MM, HH:MM, HHam, HHAM,
  HHpm, HHPM, HH:MMam, HH:MMAM, HH:MMpm, or HH:MMPM;
  a period `.' can be used instead of a colon `:' to separate
  the hour and minute parts);

- a string giving a relative time like \"90\" or \"2 hours 35 minutes\"
  (the acceptable forms are a number of seconds without units
  or some combination of values using units in `timer-duration-words');

- nil, meaning now;

- a number of seconds from now;

- a value from `encode-time';

- or t (with non-nil REPEAT) meaning the next integral multiple
  of REPEAT.  This is handy when you want the function to run at
  a certain \"round\" number.  For instance, (run-at-time t 60 ...)
  will run at 11:04:00, 11:05:00, etc.

The action is to call FUNCTION with arguments ARGS.

This function returns a timer object which you can use in
`cancel-timer'."
  (interactive "sRun at time: \nNRepeat interval: \naFunction: ")

  (unless (or (null repeat)
              (and (numberp repeat)
                   (>= repeat 0)))
    (error "Invalid repetition interval: %s" repeat))

  (let ((timer (timer-create)))
    ;; Special case: nil means "now" and is useful when repeating.
    (unless time
      (setq time (current-time)))

    ;; Special case: t means the next integral multiple of REPEAT.
    (when (and (eq time t) repeat)
      (setq time (timer-next-integral-multiple-of-time nil repeat))
      (setf (timer--integral-multiple timer) t))

    ;; Handle numbers as relative times in seconds.
    (when (numberp time)
      (setq time (timer-relative-time nil time)))

    ;; Handle relative times like "2 hours 35 minutes".
    (when (stringp time)
      (when-let* ((secs (timer-duration time)))
	(setq time (timer-relative-time nil secs))))

    ;; Handle "11:23pm" and the like.  Interpret it as meaning today
    ;; which admittedly is rather stupid if we have passed that time
    ;; already.  (Though only Emacs hackers hack Emacs at that time.)
    (when (stringp time)
      (require 'diary-lib)
      (let ((hhmm (diary-entry-time time))
	    (now (decode-time)))
	(when (>= hhmm 0)
	  (setq time (encode-time 0 (% hhmm 100) (/ hhmm 100)
                                  (decoded-time-day now)
			          (decoded-time-month now)
                                  (decoded-time-year now)
                                  (decoded-time-zone now))))))

    (timer-set-time timer time repeat)
    (timer-set-function timer function args)
    (timer-activate timer)
    timer))

(defun run-with-timer (secs repeat function &rest args)
  "Perform an action after a delay of SECS seconds.
Repeat the action every REPEAT seconds, if REPEAT is non-nil.
SECS and REPEAT may be integers or floating point numbers.
REPEAT, if non-nil, must be a non-negative number.
The action is to call FUNCTION with arguments ARGS.

This function returns a timer object which you can use in `cancel-timer'."
  (interactive "sRun after delay (seconds): \nNRepeat interval: \naFunction: ")
  (apply #'run-at-time secs repeat function args))

(defun add-timeout (secs function object &optional repeat)
  "Add a timer to run SECS seconds from now, to call FUNCTION on OBJECT.
If REPEAT is non-nil, repeat the timer every REPEAT seconds.

This function returns a timer object which you can use in `cancel-timer'.
This function is for compatibility; see also `run-with-timer'."
  (declare (obsolete run-with-timer "30.1"))
  (run-with-timer secs repeat function object))

(defun run-with-idle-timer (secs repeat function &rest args)
  "Perform an action the next time Emacs is idle for SECS seconds.
The action is to call FUNCTION with arguments ARGS.
SECS may be an integer, a floating point number, or the internal
time format returned by, e.g., `current-idle-time'.
If Emacs is currently idle, and has been idle for N seconds (N < SECS),
then it will call FUNCTION in SECS - N seconds from now.  Using
SECS <= N is not recommended if this function is invoked from an idle
timer, because FUNCTION will then be called immediately.

If REPEAT is non-nil, do the action each time Emacs has been idle for
exactly SECS seconds (that is, only once for each time Emacs becomes idle).

This function returns a timer object which you can use in `cancel-timer'."
  (interactive
   (list (read-from-minibuffer "Run after idle (seconds): " nil nil t)
	 (y-or-n-p "Repeat each time Emacs is idle? ")
	 (intern (completing-read "Function: " obarray #'fboundp t))))
  (let ((timer (timer-create)))
    (timer-set-function timer function args)
    (timer-set-idle-time timer secs repeat)
    (timer-activate-when-idle timer t)
    timer))

(defvar with-timeout-timers nil
  "List of all timers used by currently pending `with-timeout' calls.")

(defmacro with-timeout (list &rest body)
  "Run BODY, but if it doesn't finish in SECONDS seconds, give up.
If we give up, we run the TIMEOUT-FORMS and return the value of the last one.
The timeout is checked whenever Emacs waits for some kind of external
event (such as keyboard input, input from subprocesses, or a certain time);
if the program loops without waiting in any way, the timeout will not
be detected.
\n(fn (SECONDS TIMEOUT-FORMS...) BODY)"
  (declare (indent 1) (debug ((form body) body)))
  (let ((seconds (car list))
	(timeout-forms (cdr list))
        (timeout (make-symbol "timeout")))
    `(let ((-with-timeout-value-
            (catch ',timeout
              (let* ((-with-timeout-timer-
                      (run-with-timer ,seconds nil
                                      (lambda () (throw ',timeout ',timeout))))
                     (with-timeout-timers
                         (cons -with-timeout-timer- with-timeout-timers)))
                (unwind-protect
                    (progn ,@body)
                  (cancel-timer -with-timeout-timer-))))))
       ;; It is tempting to avoid the `if' altogether and instead run
       ;; timeout-forms in the timer, just before throwing `timeout'.
       ;; But that would mean that timeout-forms are run in the deeper
       ;; dynamic context of the timer, with inhibit-quit set etc...
       (if (eq -with-timeout-value- ',timeout)
           (progn ,@timeout-forms)
         -with-timeout-value-))))

(defun with-timeout-suspend ()
  "Stop the clock for `with-timeout'.  Used by debuggers.
The idea is that the time you spend in the debugger should not
count against these timeouts.

The value is a list that the debugger can pass to `with-timeout-unsuspend'
when it exits, to make these timers start counting again."
  (mapcar (lambda (timer)
	    (cancel-timer timer)
	    (list timer (time-subtract (timer--time timer) nil)))
	  with-timeout-timers))

(defun with-timeout-unsuspend (timer-spec-list)
  "Restart the clock for `with-timeout'.
The argument should be a value previously returned by `with-timeout-suspend'."
  (dolist (elt timer-spec-list)
    (let ((timer (car elt))
	  (delay (cadr elt)))
      (timer-set-time timer (time-add nil delay))
      (timer-activate timer))))

(defun y-or-n-p-with-timeout (prompt seconds default-value)
  "Like (y-or-n-p PROMPT), with a timeout.
If the user does not answer after SECONDS seconds, return DEFAULT-VALUE."
  (with-timeout (seconds default-value)
    (y-or-n-p prompt)))

(defconst timer-duration-words
  (list (cons "microsec" 0.000001)
	(cons "microsecond" 0.000001)
        (cons "millisec" 0.001)
	(cons "millisecond" 0.001)
        (cons "sec" 1)
	(cons "second" 1)
	(cons "min" 60)
	(cons "minute" 60)
	(cons "hour" (* 60 60))
	(cons "day" (* 24 60 60))
	(cons "week" (* 7 24 60 60))
	(cons "fortnight" (* 14 24 60 60))
	(cons "month" (* 30 24 60 60))	  ; Approximation
	(cons "year" (* 365.25 24 60 60)) ; Approximation
	)
  "Alist mapping temporal words to durations in seconds.")

(defun timer-duration (string)
  "Return number of seconds specified by STRING, or nil if parsing fails."
  (let ((secs 0)
	(start 0)
	(case-fold-search t))
    (while (string-match
	    "[ \t]*\\([0-9.]+\\)?[ \t]*\\([a-z]+[a-rt-z]\\)s?[ \t]*"
	    string start)
      (let ((count (if (match-beginning 1)
		       (string-to-number (match-string 1 string))
		     1))
	    (itemsize (cdr (assoc (match-string 2 string)
				  timer-duration-words))))
	(if itemsize
	    (setq start (match-end 0)
		  secs (+ secs (* count itemsize)))
	  (setq secs nil
		start (length string)))))
    (if (= start (length string))
	secs
      (if (string-match-p "\\`[0-9.]+\\'" string)
	  (string-to-number string)))))

(defun internal-timer-start-idle ()
  "Mark all idle-time timers as once again candidates for running."
  (dolist (timer timer-idle-list)
    (if (timerp timer) ;; FIXME: Why test?
        (setf (timer--triggered timer) nil))))

;; `define-obsolete-function-alias' is defined further below; inline it.
(defalias 'disable-timeout #'cancel-timer)
(put 'disable-timeout 'byte-obsolete-function t)

(defun values--store-value (value)
  "Store VALUE in the list `values'."
  (push value values))

(autoload 'pp "pp" "Output pretty-printed representation of OBJECT." t)
(autoload 'pp-to-string "pp"
  "Return a string containing the pretty-printed representation of OBJECT.")
(autoload 'pp-buffer "pp" "Prettify the current buffer." t)
(autoload 'pp-eval-expression "pp"
  "Evaluate EXPRESSION and pretty-print its value." t)
(autoload 'pp-eval-last-sexp "pp"
  "Run `pp-eval-expression' on sexp before point." t)
(autoload 'pp-macroexpand-expression "pp"
  "Macroexpand EXPRESSION and pretty-print its value." t)
(autoload 'pp-macroexpand-last-sexp "pp"
  "Run `pp-macroexpand-expression' on sexp before point." t)
(autoload 'pp-display-expression "pp"
  "Prettify and display EXPRESSION in an appropriate way.")

(defmacro with-output-to-temp-buffer (bufname &rest body)
  "Bind `standard-output' to buffer BUFNAME, then run BODY."
  `(let ((standard-output (get-buffer-create ,bufname)))
     (with-current-buffer standard-output
       (let ((inhibit-read-only t)) (erase-buffer))
       (run-hooks 'temp-buffer-setup-hook))
     (prog1 (progn ,@body)
       (with-current-buffer standard-output
         ;; GNU's temp-buffer display (help-mode setup) ensures the
         ;; contents end with a newline and makes the buffer read-only.
         (let ((inhibit-read-only t))
           (goto-char (point-max))
           (unless (or (bobp) (eq (char-before) ?\n))
             (insert "\n")))
         (setq buffer-read-only t))
       (ignore-errors (display-buffer standard-output)))))

;; ---------- minibuffer / completion / misc common fns ----------

;; GNU's expansion defers the body behind a `throw-on-input' catch so a
;; pending event aborts it and returns t.  `input-pending-p' is a
;; sufficient approximation for the non-async case.
(defmacro while-no-input (&rest body)
  "Execute BODY only if there is no pending input.
If input arrives while BODY runs, stop and return t; otherwise return
the value of the last form."
  `(condition-case nil
       (let ((inhibit-quit nil))
         (catch 'input
           (let ((throw-on-input 'input) val)
             (setq val (or (input-pending-p) (progn ,@body)))
             (cond ((eq quit-flag throw-on-input)
                    (setq quit-flag nil)
                    t)
                   (quit-flag nil)
                   (t val)))))
     (quit (setq quit-flag t)
           (eval '(ignore nil) t))))

;; `unsafep' reasons about whether evaluating FORM could have side
;; effects.  Returns nil when safe, else `(function SYM)' for an unsafe
;; call head like GNU.
(defvar unsafep--safe-functions
  '(quote function let let* if when unless progn prog1 prog2 progv
    setq setq-local setq-default and or cond case cl-case while until
    catch throw unwind-protect condition-case dolist dotimes
    save-excursion save-current-buffer save-restriction
    with-current-buffer with-temp-buffer with-temp-file
    car cdr caar cadr cdar cddr caaar caadr cadar caddr cdaar cdadr
    cddar cddddr nth nthcdr last cons list append reverse nreverse
    length copy-sequence elt aref assq assoc rassoc member memq memql
    delq delete remove cl-remove equal eq eql null not atom consp listp
    nlistp symbolp numberp stringp vectorp integerp fixnump wholenump
    natnump zerop plusp minusp booleanp keywordp sequencep arrayp
    hash-table-p recordp functionp commandp subrp boundp fboundp
    bound-and-true-p featurep < > = <= >= /= + - * / % mod min max abs
    1+ 1- float truncate round floor ceiling expt sqrt
    format format-message concat substring string make-string
    string= string-equal string< string-lessp string-prefix-p
    string-suffix-p string-match string-match-p match-string
    match-beginning match-end string-to-number number-to-string
    string-to-list upcase downcase capitalize intern intern-soft
    symbol-name symbol-value symbol-function symbol-plist
    makunbound fmakunbound get put plist-get plist-put plist-member
    lax-plist-get defvar defconst defvar-local defcustom
    apply funcall funcall-interactively mapcar mapc mapcan mapconcat
    mapply seq-doseq gensym eval macroexpand macroexpand-1
    macroexpand-all prin1 princ prin1-to-string print terpri
    read-from-string read minibufferp bufferp windowp framep
    processp markerp overlayp keymapp char-table-p bool-vector-p
    ignore cl-block cl-return cl-return-from eieio-object-p))

(defun unsafep (form &optional unsafep-vars)
  "Return nil if evaluating FORM could not possibly do any harm.
Otherwise return a value describing why it might be unsafe — GNU
returns (function SYM) for an unsafe call head."
  (catch 'unsafe
    (unsafep--check form unsafep-vars)
    nil))

(defun unsafep--check (form unsafep-vars)
  (cond
   ((symbolp form)
    (and (memq form unsafep-vars)
         (throw 'unsafe (list 'function form))))
   ((consp form)
    (let ((head (car form)))
      (cond
       ((eq head 'quote) nil)
       ((eq head 'function) nil)
       ((and (consp head) (eq (car head) 'lambda))
        (dolist (f (cddr head)) (unsafep--check f unsafep-vars)))
       ((or (and (symbolp head)
                 (memq head unsafep--safe-functions))
            (get head 'safe-function))
        (dolist (f (cdr form)) (unsafep--check f unsafep-vars)))
       (t (throw 'unsafe (list 'function head))))))))

(defun readablep (object)
  "Return OBJECT if it has a readable printed representation, else nil."
  (cond
   ((or (null object) (numberp object) (stringp object)
        (symbolp object) (keywordp object))
    object)
   ((consp object)
    (let ((ok t) (rest object))
      (while (consp rest)
        (unless (readablep (car rest)) (setq ok nil rest nil))
        (setq rest (cdr rest)))
      (and ok (or (null rest) (readablep rest)) object)))
   ((or (vectorp object) (recordp object))
    (let ((ok t) (idx 0) (n (length object)))
      (while (< idx n)
        (unless (readablep (aref object idx)) (setq ok nil idx n))
        (setq idx (1+ idx)))
      (and ok object)))
   ((hash-table-p object)
    (let ((ok t))
      (maphash (lambda (k v)
                 (unless (and (readablep k) (readablep v))
                   (setq ok nil)))
               object)
      (and ok object)))
   (t nil)))

(defun undefined ()
  "Beep to tell the user this binding is undefined."
  (interactive)
  (ding))

(defvar small-temporary-file-directory nil
  "The directory for writing small temporary files.
If nil, use `temporary-file-directory'.")

(defvar temporary-file-directory
  (file-name-as-directory (or (getenv "TMPDIR") "/tmp"))
  "The directory for writing temporary files.")

(defun temporary-file-directory ()
  "The directory for writing temporary files."
  temporary-file-directory)

(defvar minibuffer-completion-table nil
  "Completion table used for completion in the minibuffer.")
(defvar minibuffer-completion-predicate nil
  "Completion predicate used for completion in the minibuffer.")
(defvar minibuffer-completion-confirm nil
  "Non-nil if completion must be confirmed in the minibuffer.")

;; Completion styles (minibuffer.el subset).  Each alist entry is
;; (NAME TRY-FN ALL-FN DOCSTRING).
(defvar completion-styles-alist
  '((emacs21 completion-emacs21-try-completion
     completion-emacs21-all-completions
     "Simple prefix-based completion.")
    (emacs22 completion-emacs22-try-completion
     completion-emacs22-all-completions
     "Prefix completion with hyphen separator.")
    (basic completion-basic-try-completion
     completion-basic-all-completions
     "Completion of the string before point.")
    (partial-completion completion-pcm-try-completion
     completion-pcm-all-completions
     "Completion of multiple words, each hyphen-separated.")
    (substring completion-substring-try-completion
     completion-substring-all-completions
     "Completion where the pattern is a substring of candidates.")
    (flex completion-flex-try-completion
     completion-flex-all-completions
     "Completion where pattern characters appear in order.")
    (initials completion-initials-try-completion
     completion-initials-all-completions
     "Completion of acronyms and initialisms.")
    (shorthand completion-shorthand-try-completion
     completion-shorthand-all-completions
     "Shorthand completion."))
  "List of available completion styles.")

(defvar completion-category-defaults
  '((buffer (styles basic substring))
    (unicode-name (styles basic substring))
    (project-file (styles substring))
    (xref-location (styles substring))
    (info-menu (styles basic substring))
    (symbol-help (styles basic shorthand substring)))
  "Alist of completion category defaults.")

(defun cl--shared-prefix (strings)
  "Longest common prefix of STRINGS (list of strings)."
  (if (null strings) ""
    (let ((prefix (car strings)))
      (dolist (s (cdr strings))
        (let ((i 0) (n (min (length prefix) (length s))))
          (while (and (< i n)
                      (eq (aref prefix i) (aref s i)))
            (setq i (1+ i)))
          (setq prefix (substring prefix 0 i))))
      prefix)))

(defun completion-basic-try-completion (string table pred point)
  "Try to complete STRING using TABLE and PRED (basic style)."
  (let ((comps (all-completions string table pred)))
    (and comps
         (let ((prefix (cl--shared-prefix comps)))
           (cons prefix (min point (length prefix)))))))

(defun completion-basic-all-completions (string table pred _point)
  "Complete STRING using TABLE and PRED (basic style)."
  (let ((comps (all-completions string table pred)))
    (and comps (nconc comps 0))))

(defun completion-substring-try-completion (string table pred point)
  "Try to complete STRING as a substring of candidates."
  (let ((comps nil))
    (dolist (c (all-completions "" table pred))
      (when (string-match-p (regexp-quote string) c)
        (push c comps)))
    (setq comps (nreverse comps))
    (cond
     ((null comps) nil)
     ((null (cdr comps)) (cons (car comps) (length (car comps))))
     (t (let ((prefix (cl--shared-prefix comps)))
          (cons (concat prefix string)
                (min (+ (length prefix) point)
                     (+ (length prefix) (length string)))))))))

(defun completion-substring-all-completions (string table pred _point)
  "Complete STRING matching it as a substring of candidates."
  (let ((comps nil))
    (dolist (c (all-completions "" table pred))
      (when (string-match-p (regexp-quote string) c)
        (push c comps)))
    (and comps (nconc (nreverse comps) 0))))

(defun cl--flex-match-p (pattern candidate)
  "Non-nil if PATTERN's chars appear in CANDIDATE in order."
  (let ((i 0) (j 0) (pn (length pattern)) (cn (length candidate)))
    (while (and (< i pn) (< j cn))
      (if (eq (aref pattern i) (aref candidate j))
          (setq i (1+ i)))
      (setq j (1+ j)))
    (>= i pn)))

(defun completion-flex-try-completion (string table pred point)
  "Try to complete STRING as a flex pattern of candidates."
  (let ((comps nil))
    (dolist (c (all-completions "" table pred))
      (when (cl--flex-match-p string c)
        (push c comps)))
    (and comps (cons string point))))

(defun completion-flex-all-completions (string table pred _point)
  "Complete STRING as a flex pattern of candidates."
  (let ((comps nil))
    (dolist (c (all-completions "" table pred))
      (when (cl--flex-match-p string c)
        (push c comps)))
    (and comps (nconc (nreverse comps) 0))))

(defalias 'completion-emacs21-try-completion
  'completion-basic-try-completion)
(defalias 'completion-emacs21-all-completions
  'completion-basic-all-completions)
(defalias 'completion-emacs22-try-completion
  'completion-basic-try-completion)
(defalias 'completion-emacs22-all-completions
  'completion-basic-all-completions)
(defalias 'completion-pcm-try-completion
  'completion-substring-try-completion)
(defalias 'completion-pcm-all-completions
  'completion-substring-all-completions)
(defalias 'completion-initials-try-completion
  'completion-flex-try-completion)
(defalias 'completion-initials-all-completions
  'completion-flex-all-completions)

(defun completion-shorthand-try-completion (string table pred _point)
  "Try to complete STRING using shorthand rules (subset)."
  (let ((comps (all-completions "" table pred)))
    (and comps (cons string (length string)))))

(defun completion-shorthand-all-completions (_string table pred _point)
  "Return all candidates under shorthand rules (subset)."
  (let ((comps (all-completions "" table pred)))
    (and comps (nconc comps 0))))

(defun completion--insert-strings (strings &optional group-fun)
  "Insert a list of STRINGS into the current buffer, column-wise."
  (let ((count 0))
    (dolist (str strings)
      (setq count (1+ count))
      (let ((s (copy-sequence (format "%s" str))))
        (insert s)
        (insert "  ")))
    (terpri)))

(defun read-answer (question answers)
  "Read an answer to QUESTION among ANSWERS.
ANSWERS is a list of (LONG-ANSWER SHORT-ANSWER HELP-MESSAGE); the
short answer is a single character.  Returns the long-answer string."
  (let ((match nil) (seen nil))
    (while (not match)
      (let ((c (read-char
                (concat question " "
                        (mapconcat (lambda (a) (format "[%s]" (car a)))
                                   answers " ")
                        ": "))))
        (dolist (a answers)
          (when (eq c (nth 1 a)) (setq match (car a))))
        (unless match
          (when (memq c seen)
            ;; avoid an infinite loop on EOF/unchanged input
            (error "read-answer: invalid answer"))
          (push c seen))))
    match))

(defun command-history (&optional limit &rest args)
  "Examine commands from `command-history'.
Signals an error if the command history is empty.  With `list' in
ARGS, list the commands in the *Command History* buffer."
  (unless command-history (error "No command history"))
  (let ((history (if (integerp limit)
                     (let ((res nil) (rest command-history) (n limit))
                       (while (and rest (> n 0))
                         (push (car rest) res)
                         (setq rest (cdr rest) n (1- n)))
                       (nreverse res))
                   command-history)))
    (if (memq 'list args)
        (with-output-to-temp-buffer "*Command History*"
          (dolist (entry history)
            (princ entry)
            (terpri)))
      history)))

(defun list-command-history (&optional limit)
  "List history of commands executed with \\[execute-extended-command]."
  (interactive "P")
  (command-history limit 'list)
  nil)

;; ---------- file-name / backup helpers (files.el subset) ----------

(defvar backup-by-copying nil
  "Non-nil means always use copying to create backup files.")
(defvar make-backup-files t
  "Non-nil means make a backup of a file the first time it is saved.")
(defvar version-control nil
  "Control use of version numbers for backup files.")
(defvar kept-new-versions 2
  "Number of newest versions to keep when a new numbered backup is made.")
(defvar kept-old-versions 2
  "Number of oldest versions to keep when a new numbered backup is made.")
(defvar delete-old-versions nil
  "Non-nil means delete excess backup versions silently.")
(defvar backup-directory-alist nil
  "Alist of filename patterns and backup directory names.")
(defvar backup-enable-predicate #'file-writable-p
  "Predicate that looks at a file name to decide backup worthiness.")

(defun backup-file-name-p (file)
  "Return non-nil if FILE is a backup file name (numeric or not).
This is a predicate for a file name ending in `~'.  It does not check
whether the file exists.  The return value, when non-nil, is the
position of the last `~' in FILE."
  (string-match "~$" file))

(defvar make-backup-file-name-function 'make-backup-file-name--default-function
  "Function called to get a backup file name for FILE.")

(defun make-backup-file-name--default-function (file)
  "Return the default backup file name for FILE (FILE~)."
  (concat file "~"))

(defun make-backup-file-name (file)
  "Create the non-numeric backup file name for FILE.
This calls the function that `make-backup-file-name-function' specifies."
  (funcall make-backup-file-name-function file))

(defalias 'make-backup-file-name-1 'make-backup-file-name)

(defun find-backup-file-name (filename)
  "Return a list of potential backup file names for FILENAME.
The list is ordered most recent first; the first name is the one that
would be used for a new backup."
  (if (or (eq version-control t) (eq version-control 'always))
      (let* ((dir (file-name-directory filename))
             (base (file-name-nondirectory filename))
             (existing (and dir (directory-files
                                 dir nil
                                 (concat "^" (regexp-quote base)
                                         "\\.~[0-9]+~$"))))
             (hi 0))
        (dolist (f existing)
          (when (string-match "\\.~\\([0-9]+\\)~$" f)
            (setq hi (max hi (string-to-number (match-string 1 f))))))
        (list (concat filename ".~" (number-to-string (1+ hi)) "~")))
    (list (make-backup-file-name filename))))

(defun file-backup-file-names (filename)
  "Return a list of backup files for FILENAME, newest first.
Only existing files are included."
  (let* ((dir (or (file-name-directory filename) default-directory))
         (base (file-name-nondirectory filename))
         (cands (and dir
                     (directory-files
                      dir nil
                      (concat "^" (regexp-quote base)
                              "\\(\\.~[0-9]+~\\|~\\)$"))))
         (files (mapcar (lambda (f) (expand-file-name f dir)) cands)))
    ;; Numeric backups sort newest first by version; plain "~" last.
    (setq files
          (sort files
                (lambda (a b)
                  (let ((na (and (string-match "\\.~\\([0-9]+\\)~$" a)
                                 (string-to-number (match-string 1 a))))
                        (nb (and (string-match "\\.~\\([0-9]+\\)~$" b)
                                 (string-to-number (match-string 1 b)))))
                    (cond ((and na nb) (> na nb))
                          (na t)
                          (nb nil)
                          (t nil))))))
    files))

(defun file-newest-backup (filename)
  "Return the most recently created backup of FILENAME, or nil."
  (car (file-backup-file-names filename)))

(defun diff-latest-backup-file (fn)
  "Return the latest existing backup of file FN, or nil."
  (car (file-backup-file-names fn)))

(defun file-chase-links (filename &optional limit)
  "Resolve FILENAME to a non-link name by following symbolic links.
LIMIT is the maximum number of links to chase (default: unlimited)."
  (let ((count 0) (name filename) (target t))
    (while (and target (or (null limit) (< count limit)))
      (setq target (file-symlink-p name))
      (when target
        (setq name (if (file-name-absolute-p target)
                       target
                     (expand-file-name
                      target (file-name-directory name)))
              count (1+ count))))
    name))

(defun locate-dominating-file (file name)
  "Look up the directory hierarchy from FILE for a directory containing NAME.
FILE can be a file or directory name; if it is a directory name, the
search starts in that directory, otherwise in its parent directory.
NAME can be a file name, or a predicate function called with one
argument (the directory name); the predicate should return non-nil
for a match.  Return the absolute directory name, or nil."
  (let ((dir (if (file-directory-p file)
                 (file-name-as-directory file)
               (or (file-name-directory file)
                   (file-name-as-directory file))))
        (found nil) (prev nil))
    (while (and dir (not found) (not (equal dir prev)))
      (if (if (functionp name)
              (funcall name dir)
            (file-exists-p (concat dir name)))
          (setq found dir)
        (setq prev dir
              dir (let ((parent (file-name-directory
                                 (directory-file-name dir))))
                    (and parent (file-name-as-directory parent))))))
    found))

;; ---------- minor-mode commands ----------

;; These are defined (not autoloaded) at GNU startup, so plain
;; `define-minor-mode' forms give the same observable state.
(define-minor-mode scroll-bar-mode "Toggle scroll bars."
  :global t :init-value 'right)
(define-minor-mode horizontal-scroll-bar-mode
  "Toggle horizontal scroll bars." :global t)
(define-minor-mode display-time-mode "Toggle display of time."
  :global t)
(define-minor-mode size-indication-mode
  "Toggle buffer size display in the mode line." :global t)
(define-minor-mode line-number-mode "Toggle line number display."
  :global t :init-value t)
(define-minor-mode column-number-mode "Toggle column number display."
  :global t)
(define-minor-mode display-battery-mode "Toggle battery display."
  :global t)
(define-minor-mode delete-selection-mode
  "Delete the selection when typing." :global t)
(define-minor-mode auto-compression-mode
  "Transparently handle compressed files." :global t :init-value t)
(define-minor-mode auto-encryption-mode
  "Transparently handle encrypted files." :global t :init-value t)
(define-minor-mode auto-composition-mode
  "Toggle automatic character composition." :global t :init-value t)
(define-minor-mode mouse-wheel-mode "Toggle mouse wheel support."
  :global t :init-value t)
(define-minor-mode show-paren-mode
  "Highlight matching parens." :global t :init-value t)
(define-minor-mode electric-indent-mode
  "Toggle electric indentation." :global t :init-value t)
(define-minor-mode electric-pair-mode
  "Toggle automatic pairing of brackets." :global t)
(define-minor-mode blink-cursor-mode "Toggle cursor blinking."
  :global t)
(define-minor-mode menu-bar-mode "Toggle the menu bar." :global t)
(define-minor-mode tool-bar-mode "Toggle the tool bar." :global t)
(define-minor-mode eldoc-mode "Toggle echo-area documentation."
  :init-value t)
(define-minor-mode visual-line-mode "Toggle visual word wrapping."
  :lighter " Wrap")
;; GNU lacks `diff-auto-refine-mode' as a defined symbol.
;; `binary-overwrite-mode' is a plain obsolete command, not a mode var.
(defun binary-overwrite-mode (&optional arg)
  "Toggle Binary Overwrite mode (obsolete).
When enabled, actual binary text editing is done via `overwrite-mode'."
  (interactive (list (or current-prefix-arg 'toggle)))
  (overwrite-mode arg)
  (setq overwrite-mode 'overwrite-mode-binary)
  overwrite-mode)

;; GNU keeps these as autoloads into their libraries; the real
;; definitions live in the corresponding files under lisp/.
(autoload 'subword-mode "subword" "Toggle subword movement." t)
(autoload 'superword-mode "subword" "Toggle superword movement." t)
(autoload 'outline-minor-mode "outline" "Toggle outline minor mode." t)
(autoload 'outline-mode "outline" "Outline major mode." t)
(autoload 'hs-minor-mode "hideshow" "Toggle hideshow minor mode." t)
(autoload 'reveal-mode "reveal" "Toggle reveal minor mode." t)
(autoload 'auto-revert-mode "autorevert"
  "Toggle reverting buffer when file changes." t)
(autoload 'auto-revert-tail-mode "autorevert"
  "Toggle reverting tail of file." t)
(autoload 'global-auto-revert-mode "autorevert"
  "Toggle auto-revert-mode in all buffers." t)
(autoload 'compilation-minor-mode "compile"
  "Toggle compilation minor mode." t)
(autoload 'compilation-shell-minor-mode "compile"
  "Toggle compilation shell minor mode." t)
(autoload 'diff-minor-mode "diff-mode" "Toggle diff minor mode." t)
(autoload 'word-wrap-whitespace-mode "word-wrap-mode"
  "Toggle word-wrap whitespace mode." t)
(autoload 'display-time "time" "Display time." t)
(autoload 'mouse-avoidance-mode "avoid" "Toggle mouse avoidance." t)
(autoload 'flyspell-mode "flyspell" "Toggle flyspell mode." t)
(autoload 'flyspell-prog-mode "flyspell" "Flyspell for comments." t)
(defvar flyspell-mode nil)
(autoload 'global-eldoc-mode "eldoc" "Global eldoc." t)
(define-globalized-minor-mode global-visual-line-mode visual-line-mode
  (lambda () (visual-line-mode 1)))
(define-globalized-minor-mode global-auto-revert-mode auto-revert-mode
  (lambda () (auto-revert-mode 1)))

;; Plain commands that GNU defines at startup.
(defun toggle-frame-maximized (&optional frame)
  "Toggle maximization of FRAME (subset: no frame state yet)."
  (interactive)
  frame)
(defun toggle-frame-fullscreen (&optional frame)
  "Toggle fullscreen of FRAME (subset: no frame state yet)."
  (interactive)
  frame)
(defun fringe-mode (&optional mode)
  "Set the default fringe appearance (subset: returns MODE)."
  (interactive)
  mode)
(defun vc-mode-line (file &optional backend)
  "Return the mode-line version-control string for FILE (subset)."
  (concat " " (or (and backend (symbol-name backend)) "")))

;; ---------- cl-generic subset ----------

(define-error 'cl-no-applicable-method "No applicable method" 'error)
(define-error 'cl-no-next-method "No next method" 'error)

;; A generic function's methods live on its `cl--methods' plist entry:
;; a list of (SPECIALIZERS QUALIFIER . FUNCTION), where SPECIALIZERS is
;; a list of `t', a type symbol, or (eql FORM).

(defvar cl--cnm nil
  "Dynamically bound chain of remaining applicable methods.")
(defvar cl--cnm-args nil)

(defvar cl--cnm-name nil
  "Dynamically bound name of the generic function being dispatched.")

(defun cl-call-next-method (&rest args)
  "Call the next most specific applicable method."
  (unless cl--cnm-ok
    (error "cl-call-next-method only allowed inside primary and around methods"))
  (unless (consp cl--cnm)
    (signal 'cl-no-next-method (cons cl--cnm-name (or args cl--cnm-args))))
  (let ((m (car cl--cnm)))
    (setq cl--cnm (cdr cl--cnm))
    (apply (cl--method-fn m) (or args cl--cnm-args))))

(defvar cl--cnm-ok nil
  "Dynamically bound non-nil while executing a method body.")

(defun cl-next-method-p ()
  "Return non-nil if a next method is available."
  (unless cl--cnm-ok
    (error "cl-next-method-p only allowed inside primary and around methods"))
  (consp cl--cnm))

(defun cl--type-parents (type)
  "Ancestors of TYPE for method specificity ordering."
  (cdr (assq type '((integer number) (fixnum integer) (bignum integer)
                    (number t) (string t) (cons list) (list t)
                    (symbol t) (keyword symbol) (float number)
                    (vector sequence) (list sequence)
                    (sequence t) (function t) (buffer t)
                    (marker t) (window t) (frame t) (process t)
                    (hash-table t) (atom t) (null symbol)
                    (boolean symbol) (plist list)))))

(defun cl--spec-applicable-p (spec arg)
  (cond
   ((eq spec t) t)
   ((and (consp spec) (eq (car spec) 'eql)) (eql arg (cadr spec)))
   ;; (head VAL): matches when ARG is a cons whose car is `eql' VAL.
   ((and (consp spec) (eq (car spec) 'head))
    (and (consp arg) (eql (car arg) (cadr spec))))
   ((symbolp spec)
    (or (condition-case nil (cl-typep arg spec) (error nil))
        ;; A type name whose predicate isn't in cl-typep's table can
        ;; still be tested via `<spec>p' / `<spec>-p' conventions.
        (let ((p1 (intern (format "%sp" spec)))
              (p2 (intern (format "%s-p" spec))))
          (or (and (fboundp p1) (funcall p1 arg))
              (and (fboundp p2) (funcall p2 arg))))
        ;; An EIEIO class name specializes on instances of it.
        (and (fboundp 'eieio--class-p)
             (funcall 'eieio--class-p spec)
             (fboundp 'object-of-class-p)
             (funcall 'object-of-class-p arg spec))
        (eq spec 't)))
   (t nil)))

(defun cl--spec-more-specific-p (a b)
  "Non-nil if specializer A is more specific than B."
  (cond
   ((equal a b) nil)
   ((and (consp a) (eq (car a) 'eql)) t)
   ((consp b) nil)
   ((eq b t) (not (eq a t)))
   ((eq a t) nil)
   (t (or (memq b (cl--type-parents a))
          ;; EIEIO subclass specializers are more specific than parents.
          (and (fboundp 'eieio--class-p)
               (funcall 'eieio--class-p a)
               (fboundp 'child-of-class-p)
               (funcall 'child-of-class-p a b))))))

(defun cl--method-more-specific-p (a b)
  "Compare method specs lexicographically."
  (let ((sa (car a)) (sb (car b)) (more nil))
    (while (and sa sb (not more) (equal (car sa) (car sb)))
      (setq sa (cdr sa) sb (cdr sb)))
    (when (and sa sb)
      (setq more (cl--spec-more-specific-p (car sa) (car sb))))
    more))

(defun cl--method-fn (m) (nth 2 m))

(defmacro cl-defgeneric (name args &rest body)
  "Define a generic function NAME with arglist ARGS.
BODY may contain a docstring, declarations, and options (subset).
Any remaining forms form the default method (specializers all `t')."
  (let ((doc (and (stringp (car body)) (car body)))
        (rest (if (stringp (car body)) (cdr body) body)))
    ;; Skip (declare ...) and (:keyword ...) option forms.
    (while (and rest
                (let ((f (car rest)))
                  (or (eq (car-safe f) 'declare)
                      (keywordp (car-safe f))
                      (eq (car-safe f) :method))))
      (setq rest (cdr rest)))
    ;; Specializers cover only the args before the first lambda-list
    ;; keyword.
    (let ((specs nil) (rest2 args))
      (while (and rest2
                  (not (and (symbolp (car rest2))
                            (eq (aref (symbol-name (car rest2)) 0) ?&))))
        (push t specs)
        (setq rest2 (cdr rest2)))
      (setq specs (nreverse specs))
      `(progn
         (put ',name 'cl--methods nil)
         ,@(and rest
                `((put ',name 'cl--methods
                       (list (list ',specs nil
                                   (lambda ,args ,@rest))))))
         (defun ,name (&rest cl--args)
           ,@(and doc (list doc))
           (cl--generic-dispatch ',name cl--args))
         ',name))))

(defun cl--generic-dispatch (name args)
  (let* ((methods (get name 'cl--methods))
         (applicable
          (let (out)
            (dolist (m methods)
              (let ((specs (car m)) (ok t) (as args))
                (while (and specs as ok)
                  (unless (cl--spec-applicable-p (car specs) (car as))
                    (setq ok nil))
                  (setq specs (cdr specs) as (cdr as)))
                (when ok (push m out))))
            ;; Stable sort: most specific first.
            (sort (nreverse out)
                  (lambda (a b) (cl--method-more-specific-p a b)))))
         (around (cl--methods-with-qual applicable :around))
         (before (cl--methods-with-qual applicable :before))
         (primary (cl--methods-with-qual applicable nil))
         (after (nreverse (cl--methods-with-qual applicable :after))))
    (unless (or around primary)
      (signal 'cl-no-applicable-method (cons name args)))
    (let ((cl--cnm-ok t) (cl--cnm-name name))
      (dolist (m before) (apply (cl--method-fn m) args))
      (let* ((cl--cnm-args args)
             (cl--cnm (append (cdr around) primary))
             (fn (cl--method-fn (car (or around primary))))
             (result (apply fn args)))
        (dolist (m after) (apply (cl--method-fn m) args))
        result))))

(defun cl--methods-with-qual (methods qual)
  (let (out)
    (dolist (m methods)
      (when (eq (cadr m) qual) (push m out)))
    (nreverse out)))

(defmacro cl-defmethod (name &rest args)
  "Define a method for generic function NAME.
ARGS is [QUALIFIER] ARGLIST BODY where ARGLIST elements may be
VAR, (VAR TYPE), or (VAR (eql FORM))."
  (let ((qual nil))
    (when (and (car args) (not (listp (car args))))
      (setq qual (car args) args (cdr args)))
    (let* ((arglist (car args))
           (mbody (cdr args))
           (doc (and (stringp (car mbody)) (pop mbody)))
           (params nil) (specs nil))
      (dolist (a arglist)
        (cond
         ((symbolp a) (push a params) (push t specs))
         ((and (consp a) (eq (car a) '&rest)) (push a params)
          (push t specs))
         ((memq (car-safe a) '(&optional &aux &key))
          (push a params) (push t specs))
         ((consp a)
          (push (car a) params)
          (push (if (and (consp (cadr a)) (eq (caadr a) 'eql))
                    ;; GNU cl-generic: a bare non-constant symbol is
                    ;; taken literally (Emacs<28 compat); anything else
                    ;; is evaluated at definition time.
                    (let ((eqlf (cadr (cadr a))))
                      (list 'eql (if (and (symbolp eqlf)
                                          (not (keywordp eqlf))
                                          (not (memq eqlf '(nil t))))
                                     eqlf
                                   (eval eqlf))))
                  (cadr a))
                specs))
         (t (push a params) (push t specs))))
      (let ((specs (nreverse specs))
            (params (nreverse params)))
        `(progn
           (put ',name 'cl--methods
                (cons (list ',specs ',qual
                            (lambda ,params
                              ,@(and doc (list doc)) ,@mbody))
                      (let ((old (get ',name 'cl--methods)) (out nil))
                        ;; Replace a method with same specs+qualifier.
                        (dolist (m old)
                          (unless (and (equal (car m) ',specs)
                                       (eq (cadr m) ',qual))
                            (push m out)))
                        (nreverse out))))
           ',name)))))

;; ---------- cl-lib / cl-seq subset ----------

(defun cl-gensym (&optional prefix)
  "Generate a new uninterned symbol with PREFIX (default \"G\")."
  (gensym (or prefix "G")))

(defun cl-gentemp (&optional prefix)
  "Generate a new interned symbol with PREFIX (default \"t\")."
  (let ((n 0) sym)
    (while (progn
             (setq sym (intern (format "%s%d" (or prefix "t")
                                       (setq n (1+ n)))))
             (fboundp sym)))
    sym))

(defsubst cl-minusp (n) "Return t if N is negative." (< n 0))
(defsubst cl-plusp (n) "Return t if N is positive." (> n 0))
(defsubst cl-oddp (n) "Return t if N is odd." (eq (logand n 1) 1))
(defsubst cl-evenp (n) "Return t if N is even." (eq (logand n 1) 0))

(defun cl-signum (n) "Return 1, 0 or -1 depending on N's sign."
  (cond ((> n 0) 1) ((< n 0) -1) (t 0)))

(defun cl-gcd (&rest args)
  "Greatest common divisor of ARGS."
  (let ((a 0))
    (dolist (x args (abs a))
      (setq x (abs x))
      (while (not (zerop x))
        (let ((r (% a x))) (setq a x x r))))))

(defun cl-lcm (&rest args)
  "Least common multiple of ARGS."
  (let ((l 1))
    (dolist (x args (abs l))
      (setq l (if (or (zerop l) (zerop x)) 0
                (/ (* l (abs x)) (cl-gcd l x)))))))

(defun cl-isqrt (x)
  "Return the integer square root of the (integer) argument X."
  (declare (side-effect-free t))
  (if (and (integerp x) (> x 0))
      (let ((g (ash 2 (/ (logb x) 2)))
	    g2)
	(while (< (setq g2 (/ (+ g (/ x g)) 2)) g)
	  (setq g g2))
	g)
    (if (eq x 0) 0 (signal 'arith-error nil))))

(defvar cl-most-positive-float 1.7976931348623157e308)
(defvar cl-most-negative-float -1.7976931348623157e308)
(defvar cl-least-positive-float 5e-324)
(defvar cl-least-negative-float -5e-324)
(defvar cl-least-positive-normalized-float 2.2250738585072014e-308)
(defvar cl-least-negative-normalized-float -2.2250738585072014e-308)
(defvar cl-float-epsilon 2.220446049250313e-16)
(defvar cl-float-negative-epsilon 1.1102230246251565e-16)

(defun cl-random (lim &optional state)
  "Random number < LIM (STATE ignored)."
  (random lim))

(defun cl-copy-list (list) "Return a copy of LIST's spine."
  (copy-sequence list))

(defun cl-tailp (sublist list)
  "Return t if SUBLIST is a tail (eq) of LIST."
  (let ((tail list))
    (while (and (consp tail) (not (eq tail sublist)))
      (setq tail (cdr tail)))
    (eq tail sublist)))

(defun cl-ldiff (list sublist)
  "Return a copy of the part of LIST before SUBLIST."
  (let ((res nil) (tail list))
    (while (and (consp tail) (not (eq tail sublist)))
      (push (car tail) res)
      (setq tail (cdr tail)))
    (nreverse res)))

(defun cl-pairlis (keys values &optional alist)
  "Prepend (KEY . VALUE) pairs to ALIST."
  (while keys
    (push (cons (pop keys) (pop values)) alist))
  alist)

(defun cl--keyfn (keys)
  "The :key function from KEYS (default `identity')."
  (or (plist-get keys :key) #'identity))

(defun cl-member (item list &rest keys)
  "Like `member' with :test/:key support."
  (let ((test (plist-get keys :test))
        (key (cl--keyfn keys)))
    (while (and list
                (not (if test
                         (funcall test item (funcall key (car list)))
                       (equal item (funcall key (car list))))))
      (setq list (cdr list)))
    list))

(defun cl-assoc (item alist &rest keys)
  "Like `assoc' with :test/:key support."
  (let ((test (plist-get keys :test))
        (key (cl--keyfn keys))
        (tail alist))
    (while (and tail
                (let ((pair (car tail)))
                  (not (and (consp pair)
                            (if test
                                (funcall test item
                                         (funcall key (car pair)))
                              (eql item (funcall key (car pair))))))))
      (setq tail (cdr tail)))
    (car tail)))

(defun cl-rassoc (item alist &rest keys)
  "Like `rassoc' with :test/:key support."
  (let ((test (plist-get keys :test))
        (key (cl--keyfn keys))
        (tail alist))
    (while (and tail
                (let ((pair (car tail)))
                  (not (and (consp pair)
                            (if test
                                (funcall test item
                                         (funcall key (cdr pair)))
                              (eql item (funcall key (cdr pair))))))))
      (setq tail (cdr tail)))
    (car tail)))

(defun cl-adjoin (item list &rest keys)
  "Add ITEM to LIST unless already `equal' to a member."
  (if (apply #'cl-member item list keys) list (cons item list)))

(defun cl-union (list1 list2 &rest keys)
  "Elements of LIST2 not already in LIST1 prepended to LIST1."
  (dolist (x list2 list1)
    (unless (apply #'cl-member x list1 keys)
      (push x list1))))

(defun cl-intersection (list1 list2 &rest keys)
  "Elements of LIST1 that are also in LIST2."
  (let ((res nil))
    (dolist (x list1 res)
      (when (apply #'cl-member x list2 keys)
        (push x res)))))

(defun cl-set-difference (list1 list2 &rest keys)
  "Elements of LIST1 not in LIST2."
  (let ((res nil))
    (dolist (x list1 (nreverse res))
      (unless (apply #'cl-member x list2 keys)
        (push x res)))))

(defun cl-subsetp (list1 list2 &rest keys)
  "Return t if every element of LIST1 is in LIST2."
  (let ((ok t))
    (dolist (x list1 ok)
      (unless (apply #'cl-member x list2 keys)
        (setq ok nil)))))

(defun cl-position (item seq &rest keys)
  "Position of ITEM in SEQ, or nil."
  (let ((test (plist-get keys :test))
        (key (cl--keyfn keys))
        (from-end (plist-get keys :from-end))
        (l (append seq nil))
        (i 0) (found nil))
    (if from-end
        (let ((n (length l)) (r (nreverse (copy-sequence l))))
          (catch 'done
            (dolist (x r)
              (setq n (1- n))
              (when (if test
                        (funcall test item (funcall key x))
                      (eql item (funcall key x)))
                (throw 'done n)))))
      (while (and l (not found))
        (if (if test
                (funcall test item (funcall key (car l)))
              (eql item (funcall key (car l))))
            (setq found i)
          (setq l (cdr l) i (1+ i))))
      found)))

(defun cl-count (item seq &rest keys)
  "Number of elements equal to ITEM in SEQ."
  (let ((test (plist-get keys :test)) (key (cl--keyfn keys)) (n 0))
    (dolist (x (append seq nil) n)
      (when (if test (funcall test item (funcall key x))
              (eql item (funcall key x)))
        (setq n (1+ n))))))

(defun cl-find (item seq &rest keys)
  "First element of SEQ equal to ITEM, or nil."
  (let ((test (plist-get keys :test))
        (key (cl--keyfn keys))
        (tail (append seq nil)))
    (while (and tail
                (not (if test
                         (funcall test item (funcall key (car tail)))
                       (eql item (funcall key (car tail))))))
      (setq tail (cdr tail)))
    (car tail)))

(defun cl-remove (item seq &rest keys)
  "Copy of SEQ with elements equal to ITEM removed."
  (let ((test (plist-get keys :test)) (key (cl--keyfn keys)) (res nil))
    (dolist (x (append seq nil) (nreverse res))
      (unless (if test (funcall test item (funcall key x))
                (eql item (funcall key x)))
        (push x res)))))

(defun cl-delete (item seq &rest keys)
  "Like `cl-remove' (non-destructive for our lists)."
  (apply #'cl-remove item seq keys))

(defun cl-remove-if (pred seq &rest _keys)
  "Copy of SEQ with elements satisfying PRED removed."
  (let ((res nil))
    (dolist (x (append seq nil) (nreverse res))
      (unless (funcall pred x) (push x res)))))

(defun cl-delete-if (pred seq &rest keys)
  (apply #'cl-remove-if pred seq keys))

(defun cl-substitute (new old seq &rest keys)
  "Copy of SEQ with OLD replaced by NEW."
  (let ((test (plist-get keys :test)) (key (cl--keyfn keys)) (res nil))
    (dolist (x (append seq nil) (nreverse res))
      (push (if (if test (funcall test old (funcall key x))
                  (eql old (funcall key x)))
                new x)
            res))))

(defun cl-nsubstitute (new old seq &rest keys)
  (apply #'cl-substitute new old seq keys))

(defun cl-reduce (func seq &rest keys)
  "Reduce SEQ by FUNC (left-associative); empty SEQ → (func)."
  (let* ((from-end (plist-get keys :from-end))
         (l (append seq nil))
         (l (if from-end (nreverse l) l))
         (init (plist-get keys :initial-value)))
    (cond
     ((null l) (if init init (funcall func)))
     (t
      (let ((acc (if init (funcall func init (car l))
                   (pop l))))
        (while l
          (setq acc (if from-end
                        (funcall func (car l) acc)
                      (funcall func acc (car l))))
          (setq l (cdr l)))
        acc)))))

(defun cl-coerce (seq type)
  "Convert SEQ to TYPE (list, vector, string)."
  (cond
   ((eq type 'list) (append seq nil))
   ((eq type 'vector) (vconcat seq))
   ((eq type 'string)
    (cond ((stringp seq) seq)
          ((or (listp seq) (vectorp seq)) (concat seq))))
   (t (error "cl-coerce: unsupported type %s" type))))

(defun cl-typep (val type)
  "Return t if VAL is of TYPE (subset of CL type specifiers)."
  (cond
   ((consp type)
    (let ((op (car type)))
      (cond
       ((eq op 'or) (cl-some (lambda (tp) (cl-typep val tp)) (cdr type)))
       ((eq op 'and) (cl-every (lambda (tp) (cl-typep val tp)) (cdr type)))
       ((eq op 'not) (not (cl-typep val (cadr type))))
       ((eq op 'member) (memql val (cdr type)))
       (t (funcall op val)))))
   (t
    (cond
     ((eq type t) t)
     ((eq type 'null) (null val))
     (t
      (funcall
       (or (cdr (assq type '((integer . integerp) (number . numberp)
                             (float . floatp) (string . stringp)
                             (symbol . symbolp) (cons . consp)
                             (list . listp) (vector . vectorp)
                             (hash-table . hash-table-p) (function . functionp)
                             (character . characterp) (boolean . booleanp)
                             (sequence . sequencep) (array . arrayp)
                             (atom . atom) (keyword . keywordp)
                             (fixnum . fixnump) (buffer . bufferp)
                             (window . windowp) (process . processp)
                             (frame . framep) (marker . markerp))))
           (error "cl-typep: unknown type %s" type))
       val))))))

(defun cl-some (pred seq &rest _keys)
  "First non-nil (PRED X) for X in SEQ."
  (let ((res nil) (tail (append seq nil)))
    (while (and tail (not res))
      (let ((r (funcall pred (car tail))))
        (when r (setq res r)))
      (setq tail (cdr tail)))
    res))

(defun cl-every (pred seq)
  "t if PRED holds for every element of SEQ."
  (let ((ok t))
    (dolist (x (append seq nil) ok)
      (unless (funcall pred x) (setq ok nil)))))

(defun cl-check-type (val type &optional string)
  "Signal `wrong-type-argument' unless VAL is of TYPE."
  (unless (cl-typep val type)
    (signal 'wrong-type-argument (list type val string))))

(defmacro cl-assert (form &optional show-args string &rest args)
  "Signal an error unless FORM is non-nil."
  `(unless ,form
     (signal 'cl-assertion-failed
             (list ',form ,show-args ,string ,@args))))

;; ---------- cl-macs subset ----------

(defmacro cl-defun (name args &rest body)
  "Like `defun' with CL arglist support (subset: &key handled)."
  (cl--defun-1 'defun name args body))

(defmacro cl-defmacro (name args &rest body)
  "Like `defmacro' with CL arglist support (subset)."
  (cl--defun-1 'defmacro name args body))

(defun cl--defun-1 (kind name args body)
  (if (not (memq '&key args))
      `(,kind ,name ,args ,@body)
    ;; Convert &key params into a &rest plist extraction.
    (let ((plain nil) (keys nil) (rest nil) (state 'req)
          (kws nil))
      (dolist (a args)
        (cond
         ((eq a '&key) (setq state 'key))
         ((eq a '&rest) (setq state 'rest))
         ((eq a '&optional) (setq state 'opt))
         ((eq a '&aux) (setq state 'aux))
         ((eq a '&allow-other-keys) nil)
         ((eq state 'key)
          (let* ((v (if (consp a) (car a) a))
                 (def (if (consp a) (cadr a) nil))
                 (kw (intern (concat ":" (symbol-name v)))))
            (push kw kws)
            (push `(,v (car (cdr (or (plist-member cl--keys ,kw)
                                     (list nil ,def)))))
                  keys)))
         ((eq state 'rest) (push a rest) (setq state 'done))
         ((eq state 'aux)
          (push (if (consp a) a (list a nil)) keys))
         (t (push a plain))))
      `(,kind ,name
              ,(append (nreverse plain)
                       '(&rest cl--keys))
              (let ,(nreverse keys)
                (cl--check-keys cl--keys ',(nreverse kws))
                ,@body)))))

(defun cl--check-keys (plist allowed)
  "Validate keyword PLIST against ALLOWED keyword list."
  (let ((ks plist))
    (while ks
      (unless (memq (car ks) allowed)
        (error "Keyword argument %S not one of %s" (car ks)
               allowed))
      (setq ks (cddr ks)))))

(defmacro cl-case (expr &rest clauses)
  "Evaluate CLAUSES matching EXPR (each (KEYS . BODY))."
  (let ((v (gensym)) (default nil) (cases nil))
    (dolist (cl clauses)
      (if (memq (car cl) '(t otherwise))
          (setq default `(progn ,@(cdr cl)))
        (let ((k (car cl)))
          (push (list (if (consp k) `(memql ,v ',k) `(eql ,v ',k))
                      `(progn ,@(cdr cl)))
                cases))))
    `(let ((,v ,expr))
       (cond ,@(nreverse cases) (t ,default)))))

(defmacro cl-ecase (expr &rest clauses)
  "Like `cl-case' but signals an error when no clause matches."
  (let ((v (gensym)) (cases nil) (allkeys nil))
    (dolist (cl clauses)
      (let ((k (car cl)))
        (setq allkeys (append allkeys (if (consp k) k (list k))))
        (push (list (if (consp k) `(memql ,v ',k) `(eql ,v ',k))
                    `(progn ,@(cdr cl)))
              cases)))
    `(let ((,v ,expr))
       (cond ,@(nreverse cases)
             (t (error "cl-ecase failed: %s, %s" ,v
                       ',allkeys))))))

;; ---------- seq additions ----------

(defun seq-sort (predicate sequence)
  "Return SEQUENCE sorted by PREDICATE (non-destructive)."
  (let ((l (append sequence nil)))
    (setq l (sort l predicate))
    (if (vectorp sequence) (vconcat l) l)))

(defun seq-partition (seq n)
  "Return a list of N-element subsequences of SEQ."
  (let ((l (append seq nil)) (res nil) part)
    (while l
      (setq part nil)
      (dotimes (_ n) (when l (push (pop l) part)))
      (push (nreverse part) res))
    (nreverse res)))

(defun seq-group-by (function sequence)
  "Alist grouping SEQUENCE elements by (FUNCTION elt), in first-seen order."
  (let ((res nil))
    (dolist (x (append sequence nil) (nreverse res))
      (let* ((k (funcall function x))
             (cell (cl-assoc k res)))
        (if cell
            (setcdr cell (append (cdr cell) (list x)))
          (push (list k x) res))))))

(defun seq-mapn (function seq &rest seqs)
  "Map FUNCTION over SEQ and SEQS in parallel."
  (let ((lists (cons (append seq nil)
                     (mapcar (lambda (s) (append s nil)) seqs)))
        (res nil))
    (while (cl-every #'consp lists)
      (push (apply function (mapcar #'car lists)) res)
      (setq lists (mapcar #'cdr lists)))
    (nreverse res)))

;; GNU seq.el (bodies verbatim; declared cl-defgeneric there but a
;; single default implementation covers all sequence types).
(defun seq-do-indexed (function sequence)
  "Apply FUNCTION to each element of SEQUENCE and return nil.
Unlike `seq-map', FUNCTION takes two arguments: the element of
the sequence, and its index within the sequence."
  (let ((index 0))
    (seq-do (lambda (elt)
              (funcall function elt index)
              (setq index (1+ index)))
            sequence))
  nil)

(defun seq-map-indexed (function sequence)
  "Return the result of applying FUNCTION to each element of SEQUENCE.
Unlike `seq-map', FUNCTION takes two arguments: the element of
the sequence, and its index within the sequence."
  (let ((index 0))
    (seq-map (lambda (elt)
               (prog1
                   (funcall function elt index)
                 (setq index (1+ index))))
             sequence)))

(defun seq-sort-by (function pred sequence)
  "Sort SEQUENCE transformed by FUNCTION using PRED as the comparison function.
Elements of SEQUENCE are transformed by FUNCTION before being
sorted.  FUNCTION must be a function of one argument.  The sort
operates on a copy of SEQUENCE and does not modify SEQUENCE."
  (seq-sort (lambda (a b)
              (funcall pred
                       (funcall function a)
                       (funcall function b)))
            sequence))

(defun seq-positions (sequence elt &optional testfn)
  "Return list of indices of SEQUENCE elements for which TESTFN returns non-nil.

TESTFN is a two-argument function which is called with each element of
SEQUENCE as the first argument and ELT as the second.
TESTFN defaults to `equal'.

The result is a list of (zero-based) indices."
  (let ((result '()))
    (seq-do-indexed
     (lambda (e index)
       (when (funcall (or testfn #'equal) e elt)
         (push index result)))
     sequence)
    (nreverse result)))

(defun seq-union (sequence1 sequence2 &optional testfn)
  "Return a list of all the elements that appear in either SEQUENCE1 or SEQUENCE2.
\"Equality\" of elements is defined by the function TESTFN, which
defaults to `equal'.
This does not modify SEQUENCE1 or SEQUENCE2."
  (let* ((accum (lambda (acc elt)
                  (if (seq-contains-p acc elt testfn)
                      acc
                    (cons elt acc))))
         (result (seq-reduce accum sequence2
                          (seq-reduce accum sequence1 '()))))
    (nreverse result)))

(defun seq-intersection (sequence1 sequence2 &optional testfn)
  "Return copy of SEQUENCE1 with elements that do not appear in SEQUENCE2 removed.
\"Equality\" of elements is defined by the function TESTFN, which
defaults to `equal'.
This does not modify SEQUENCE1 or SEQUENCE2."
  (seq-reduce (lambda (acc elt)
                (if (seq-contains-p sequence2 elt testfn)
                    (cons elt acc)
                  acc))
              (seq-reverse sequence1)
              '()))

(defun seq-random-elt (sequence)
  "Return a randomly chosen element from SEQUENCE.
Signal an error if SEQUENCE is empty."
  (if (seq-empty-p sequence)
      (error "Sequence cannot be empty")
    (seq-elt sequence (random (seq-length sequence)))))

(defun seq-split (sequence length)
  "Split SEQUENCE into a list of sub-sequences of at most LENGTH elements.
All the sub-sequences will be LENGTH long, except the last one,
which may be shorter.  This does not modify SEQUENCE."
  (when (< length 1)
    (error "Sub-sequence length must be larger than zero"))
  (let ((result nil)
        (seq-length (length sequence))
        (start 0))
    (while (< start seq-length)
      (push (seq-subseq sequence start
                        (setq start (min seq-length (+ start length))))
            result))
    (nreverse result)))

(defun seq-keep (function sequence)
  "Apply FUNCTION to SEQUENCE and return the list of all the non-nil results.
This does not modify SEQUENCE."
  (delq nil (seq-map function sequence)))

(defun seq-mapcat (function sequence &optional type)
  "Concatenate the results of applying FUNCTION to each element of SEQUENCE.
The result is a sequence of type TYPE; TYPE defaults to `list'.
This does not modify SEQUENCE."
  (apply #'seq-concatenate (or type 'list)
         (seq-map function sequence)))

(defalias 'cl-dolist 'dolist)
(defalias 'cl-dotimes 'dotimes)

(defmacro cl-typecase (expr &rest clauses)
  "Evaluate CLAUSES matching the type of EXPR (each (TYPE . BODY))."
  (let ((v (gensym)) (default nil) (cases nil))
    (dolist (cl clauses)
      (if (memq (car cl) '(t otherwise))
          (setq default `(progn ,@(cdr cl)))
        (push (list `(cl-typep ,v ',(car cl))
                    `(progn ,@(cdr cl)))
              cases)))
    `(let ((,v ,expr))
       (cond ,@(nreverse cases) (t ,default)))))

(defmacro cl-etypecase (expr &rest clauses)
  "Like `cl-typecase' but signals an error when no clause matches."
  (let ((v (gensym)) (cases nil) (types nil))
    (dolist (cl clauses)
      (push (car cl) types)
      (push (list `(cl-typep ,v ',(car cl)) `(progn ,@(cdr cl))) cases))
    `(let ((,v ,expr))
       (cond ,@(nreverse cases)
             (t (error "cl-etypecase failed: %s, %s" ,v
                       ',(nreverse types)))))))

(defmacro cl-destructuring-bind (args expr &rest body)
  "Bind ARGS (a list pattern) to elements of EXPR.
Subset: flat patterns with &optional/&rest support."
  (let ((vals (gensym)) (binds nil) (rest-sym nil)
        (i 0) (state 'req))
    (dolist (a args)
      (cond
       ((eq a '&optional) (setq state 'opt))
       ((eq a '&rest) (setq state 'rest))
       ((memq a '(&key &aux &allow-other-keys)) (setq state 'skip))
       ((eq state 'rest)
        (setq rest-sym a state 'done))
       ((eq state 'skip) nil)
       (t
        (let ((v (if (consp a) (car a) a))
              (def (and (consp a) (cadr a))))
          (push (list v `(or (nth ,i ,vals) ,def)) binds)
          (setq i (1+ i))))))
    (when rest-sym
      (push (list rest-sym `(nthcdr ,i ,vals)) binds))
    `(let ((,vals ,expr))
       (let ,(nreverse binds) ,@body))))

(defmacro cl-letf (bindings &rest body)
  "Bind generalized PLACEs temporarily (subset: symbols and
\=(symbol-function SYM) places)."
  (let ((saves nil) (sets nil) (rests nil))
    (dolist (b bindings)
      (let ((place (car b)) (val (cadr b)) (tmp (gensym)))
        (cond
         ((and (consp place) (eq (car place) 'symbol-function))
          (push (list tmp `(symbol-function ,(cadr place))) saves)
          (push `(fset ,(cadr place) ,tmp) rests)
          (push `(fset ,(cadr place) ,val) sets))
         ((symbolp place)
          (push (list tmp place) saves)
          (push `(setq ,place ,tmp) rests)
          (push `(setq ,place ,val) sets))
         (t (error "cl-letf: unsupported place %s" place)))))
    `(let ,(nreverse saves)
       (unwind-protect
           (progn ,@(nreverse sets) ,@body)
         ,@(nreverse rests)))))

(defmacro cl-letf* (bindings &rest body)
  "Like `cl-letf' but bindings are made sequentially."
  (if (null bindings)
      `(progn ,@body)
    `(cl-letf (,(car bindings))
       (cl-letf* ,(cdr bindings) ,@body))))

(defmacro cl-flet (bindings &rest body)
  "Bind function names locally (dynamic extent)."
  (let ((lets nil))
    (dolist (b bindings)
      (push (list `(symbol-function ',(car b))
                  `(lambda ,@(cdr b)))
            lets))
    `(cl-letf ,(nreverse lets) ,@body)))

(defmacro cl-labels (bindings &rest body)
  "Like `cl-flet' (labels are dynamically scoped in this dialect)."
  `(cl-flet ,bindings ,@body))

(defmacro cl-macrolet (bindings &rest body)
  "Bind macro names locally."
  (let ((lets nil))
    (dolist (b bindings)
      (push (list `(symbol-function ',(car b))
                  `(cons 'macro (lambda ,@(cdr b))))
            lets))
    `(cl-letf ,(nreverse lets) ,@body)))

(defmacro cl-defstruct (name &rest slots)
  "Define a structure type NAME with SLOTS (subset: no options).
Creates make-NAME, NAME-p, and NAME-SLOT accessors; objects are
records whose first element is NAME."
  (let* ((n (if (consp name) (car name) name))
         (ctor (intern (concat "make-" (symbol-name n))))
         (pred (intern (concat (symbol-name n) "-p")))
         (slots (mapcar (lambda (x) (if (consp x) (car x) x)) slots))
         (defs nil) (i 1))
    (push `(defun ,ctor (&rest cl--keys)
             (apply #'record ',n
                    (mapcar (lambda (s)
                              (plist-get cl--keys
                                         (intern (concat ":"
                                                         (symbol-name s)))))
                            ',slots)))
          defs)
    (push `(defun ,pred (ob)
             (and (recordp ob) (eq (aref ob 0) ',n)))
          defs)
    (dolist (slot slots)
      (let* ((sn (if (consp slot) (car slot) slot))
             (acc (intern (concat (symbol-name n) "-"
                                  (symbol-name sn)))))
        (push `(defun ,acc (ob) (aref ob ,i)) defs))
      (setq i (1+ i)))
    `(progn ,@(nreverse defs) ',n)))

;; ---------- registers / misc ----------

(defun register-read-with-preview (prompt)
  "Read a register name, showing PROMPT."
  (read-char prompt))

(defun append-to-register (register start end &optional delete-flag)
  "Append region text to REGISTER."
  (let ((text (buffer-substring start end)))
    (set-register register
                  (concat (or (get-register register) "") text))
    (when delete-flag (delete-region start end))))

(defun prepend-to-register (register start end &optional delete-flag)
  "Prepend region text to REGISTER."
  (let ((text (buffer-substring start end)))
    (set-register register
                  (concat text (or (get-register register) "")))
    (when delete-flag (delete-region start end))))

(defun set-file-extended-attributes (filename attributes)
  "Set extended ATTRIBUTES on FILENAME (best effort; returns nil)."
  nil)

(defmacro with-suppressed-warnings (_warnings &rest body)
  "Eval BODY with byte-compile WARNINGS suppressed."
  `(progn ,@body))

(defun exec-path ()
  "Return `exec-path'."
  exec-path)

(defun path-separator ()
  "Return `path-separator'."
  path-separator)

(defun cl-float-limits ()
  "Initialize the cl-float-* variables (already set)."
  nil)

;; ---------- cl-loop (subset) ----------

(defmacro cl-loop (&rest clauses)
  "Common Lisp `loop' macro subset: for/in/on/across/=,/from..to,
with, while, until, repeat, if/when/unless, do, collect, append,
nconc, sum, count, maximize, minimize, return, initially, finally."
  (cl--loop-expand clauses))

(defconst cl--loop-keywords
  '(for as with if when unless else end do doing collect collecting
    append appending nconc nconcing sum counting count maximize
    maximizing minimize minimizing return while until repeat
    initially finally from to upto below downto above upfrom
    downfrom in on across by = then and it being the elements
    hash-key hash-keys hash-value hash-values of each))

(defun cl--loop-action (clauses i)
  "Parse one action clause at index I; return (FORMS KINDS NEW-I).
Accumulation refers to the `cl--loop-list-acc' and
`cl--loop-num-acc' variables bound by the generated code."
  (let ((kw (nth i clauses)) (forms nil) (kinds nil))
    (cond
     ((memq kw '(do doing))
      (setq i (1+ i))
      (while (and (< i (length clauses))
                  (not (memq (nth i clauses) cl--loop-keywords)))
        (push (nth i clauses) forms)
        (setq i (1+ i)))
      (setq forms (list (cons 'progn (nreverse forms)))))
     ((memq kw '(collect collecting append appending nconc nconcing
                 sum counting count maximize maximizing minimize
                 minimizing))
      (let* ((e (nth (1+ i) clauses))
             (kind (cond ((memq kw '(collect collecting)) 'collect)
                         ((memq kw '(append appending)) 'append)
                         ((memq kw '(nconc nconcing)) 'nconc)
                         ((memq kw '(sum counting)) 'sum)
                         ((eq kw 'count) 'count)
                         ((memq kw '(maximize maximizing)) 'max)
                         (t 'min))))
        (setq i (+ i 2))
        (when (eq (nth i clauses) 'into) (setq i (+ i 2)))
        (push kind kinds)
        (push
         (cond
          ((eq kind 'collect) `(push ,e cl--loop-list-acc))
          ((eq kind 'append)
           `(setq cl--loop-list-acc
                  (nconc cl--loop-list-acc (append ,e nil))))
          ((eq kind 'nconc)
           `(setq cl--loop-list-acc (nconc cl--loop-list-acc ,e)))
          ((eq kind 'sum)
           `(setq cl--loop-num-acc (+ cl--loop-num-acc ,e)))
          ((eq kind 'count)
           `(when ,e (setq cl--loop-num-acc (1+ cl--loop-num-acc))))
          (t `(setq cl--loop-ext-acc
                    (if cl--loop-ext-acc
                        (,(if (eq kind 'max) 'max 'min)
                         cl--loop-ext-acc ,e)
                      ,e))))
         forms)))
     ((eq kw 'return)
      (push `(throw 'cl--loop ,(nth (1+ i) clauses)) forms)
      (setq i (+ i 2)))
     (t (error "cl-loop: bad action clause %s" kw)))
    (list forms kinds i)))

(defun cl--loop-expand (clauses)
  (let ((inits nil) (initially nil) (pretests nil) (pre nil)
        (steps nil) (body nil) (finally nil) (finret nil)
        (kinds nil) (i 0) (n (length clauses)))
    (while (< i n)
      (let ((kw (nth i clauses)))
        (cond
         ((memq kw '(for as))
          ;; for VAR <iter> [and VAR <iter>]*
          (let ((var nil))
            (setq i (1+ i))
            (let ((more t))
              (while more
                (setq var (nth i clauses) i (1+ i))
                (let ((op (nth i clauses)))
                  (setq i (1+ i))
                  (cond
                   ((memq op '(in on))
                    (let ((tl (gensym)) (src (nth i clauses)) (by nil))
                      (setq i (1+ i))
                      (when (eq (nth i clauses) 'by)
                        (setq by (nth (1+ i) clauses) i (+ i 2)))
                      (setq inits (append inits
                                          (list (list tl src)
                                                (list var nil))))
                      (push `(consp ,tl) pretests)
                      (push (if (eq op 'on) `(setq ,var ,tl)
                              `(setq ,var (car ,tl)))
                            pre)
                      (push (if by `(setq ,tl (funcall ,by ,tl))
                              `(setq ,tl (cdr ,tl)))
                            steps)))
                   ((eq op 'across)
                    (let ((v (gensym)) (ix (gensym)))
                      (setq inits (append inits
                                          (list (list v (nth i clauses))
                                                (list ix 0)
                                                (list var nil))))
                      (push `(< ,ix (length ,v)) pretests)
                      (push `(setq ,var (aref ,v ,ix)) pre)
                      (push `(setq ,ix (1+ ,ix)) steps)
                      (setq i (1+ i))))
                   ((eq op '=)
                    (let ((e (nth i clauses)) (then nil))
                      (setq i (1+ i))
                      (when (eq (nth i clauses) 'then)
                        (setq then (nth (1+ i) clauses) i (+ i 2)))
                      (setq inits (append inits (list (list var e))))
                      (push `(setq ,var ,(or then e)) steps)))
                   ((eq op 'being)
                    (when (eq (nth i clauses) 'the) (setq i (1+ i)))
                    (when (memq (nth i clauses) '(elements element))
                      (setq i (1+ i))
                      (when (eq (nth i clauses) 'of) (setq i (1+ i)))
                      (let ((v (gensym)) (ix (gensym)))
                        (setq inits
                              (append inits (list (list v (nth i clauses))
                                                  (list ix 0)
                                                  (list var nil))))
                        (push `(< ,ix (length ,v)) pretests)
                        (push `(setq ,var (aref ,v ,ix)) pre)
                        (push `(setq ,ix (1+ ,ix)) steps)
                        (setq i (1+ i)))))
                   ((memq op '(from upfrom downfrom below above
                                  to upto downto))
                    (let ((down (memq op '(downfrom downto)))
                          (init-e (if (memq op '(from upfrom downfrom))
                                      (prog1 (nth i clauses)
                                        (setq i (1+ i)))
                                    (if (memq op '(below to upto)) 0
                                      0)))
                          (bound nil) (cmp nil) (step 1))
                      (when (memq op '(below above to upto downto))
                        ;; Bound was consumed as the "op" itself.
                        (let ((e2 (nth i clauses)))
                          (setq i (1+ i))
                          (cond
                           ((eq op 'below) (setq bound e2 cmp '<))
                           ((eq op 'above) (setq bound e2 cmp '> down t))
                           ((memq op '(to upto)) (setq bound e2 cmp '<=))
                           ((eq op 'downto)
                            (setq bound e2 cmp '>= down t init-e
                                  e2)))))
                      (while (memq (nth i clauses)
                                   '(to upto below downto above by))
                        (let ((k2 (nth i clauses))
                              (e2 (nth (1+ i) clauses)))
                          (cond
                           ((memq k2 '(to upto))
                            (setq bound e2 cmp '<=))
                           ((eq k2 'below)
                            (setq bound e2 cmp '<))
                           ((eq k2 'downto)
                            (setq bound e2 cmp '>= down t))
                           ((eq k2 'above)
                            (setq bound e2 cmp '> down t))
                           ((eq k2 'by) (setq step e2)))
                          (setq i (+ i 2))))
                      (setq inits
                            (append inits (list (list var init-e))))
                      (when bound
                        (push (list cmp var bound) pretests))
                      (push `(setq ,var (,(if down '- '+) ,var ,step))
                            steps)))
                   (t (error "cl-loop: bad for clause %s" op))))
                (setq more (eq (nth i clauses) 'and))
                (when more (setq i (1+ i)))))))
         ((eq kw 'with)
          (let ((more t))
            (setq i (1+ i))
            (while more
              (let ((v (nth i clauses)) (e nil))
                (setq i (1+ i))
                (when (eq (nth i clauses) '=)
                  (setq e (nth (1+ i) clauses) i (+ i 2)))
                (setq inits (append inits (list (list v e))))
                (setq more (eq (nth i clauses) 'and))
                (when more (setq i (1+ i)))))))
         ((eq kw 'repeat)
          (let ((c (gensym)))
            (setq inits
                  (append inits (list (list c (nth (1+ i) clauses)))))
            (push `(> ,c 0) pretests)
            (push `(setq ,c (1- ,c)) steps)
            (setq i (+ i 2))))
         ((eq kw 'while)
          (push (nth (1+ i) clauses) pretests) (setq i (+ i 2)))
         ((eq kw 'until)
          (push `(not ,(nth (1+ i) clauses)) pretests) (setq i (+ i 2)))
         ((memq kw '(if when unless))
          (let* ((raw (nth (1+ i) clauses))
                 (cnd (if (eq kw 'unless) `(not ,raw) raw))
                 (a (cl--loop-action clauses (+ i 2)))
                 (then-forms (car a)) (j (nth 2 a)) (else-forms nil))
            (setq kinds (append kinds (cadr a)))
            (when (eq (nth j clauses) 'else)
              (let ((b (cl--loop-action clauses (1+ j))))
                (setq else-forms (car b) j (nth 2 b)
                      kinds (append kinds (cadr b)))))
            (when (eq (nth j clauses) 'end) (setq j (1+ j)))
            (push `(if ,cnd (progn ,@then-forms)
                     ,@(when else-forms `((progn ,@else-forms))))
                  body)
            (setq i j)))
         ((eq kw 'initially)
          (setq i (1+ i))
          (while (and (< i n)
                      (not (memq (nth i clauses) cl--loop-keywords)))
            (push (nth i clauses) initially)
            (setq i (1+ i))))
         ((eq kw 'finally)
          (setq i (1+ i))
          (when (memq (nth i clauses) '(do doing)) (setq i (1+ i)))
          (if (eq (nth i clauses) 'return)
              (setq finret (nth (1+ i) clauses) i (+ i 2))
            (while (and (< i n)
                        (not (memq (nth i clauses) cl--loop-keywords)))
              (push (nth i clauses) finally)
              (setq i (1+ i)))))
         ((memq kw '(do doing collect collecting append appending
                    nconc nconcing sum counting count maximize
                    maximizing minimize minimizing return))
          (let ((a (cl--loop-action clauses i)))
            (setq body (append body (car a))
                  kinds (append kinds (cadr a))
                  i (nth 2 a))))
         (t (error "cl-loop: unknown clause %s" kw)))))
    `(let ,(append (nreverse inits)
                   '((cl--loop-list-acc nil) (cl--loop-num-acc 0)
                     (cl--loop-ext-acc nil)))
       (catch 'cl--loop
         ,@(nreverse initially)
         (while (and ,@(nreverse pretests))
           ,@(nreverse pre)
           ,@body
           ,@(nreverse steps))
         ,@(nreverse finally)
         ,(or finret
              (cond
               ((memq 'collect kinds) '(nreverse cl--loop-list-acc))
               ((or (memq 'append kinds) (memq 'nconc kinds))
                'cl--loop-list-acc)
               ((or (memq 'sum kinds) (memq 'count kinds))
                'cl--loop-num-acc)
               ((or (memq 'max kinds) (memq 'min kinds))
                'cl--loop-ext-acc)
               (t nil)))))))

(defun cl--sm-subst (form bindings)
  "Substitute symbol-macrolet BINDINGS ((SYM FORM)...) in FORM tree.
`setq' on a bound symbol becomes `setf' on its expansion, like GNU's
`cl--sm-macroexpand'."
  (cond
   ((symbolp form)
    (let ((b (assq form bindings)))
      (if b (cadr b) form)))
   ((consp form)
    (cond
     ((memq (car form) '(quote function))
      form)
     ((memq (car form) '(setq setq-default))
      ;; (setq SYM VAL ...) → (setf EXPANSION VAL ...) for bound syms.
      (let ((args (cdr form)) (out nil))
        (while args
          (let* ((s (car args)) (v (cadr args))
                 (b (assq s bindings)))
            (if b
                (push (list 'setf (cl--sm-subst (cadr b) bindings)
                            (cl--sm-subst v bindings))
                      out)
              (push (list (car form) s
                          (cl--sm-subst v bindings))
                    out)))
          (setq args (cddr args)))
        (if (cdr out) (cons 'progn (nreverse out)) (car out))))
     (t
      (cons (cl--sm-subst (car form) bindings)
            (cl--sm-subst (cdr form) bindings)))))
   (t form)))

(defmacro cl-symbol-macrolet (bindings &rest body)
  "Bind symbols as macros expanding to their forms (subset)."
  `(progn ,@(cl--sm-subst body bindings)))

;; ---------- occur / replace subset ----------

(defun list-matching-lines (regexp &optional nlines buffer)
  "Show lines matching REGEXP in BUFFER (subset: *Occur* buffer)."
  (interactive "sList lines matching: ")
  (let ((buf (or buffer (current-buffer))) (matches nil))
    (with-current-buffer buf
      (save-excursion
        (goto-char (point-min))
        (while (re-search-forward regexp nil t)
          (push (buffer-substring (line-beginning-position)
                                  (line-end-position))
                matches)
          (forward-line 1))))
    (setq matches (nreverse matches))
    (if (null matches)
        (message "Searched %d buffer%s; no matches for \"%s\""
                 1 "" regexp)
      (let ((obuf (get-buffer-create "*Occur*")))
        (with-current-buffer obuf
          (erase-buffer)
          (insert (format "%d match%s for \"%s\" in buffer: %s\n"
                          (length matches)
                          (if (= (length matches) 1) "" "es")
                          regexp (buffer-name buf)))
          (dolist (m matches) (insert m "\n")))
        (display-buffer obuf)))
    t))

(defun perform-replace (from-string replacements query-flag
                        regexp-flag delimited-flag
                        &optional repeat-count map start end
                        backward region-noncontiguous-p)
  "Replace FROM-STRING with REPLACEMENTS (subset: non-query only)."
  (save-excursion
    (goto-char (or start (point-min)))
    (let ((count 0) (limit (or repeat-count most-positive-fixnum)))
      (while (and (< count limit)
                  (if regexp-flag
                      (re-search-forward from-string end t)
                    (search-forward from-string end t)))
        (replace-match replacements (not regexp-flag)
                       (not regexp-flag))
        (setq count (1+ count)))
      count)))

;; ---------- more subr.el / simple.el helpers ----------

(defun alist-get (key alist &optional default remove testfn)
  "Return the value associated with KEY in ALIST."
  (let ((x (if testfn
               (cl-assoc key alist :test testfn)
             (assoc key alist))))
    (if x (cdr x) default)))

(defun flatten-list (list)
  "Flatten LIST: return a list of all non-nil atoms."
  (let ((res nil) (stack (list list)))
    (while stack
      (let ((x (pop stack)))
        (cond
         ((null x) nil)
         ((consp x)
          (push (cdr x) stack)
          (push (car x) stack))
         (t (push x res)))))
    (nreverse res)))

(defun buffer-narrowed-p (&optional buffer)
  "Return t if BUFFER is narrowed."
  (with-current-buffer (or buffer (current-buffer))
    (or (/= (point-min) 1)
        (/= (point-max) (1+ (buffer-size))))))

(defun shell-quote-argument (arg)
  "Quote ARG for use as a shell argument (POSIX style)."
  (if (equal arg "")
      "''"
    (let ((res nil))
      (dolist (c (append arg nil))
        (push (if (string-match-p "[-a-zA-Z0-9_@%+=:,./~]"
                                  (char-to-string c))
                  (char-to-string c)
                (concat "\\" (char-to-string c)))
              res))
      (apply #'concat (nreverse res)))))

(defun combine-and-quote-strings (strings &optional separator)
  "Join STRINGS with SEPARATOR (default space), quoting strings
containing whitespace or quotes with double quotes."
  (mapconcat
   (lambda (st)
     (if (string-match "[\"\\ 	
]" st)
         (concat "\"" (string-replace "\"" "\\\"" st) "\"")
       st))
   strings
   (or separator " ")))

(defun split-string-and-unquote (string &optional separator)
  "Split STRING at SEPARATOR (default whitespace) respecting
double-quoted spans; the quotes are removed."
  (let ((seps (if separator (append separator nil)
                '(?\s ?\t ?\n ?\r ?\f ?\v)))
        (res nil) (cur nil) (inq nil) (i 0) (n (length string)))
    (while (< i n)
      (let ((c (aref string i)))
        (cond
         ((and (not inq) (memq c seps))
          (when cur (push (apply #'concat (nreverse cur)) res))
          (setq cur nil i (1+ i)))
         ((eq c ?\") (setq inq (not inq) i (1+ i)))
         (t (push (char-to-string c) cur) (setq i (1+ i))))))
    (when cur (push (apply #'concat (nreverse cur)) res))
    (nreverse res)))

(defun time-to-seconds (&optional time)
  "Convert TIME (a Lisp time value) to seconds."
  (float-time time))

(defvar emacs--start-time (float-time))

(defun emacs-init-time ()
  "Return a string describing the Emacs startup time."
  (format "%.1f seconds" (- (float-time) emacs--start-time)))

(defun function-alias-p (func &optional _noerror)
  "Return non-nil if FUNC's function cell is another symbol."
  (let ((d (and (symbolp func) (symbol-function func))))
    (and (symbolp d) (list d))))

(defun symbol-file (symbol &optional type)
  "Return the file where SYMBOL was defined (nil if unknown)."
  nil)

(defun find-lisp-object-file-name (object &optional type)
  "Return the file where OBJECT was defined (nil if unknown)."
  nil)

(defun pop-to-buffer-same-window (buffer &optional norecord)
  "Display BUFFER in the selected window."
  (pop-to-buffer buffer nil norecord))

(defun field-at-pos (pos)
  "Return the `field' property at POS."
  (get-char-property pos 'field))

(defun completion-try-completion (string table pred point
                                         &optional metadata)
  "Try to complete STRING using TABLE."
  (let ((r (try-completion string table pred)))
    (if (stringp r) (cons r (or point (length r))) r)))

(defun completion-all-completions (string table pred point
                                          &optional metadata)
  "All completions of STRING in TABLE."
  (all-completions string table pred))

(defun completion--action (action table string pred)
  "Perform completion ACTION on TABLE for STRING and PRED."
  (cond
   ((functionp table) (funcall table string pred action))
   ((or (eq action 'metadata) (eq action 'boundaries)
        (eq (car-safe action) 'boundaries)) nil)
   ((eq action nil) (try-completion string table pred))
   ((eq action 'lambda) (test-completion string table pred))
   (t (all-completions string table pred))))

(defun completion-table-dynamic (fun &optional switch-buffer)
  "Use FUN as a dynamic completion table: FUN is called with the
string to complete and returns the list of completions."
  ;; `',fun': dynamic scope means no closures — embed the value.
  (list 'lambda '(string pred action)
        (list 'completion--action 'action
              (list 'funcall (list 'quote fun) 'string)
              'string 'pred)))

(defun completion-table-merge (&rest tables)
  "Return a completion table merging the candidates of all TABLES."
  (list 'lambda '(string pred action)
        `(if (eq action 'lambda)
             (cl-some
              (lambda (tab) (completion--action action tab string pred))
              ',tables)
           (completion--action
            action
            (apply #'append
                   (mapcar (lambda (tab) (all-completions string tab pred))
                           ',tables))
            string pred))))

(defun completion-table-in-turn (&rest tables)
  "Return a completion table trying each of TABLES in turn."
  (list 'lambda '(string pred action)
        `(if (eq (car-safe action) 'boundaries)
             (completion--action action (car ',tables) string pred)
           (cl-some
            (lambda (tab) (completion--action action tab string pred))
            ',tables))))

(defun completion-table-with-cache (fun &optional bound)
  "Return a dynamic completion table caching FUN's last result.
The cache is reused while STRING still starts with the string the
cache was computed for."
  (let ((cell (cons nil nil)))
    (list 'lambda '(string pred action)
          `(let ((cache ',cell))
             (unless (and (car cache)
                          (string-prefix-p (car cache) string)
                          (or (null ,bound)
                              (<= (length string)
                                  (+ ,bound (length (car cache))))))
               (setcar cache string)
               (setcdr cache (funcall ',fun string)))
             (completion--action action (cdr cache) string pred)))))

(defun completion-table-with-context (prefix table string pred action)
  "Complete STRING in TABLE as if PREFIX preceded it.
For action nil the returned completion includes PREFIX; for t the
candidates are returned without it."
  (cond
   ((or (eq action 'metadata) (eq action 'boundaries)
        (eq (car-safe action) 'boundaries)) nil)
   (t
    (let ((comp (completion--action action table string pred)))
      (if (and (eq action nil) (stringp comp))
          (concat prefix comp)
        comp)))))

(defmacro macroexp-quote (v)
  "Return the argument V converted to a form that will \"quote\" it."
  (if (or (consp v)
          (and (symbolp v) (not (keywordp v))))
      (list 'quote v)
    v))

(defun substitute-key-definition (olddef newdef keymap
                                  &optional oldmap prefix)
  "In KEYMAP, rebind every key bound to OLDDEF in OLDMAP to NEWDEF."
  (map-keymap
   (lambda (key def)
     (when (eq def olddef)
       (define-key keymap (vector key) newdef)))
   (or oldmap (current-global-map))))

(defun add-to-ordered-list (list-var element &optional order)
  "Add ELEMENT to the value of LIST-VAR if it isn't there yet.
The test for presence of ELEMENT is done with `eq'.  Numeric ORDER
is recorded as the element's rank; elements with ranks sort before
those without.  LIST-VAR cannot refer to a lexical variable."
  (let ((ordering (get list-var 'list-order)))
    (unless ordering
      (put list-var 'list-order
           (setq ordering (make-hash-table :weakness 'key :test 'eq))))
    (when order
      (puthash element (and (numberp order) order) ordering))
    (unless (memq element (symbol-value list-var))
      (set list-var (cons element (symbol-value list-var))))
    (set list-var
         (sort (symbol-value list-var)
               (lambda (a b)
                 (let ((oa (gethash a ordering))
                       (ob (gethash b ordering)))
                   (if (and oa ob) (< oa ob) oa)))))
    (symbol-value list-var)))

(defvar history-length 60)
(defvar history-delete-duplicates nil
  "Non-nil means `add-to-history' removes duplicate entries.")

(defun add-to-history (history-var newelt &optional maxelt keep-all)
  "Add NEWELT to the history list stored in the variable HISTORY-VAR.
MAXELT bounds the length (default: the `history-length' property of
HISTORY-VAR, else the `history-length' variable).  Empty strings and
entries equal to the most recent element are skipped unless KEEP-ALL.
HISTORY-VAR cannot refer to a lexical variable."
  (unless maxelt
    (setq maxelt (or (get history-var 'history-length)
                     history-length)))
  (let ((history (symbol-value history-var))
        tail)
    (when (and (listp history)
               (or keep-all (not (stringp newelt))
                   (plusp (length newelt)))
               (or keep-all (not (equal (car history) newelt))))
      (if history-delete-duplicates
          (setq history (delete newelt history)))
      (setq history (cons newelt history))
      (when (integerp maxelt)
        (if (>= 0 maxelt)
            (setq history nil)
          (setq tail (nthcdr (1- maxelt) history))
          (when (consp tail)
            (setcdr tail nil)))))
    (set history-var history)))

(defun cl--take (n list)
  (let ((res nil))
    (while (and (> n 0) list)
      (push (pop list) res) (setq n (1- n)))
    (nreverse res)))

(defvar buffer-invisibility-spec t
  "If t, all invisible text is invisible; if a list, only listed
symbols (and (sym . t) entries) make text invisible.")

(defun add-to-invisibility-spec (element)
  "Add ELEMENT to `buffer-invisibility-spec'.
See documentation for `buffer-invisibility-spec' for the kind of
elements that can be added."
  (if (eq buffer-invisibility-spec t)
      (setq buffer-invisibility-spec (list t)))
  (add-to-list 'buffer-invisibility-spec element))

(defun remove-from-invisibility-spec (element)
  "Remove ELEMENT from `buffer-invisibility-spec'."
  (if (consp buffer-invisibility-spec)
      (setq buffer-invisibility-spec
            (delete element buffer-invisibility-spec))))

(defun add-minor-mode (toggle name &optional keymap after lighter)
  "Register a minor mode in `minor-mode-alist'."
  (let ((existing (assq toggle minor-mode-alist))
        (entry (list toggle (or lighter name))))
    (if existing
        (setcdr existing (cdr entry))
      (setq minor-mode-alist (cons entry minor-mode-alist)))
    (when keymap
      (setq minor-mode-map-alist
            (cons (cons toggle keymap) minor-mode-map-alist)))))

(defun event--posn-at-point ()
  (if (fboundp 'posn-at-point) (posn-at-point)))

(defun event-start (event)
  "Return the starting position of EVENT, a click or drag event.
If EVENT is nil, the value of `posn-at-point' is used instead."
  (if (and (consp event)
           (memq (car event) '(touchscreen-begin touchscreen-end)))
      (cdr (car-safe (cdr event)))
    (or (and (consp event)
             (not (eq (car event) 'touchscreen-update))
             (nth 1 event))
        (event--posn-at-point))))

(defun event-end (event)
  "Return the ending position of EVENT.
See `event-start' for a description of the value returned."
  (if (and (consp event)
           (memq (car event) '(touchscreen-begin touchscreen-end)))
      (cdr (car-safe (cdr event)))
    (or (and (consp event)
             (not (eq (car event) 'touchscreen-update))
             (nth (if (consp (nth 2 event)) 2 1) event))
        (event--posn-at-point))))

(defsubst event-click-count (event)
  "Return the multi-click count of EVENT, a click or drag event."
  (if (and (consp event) (integerp (nth 2 event))) (nth 2 event) 1))

(defsubst event-line-count (event)
  "Return the line count of EVENT, a mousewheel event."
  (if (and (consp event) (integerp (nth 3 event))) (nth 3 event) 1))

(defun posnp (obj)
  "Return non-nil if OBJ appears to be a valid posn object."
  (and (windowp (car-safe obj))
       (atom (car-safe (car-safe (cdr obj))))
       (integerp (car-safe (car-safe (cdr (cdr obj)))))
       (integerp (car-safe (cdr (cdr (cdr obj)))))))

(defun posn-window (position) "Return the window in POSITION."
  (nth 0 position))

(defun posn-area (position)
  "Return the window area recorded in POSITION, or nil for the text area."
  (let ((area (if (consp (nth 1 position))
                  (car (nth 1 position))
                (nth 1 position))))
    (and (symbolp area) area)))

(defun posn-point (position)
  "Return the buffer location in POSITION."
  (or (nth 5 position)
      (let ((pt (nth 1 position)))
        (or (car-safe pt)
            (if (integerp pt) pt)))))

(defsubst posn-x-y (position)
  "Return the x and y coordinates in POSITION as (X . Y)."
  (nth 2 position))

(defun posn-actual-col-row (position)
  "Return the window row number and character number in POSITION."
  (nth 6 position))

(defun posn-col-row (position &optional use-window)
  "Return the nominal column and row in POSITION, in characters."
  (let* ((pair (posn-x-y position))
         (frame-or-window (posn-window position))
         (window (and (windowp frame-or-window) frame-or-window))
         (area (posn-area position)))
    (cond
     ((null frame-or-window) '(0 . 0))
     ((eq area 'vertical-scroll-bar)
      (cons 0 (scroll-bar-scale pair (1- (window-height window)))))
     ((eq area 'horizontal-scroll-bar)
      (cons (scroll-bar-scale pair (window-width window)) 0))
     (t (if use-window
            (cons (/ (car pair) (window-font-width window))
                  (/ (cdr pair) (window-font-height window)))
          (cons (/ (car pair)
                   (frame-char-width
                    (if (framep frame-or-window) frame-or-window
                      (window-frame frame-or-window))))
                (/ (cdr pair)
                   (frame-char-height
                    (if (framep frame-or-window) frame-or-window
                      (window-frame frame-or-window))))))))))

(defsubst posn-timestamp (position)
  "Return the timestamp of POSITION."
  (nth 3 position))

(defun posn-string (position)
  "Return the string object of POSITION: a cons (STRING . POS) or nil."
  (let ((x (nth 4 position)))
    (when (consp x) x)))

(defsubst posn-image (position)
  "Return the image object of POSITION, or nil if not an image."
  (nth 7 position))

(defsubst posn-object (position)
  "Return the object (image or string) of POSITION."
  (or (posn-image position) (posn-string position)))

(defsubst posn-object-x-y (position)
  "Return the (DX . DY) offset of the object glyph in POSITION."
  (nth 8 position))

(defsubst posn-object-width-height (position)
  "Return the (WIDTH . HEIGHT) of the object glyph in POSITION."
  (nth 9 position))

(defun posn-set-point (position)
  "Move point to POSITION; select the corresponding window."
  (if (framep (posn-window position))
      (progn
        (unless (windowp (frame-selected-window (posn-window position)))
          (error "Position not in text area of window"))
        (select-window (frame-selected-window (posn-window position))))
    (unless (windowp (posn-window position))
      (error "Position not in text area of window"))
    (select-window (posn-window position)))
  (if (numberp (posn-point position))
      (goto-char (posn-point position))))

;; ---------- mode machinery ----------

(defmacro define-derived-mode (variant parent name &optional docstring
                                       &rest body)
  "Define VARIANT as a major mode derived from PARENT (subset).
Mirrors GNU's `define-derived-mode': leading keyword arguments
(`:group', `:abbrev-table', `:syntax-table', `:after-hook',
`:interactive') are consumed before the body; each mode gets a
`<mode>-syntax-table' and (unless `:abbrev-table' is given) a
`<mode>-abbrev-table' obarray var.  At activation the mode table's
syntax parent is redirected to the buffer's previous syntax table
when it still points at `standard-syntax-table', and
`local-abbrev-table' is set to the mode's abbrev table."
  (let* ((map-sym (intern (concat (symbol-name variant) "-map")))
         (hook-sym (intern (concat (symbol-name variant) "-hook")))
         (syntax-sym (intern (concat (symbol-name variant)
                                     "-syntax-table")))
         (abbrev-sym (intern (concat (symbol-name variant)
                                     "-abbrev-table")))
         (abbrev abbrev-sym)
         (declare-abbrev t)
         (interactive t)
         (after-hook nil)
         (rest body))
    ;; Consume GNU's keyword arguments (derived.el).
    (while (keywordp (car rest))
      (let ((kw (pop rest)))
        (cond
         ((eq kw :abbrev-table)
          (setq abbrev (pop rest) declare-abbrev nil))
         ((eq kw :interactive) (setq interactive (pop rest)))
         ((eq kw :after-hook) (setq after-hook (pop rest)))
         ;; :group and unknown keywords consume one argument each.
         (t (pop rest)))))
    (setq body rest)
    `(progn
       (defvar ,map-sym
               ,(if parent
                    `(let ((m (make-sparse-keymap))
                           (pmsym ',(intern
                                     (concat (symbol-name parent)
                                             "-map"))))
                       (when (boundp pmsym)
                         (set-keymap-parent m (symbol-value pmsym)))
                       m)
                  '(make-sparse-keymap))
               ,(concat "Keymap for `" (symbol-name variant) "'."))
       (defvar ,syntax-sym)
       (unless (boundp ',syntax-sym)
         (put ',syntax-sym 'definition-name ',variant)
         (defvar ,syntax-sym (make-syntax-table)
           ,(concat "Syntax table for `" (symbol-name variant) "'.")))
       ,@(when declare-abbrev
           `((defvar ,abbrev)
             (unless (boundp ',abbrev)
               (put ',abbrev 'definition-name ',variant)
               ;; `make-abbrev-table' at obarray level (it is defined
               ;; later in the prelude than most mode declarations).
               (defvar ,abbrev
                 (let ((remacs--ab (obarray-make)))
                   (let ((s (intern "" remacs--ab)))
                     (set s nil)
                     (setplist s (list :abbrev-table-modiff 0)))
                   remacs--ab)))))
       (defvar ,hook-sym nil)
       (defun ,variant ()
         ,@(when docstring (list docstring))
         ,@(when interactive '((interactive)))
         ;; GNU runs the parent first; the kill-all-local-variables at
         ;; the chain's root (fundamental-mode) resets locals.
         ,@(if parent
               `((when (fboundp ',parent) (,parent)))
               '((kill-all-local-variables)))
        (setq major-mode ',variant
               mode-name ,name)
         ;; GNU derived.el: the table-parent fixes below run only when
         ;; the mode has a parent (prog-mode's parent is nil).
         ,@(when parent
             `((let ((parent (char-table-parent ,syntax-sym)))
                 (unless (and parent
                            (not (eq parent (standard-syntax-table))))
                   (set-char-table-parent ,syntax-sym (syntax-table))))
               ,@(when declare-abbrev
                   `((unless (or (abbrev-table-get ,abbrev :parents)
                                (eq ,abbrev local-abbrev-table))
                       (abbrev-table-put ,abbrev :parents
                                         (list local-abbrev-table)))))))
         (use-local-map ,map-sym)
         (set-syntax-table ,syntax-sym)
         ,@(when abbrev `((setq local-abbrev-table ,abbrev)))
         ,@body
         ,@(when after-hook
             `((push (lambda () ,after-hook)
                     delayed-after-hook-functions)))
         (run-mode-hooks ',hook-sym))
       ,@(when parent
           `((derived-mode-set-parent ',variant ',parent)))
       ',variant)))

(defun merge-ordered-lists (lists &optional error-function)
  "Merge LISTS in a consistent order (C3 linearization).
Equality is tested with `eql'.  On inconsistency, ERROR-FUNCTION is
called with the remaining lists; by default the head of the first
list is used."
  (let ((result nil))
    (setq lists (remq nil lists))
    (while (cdr lists)
      (let* ((find-next
              (lambda (lists)
                (let ((next nil)
                      (tail lists))
                  (while tail
                    (let ((candidate (caar tail))
                          (other-lists lists))
                      (while other-lists
                        (if (not (memql candidate (cdr (car other-lists))))
                            (setq other-lists (cdr other-lists))
                          (setq candidate nil)
                          (setq other-lists nil)))
                      (if (not candidate)
                          (setq tail (cdr tail))
                        (setq next candidate)
                        (setq tail nil))))
                  next)))
             (next (funcall find-next lists)))
        (unless next
          (let ((tail lists))
            (while (and (cdr tail) (null (funcall find-next (cdr tail))))
              (setq tail (cdr tail)))
            (setq next
                  (funcall (or error-function
                               (lambda (remaining-lists)
                                 (message "Inconsistent hierarchy: %S"
                                          remaining-lists)
                                 (caar remaining-lists)))
                           tail))
            (unless (assoc next lists #'eql)
              (error "Invalid candidate returned by error-function: %S"
                     next))
            (dolist (list lists) (setcdr list (remq next (cdr list))))))
        (push next result)
        (setq lists
              (delq nil
                    (mapcar (lambda (l) (if (eql (car l) next) (cdr l) l))
                            lists)))))
    (if (null result) (car lists)
      (append (nreverse result) (car lists)))))

(defun derived-mode-all-parents (mode &optional known-children)
  "Return all the parents of MODE, starting with MODE."
  (let ((ps (get mode 'derived-mode--all-parents)))
    (cond
     (ps ps)
     ((memq mode known-children)
      (memq mode (reverse known-children)))
     (t
      (let* ((new-children (cons mode known-children))
             (parent (or (get mode 'derived-mode-parent)
                         (let ((alias (symbol-function mode)))
                           (and (symbolp alias) alias))))
             (extras (get mode 'derived-mode-extra-parents))
             (all-parents
              (merge-ordered-lists
               (cons (if (and parent (not (memq parent extras)))
                         (derived-mode-all-parents parent new-children))
                     (mapcar (lambda (p)
                               (derived-mode-all-parents p new-children))
                             extras)))))
        (if (and (memq mode all-parents) known-children)
            (cons mode (remq mode all-parents))
          (put mode 'derived-mode--all-parents (cons mode all-parents))))))))

(defun provided-mode-derived-p (mode &optional modes &rest old-modes)
  "Non-nil if MODE is derived from a member of MODES."
  (cond
   (old-modes (setq modes (cons modes old-modes)))
   ((not (listp modes)) (setq modes (list modes))))
  (let ((ps (derived-mode-all-parents mode)))
    (while (and modes (not (memq (car modes) ps)))
      (setq modes (cdr modes)))
    (car modes)))

(defun derived-mode-p (&optional modes &rest old-modes)
  "Return non-nil if the current major mode is derived from one of MODES."
  (provided-mode-derived-p major-mode (if old-modes (cons modes old-modes)
                                        modes)))

(defun derived-mode--flush (mode)
  (put mode 'derived-mode--all-parents nil)
  (let ((followers (get mode 'derived-mode--followers)))
    (when followers
      (put mode 'derived-mode--followers nil)
      (mapc #'derived-mode--flush followers))))

(defun derived-mode-set-parent (mode parent)
  "Declare PARENT to be the parent of MODE."
  (put mode 'derived-mode-parent parent)
  (derived-mode--flush mode))

(defun derived-mode-add-parents (mode extra-parents)
  "Add EXTRA-PARENTS to the parents of MODE."
  (put mode 'derived-mode-extra-parents extra-parents)
  (derived-mode--flush mode))

(defun modify-face (face &optional foreground background stipple bold-p
                         italic-p underline-p inverse-p frame)
  "Change the display attributes of FACE.
Obsolete: use `set-face-attribute' instead."
  (declare (obsolete set-face-attribute "22.1"))
  (unless (memq face (face-list))
    (signal 'error (list 'Invalid 'face face)))
  (when foreground
    (set-face-attribute face frame :foreground foreground))
  (when background
    (set-face-attribute face frame :background background))
  (when stipple
    (set-face-attribute face frame :stipple stipple))
  (when bold-p
    (set-face-attribute face frame :weight (if bold-p 'bold 'normal)))
  (when italic-p
    (set-face-attribute face frame :slant (if italic-p 'italic 'normal)))
  (when underline-p
    (set-face-attribute face frame :underline underline-p))
  (when inverse-p
    (set-face-attribute face frame :inverse-video inverse-p)))

;; `defface' is defined earlier with the custom.el core (it calls
;; `custom-declare-face').

(defmacro define-generic-mode (&rest args)
  "Define a generic mode (subset: aliases define-derived-mode)."
  (declare (indent 1))
  `(define-derived-mode ,(car args) fundamental-mode ,(cadr args)
                        ,(caddr args)))

;; ---------- eval-after-load plumbing is in load.rs ----------

(defmacro with-buffer-unmodified-if-unchanged (&rest body)
  (let ((doc (if (stringp (car body)) (pop body))))
    `(let ((modp (buffer-modified-p))
           (buffer-undo-list buffer-undo-list))
       (with-silent-modifications
         ,doc
         ,@body
         (restore-buffer-modified-p modp)))))

;; Character categories are implemented as bool-vector sets in
;; category tables (see `make-category-table').

(defvar text-property-default-nonsticky nil)

;;; ---- newcomment.el cluster ----
(defun comment-string-strip (str beforep afterp)
  "Strip STR of any leading (if BEFOREP) and/or trailing (if AFTERP) space."
  (string-match (concat "\\`" (if beforep "\\s-*")
			"\\(.*?\\)" (if afterp "\\s-*\n?")
			"\\'") str)
  (match-string 1 str))

(defun comment-string-reverse (s)
  "Return the mirror image of string S, without any trailing space."
  (comment-string-strip (concat (nreverse (string-to-list s))) nil t))

(defun comment-normalize-vars (&optional noerror)
  "Check and set up variables needed by other commenting functions.
All the `comment-*' commands call this function to set up various
variables, like `comment-start', to ensure that the commenting
functions work correctly.  Lisp callers of any other `comment-*'
function should first call this function explicitly."
  (funcall comment-setup-function)
  (unless (and (not comment-start) noerror)
    (unless comment-start
      (let ((cs (read-string "No comment syntax is defined.  Use: ")))
	(if (zerop (length cs))
	    (error "No comment syntax defined")
          (setq-local comment-start cs)
          (setq-local comment-start-skip cs))))
    ;; comment-use-syntax
    (when (eq comment-use-syntax 'undecided)
      (setq-local comment-use-syntax
                  (let ((st (syntax-table))
                        (cs comment-start)
                        (ce (if (string= "" comment-end) "\n" comment-end)))
                    ;; Try to skip over a comment using forward-comment
                    ;; to see if the syntax tables properly recognize it.
                    (with-temp-buffer
                      (set-syntax-table st)
                      (insert cs " hello " ce)
                      (goto-char (point-min))
                      (and (forward-comment 1) (eobp))))))
    ;; comment-padding
    (unless comment-padding (setq comment-padding 0))
    (when (integerp comment-padding)
      (setq comment-padding (make-string comment-padding ? )))
    ;; comment markers
    ;;(setq comment-start (comment-string-strip comment-start t nil))
    ;;(setq comment-end (comment-string-strip comment-end nil t))
    ;; comment-continue
    (unless (or comment-continue (string= comment-end ""))
      (setq-local comment-continue
                  (concat (if (string-match "\\S-\\S-" comment-start) " " "|")
                          (substring comment-start 1)))
      ;; Hasn't been necessary yet.
      ;; (unless (string-match comment-start-skip comment-continue)
      ;;	(kill-local-variable 'comment-continue))
      )
    ;; comment-skip regexps
    (unless (and comment-start-skip
		 ;; In case comment-start has changed since last time.
		 (string-match comment-start-skip comment-start))
      (setq-local comment-start-skip
                  (concat (unless (eq comment-use-syntax t)
                            ;; `syntax-ppss' will detect escaping.
                            "\\(\\(^\\|[^\\\n]\\)\\(\\\\\\\\\\)*\\)")
                          "\\(?:\\s<+\\|"
                          (regexp-quote (comment-string-strip comment-start t t))
                          ;; Let's not allow any \s- but only [ \t] since \n
                          ;; might be both a comment-end marker and \s-.
                          "+\\)[ \t]*")))
    (unless (and comment-end-skip
		 ;; In case comment-end has changed since last time.
		 (string-match comment-end-skip
                               (if (string= "" comment-end) "\n" comment-end)))
      (let ((ce (if (string= "" comment-end) "\n"
		  (comment-string-strip comment-end t t))))
        (setq-local comment-end-skip
                    ;; We use [ \t] rather than \s- because we don't want to
                    ;; remove ^L in C mode when uncommenting.
                    (concat "[ \t]*\\(\\s>" (if comment-quote-nested "" "+")
                            "\\|" (regexp-quote (substring ce 0 1))
                            (if (and comment-quote-nested (<= (length ce) 1)) "" "+")
                            (regexp-quote (substring ce 1))
                            "\\)"))))))

(defun comment-quote-re (str unp)
  (concat (regexp-quote (substring str 0 1))
	  "\\\\" (if unp "+" "*")
	  (regexp-quote (substring str 1))))

(defvar comment-quote-nested t
  "Non-nil if nested comments should be quoted.
This should be locally set by each major mode if needed.")

(defun comment-quote-nested (cs ce unp)
  "Quote or unquote nested comments.
If UNP is non-nil, unquote nested comment markers."
  (setq cs (comment-string-strip cs t t))
  (setq ce (comment-string-strip ce t t))
  (when (and comment-quote-nested
	     (> (length ce) 0))
    (funcall comment-quote-nested-function cs ce unp)))

(defun comment-quote-nested-default (cs ce unp)
  "Quote comment delimiters in the buffer.
It expects to be called with the buffer narrowed to a single comment.
It is used as a default for `comment-quote-nested-function'.

The arguments CS and CE are strings matching comment starting and
ending delimiters respectively.

If UNP is non-nil, comments are unquoted instead.

To quote the delimiters, a \\ is inserted after the first
character of CS or CE.  If CE is a single character it will
change CE into !CS."
  (let ((re (concat (comment-quote-re ce unp)
		    "\\|" (comment-quote-re cs unp))))
    (goto-char (point-min))
    (while (re-search-forward re nil t)
      (goto-char (match-beginning 0))
      (forward-char 1)
      (if unp (delete-char 1) (insert "\\"))
      (when (= (length ce) 1)
	;; If the comment-end is a single char, adding a \ after that
	;; "first" char won't deactivate it, so we turn such a CE
	;; into !CS.  I.e. for pascal, we turn } into !{
	(if (not unp)
	    (when (string= (match-string 0) ce)
	      (replace-match (concat "!" cs) t t))
	  (when (and (< (point-min) (match-beginning 0))
		     (string= (buffer-substring (1- (match-beginning 0))
						(1- (match-end 0)))
			      (concat "!" cs)))
	    (backward-char 2)
	    (delete-char (- (match-end 0) (match-beginning 0)))
	    (insert ce)))))))

(defun comment-search-forward (limit &optional noerror)
  "Find a comment start between point and LIMIT.
Moves point to inside the comment and returns the position of the
comment-starter.  If no comment is found, moves point to LIMIT
and raises an error or returns nil if NOERROR is non-nil.

Ensure that `comment-normalize-vars' has been called before you use this."
  (if (not comment-use-syntax)
      (if (re-search-forward comment-start-skip limit noerror)
	  (or (match-end 1) (match-beginning 0))
	(goto-char limit)
	(unless noerror (error "No comment")))
    (let* ((pt (point))
	   ;; Assume (at first) that pt is outside of any string.
	   (s (parse-partial-sexp pt (or limit (point-max)) nil nil
				  (if comment-use-global-state (syntax-ppss pt))
				  t)))
      (when (and (nth 8 s) (nth 3 s) (not comment-use-global-state))
	;; The search ended at eol inside a string.  Try to see if it
	;; works better when we assume that pt is inside a string.
	(setq s (parse-partial-sexp
		 pt (or limit (point-max)) nil nil
		 (list nil nil nil (nth 3 s) nil nil nil nil)
		 t)))
      (if (or (not (and (nth 8 s) (not (nth 3 s))))
	      ;; Make sure the comment starts after PT.
	      (< (nth 8 s) pt))
	  (unless noerror (error "No comment"))
	;; We found the comment.
	(let ((pos (point))
	      (start (nth 8 s))
	      (bol (line-beginning-position))
	      (end nil))
	  (while (and (null end) (>= (point) bol))
	    (if (looking-at comment-start-skip)
		(setq end (min (or limit (point-max)) (match-end 0)))
	      (backward-char)))
	  (goto-char (or end pos))
	  start)))))

(defun comment-search-backward (&optional limit noerror)
  "Find a comment start between LIMIT and point.
Moves point to inside the comment and returns the position of the
comment-starter.  If no comment is found, moves point to LIMIT
and raises an error or returns nil if NOERROR is non-nil.

Ensure that `comment-normalize-vars' has been called before you use this."
  ;; FIXME: If a comment-start appears inside a comment, we may erroneously
  ;; stop there.  This can be rather bad in general, but since
  ;; comment-search-backward is only used to find the comment-column (in
  ;; comment-set-column) and to find the comment-start string (via
  ;; comment-beginning) in indent-new-comment-line, it should be harmless.
  (if (not (re-search-backward comment-start-skip limit 'move))
      (unless noerror (error "No comment"))
    (beginning-of-line)
    (let* ((end (match-end 0))
	   (cs (comment-search-forward end t))
	   (pt (point)))
      (if (not cs)
	  (progn (beginning-of-line)
		 (comment-search-backward limit noerror))
	(while (progn (goto-char cs)
		      (comment-forward)
		      (and (< (point) end)
			   (setq cs (comment-search-forward end t))))
	  (setq pt (point)))
	(goto-char pt)
	cs))))

(defun comment-beginning ()
  "Find the beginning of the enclosing comment.
Returns nil if not inside a comment, else moves point and returns
the same as `comment-search-backward'."
  (if (and comment-use-syntax comment-use-global-state)
      (let ((state (syntax-ppss)))
        (when (nth 4 state)
          (goto-char (nth 8 state))
          (prog1 (point)
            (when (save-restriction
                    ;; `comment-start-skip' sometimes checks that the
                    ;; comment char is not escaped.  (Bug#16971)
                    (narrow-to-region (point) (point-max))
                    (looking-at comment-start-skip))
              (goto-char (match-end 0))))))
    ;; Can't rely on the syntax table, let's guess based on font-lock.
    (unless (eq (get-text-property (point) 'face) 'font-lock-string-face)
      (let ((pt (point))
            (cs (comment-search-backward nil t)))
        (when cs
          (if (save-excursion
                (goto-char cs)
                (and
                 ;; For modes where comment-start and comment-end are the same,
                 ;; the search above may have found a `ce' rather than a `cs'.
                 (or (if comment-end-skip (not (looking-at comment-end-skip)))
                     ;; Maybe font-lock knows that it's a `cs'?
                     (eq (get-text-property (match-end 0) 'face)
                         'font-lock-comment-face)
                     (unless (eq (get-text-property (point) 'face)
                                 'font-lock-comment-face)
                       ;; Let's assume it's a `cs' if we're on the same line.
                       (>= (line-end-position) pt)))
                 ;; Make sure that PT is not past the end of the comment.
                 (if (comment-forward 1) (> (point) pt) (eobp))))
              cs
            (goto-char pt)
            nil))))))

(defun comment-forward (&optional n)
  "Skip forward over N comments.
Just like `forward-comment' but only for positive N
and can use regexps instead of syntax."
  (setq n (or n 1))
  (if (< n 0) (error "No comment-backward")
    (if comment-use-syntax (forward-comment n)
      (while (> n 0)
	(setq n
	      (if (or (forward-comment 1)
		      (and (looking-at comment-start-skip)
			   (goto-char (match-end 0))
			   (re-search-forward comment-end-skip nil 'move)))
		  (1- n) -1)))
      (= n 0))))

(defun comment-enter-backward ()
  "Move from the end of a comment to the end of its content.
Point is assumed to be just at the end of a comment."
  (if (bolp)
      ;; comment-end = ""
      (progn (backward-char) (skip-syntax-backward " "))
    (cond
     ((save-excursion
        (save-restriction
          (narrow-to-region (line-beginning-position) (point))
          (goto-char (point-min))
          (re-search-forward (concat comment-end-skip "\\'") nil t)))
      (goto-char (match-beginning 0)))
     ;; comment-end-skip not found probably because it was not set
     ;; right.  Since \\s> should catch the single-char case, let's
     ;; check that we're looking at a two-char comment ender.
     ((not (or (<= (- (point-max) (line-beginning-position)) 1)
               (zerop (logand (car (syntax-after (- (point) 1)))
                              ;; Here we take advantage of the fact that
                              ;; the syntax class " " is encoded to 0,
                              ;; so "  4" gives us just the 4 bit.
                              (car (string-to-syntax "  4"))))
               (zerop (logand (car (syntax-after (- (point) 2)))
                              (car (string-to-syntax "  3"))))))
      (backward-char 2)
      (skip-chars-backward (string (char-after)))
      (skip-syntax-backward " "))
     ;; No clue what's going on: maybe we're really not right after the
     ;; end of a comment.  Maybe we're at the "end" because of EOB rather
     ;; than because of a marker.
     (t (skip-syntax-backward " ")))))

(defun comment-indent-default ()
  "Default for `comment-indent-function'."
  (if (and (looking-at "\\s<\\s<\\(\\s<\\)?")
	   (or (match-end 1) (/= (current-column) (current-indentation))))
      0
    (when (or (/= (current-column) (current-indentation))
	      (and (> comment-add 0) (looking-at "\\s<\\(\\S<\\|\\'\\)")))
      comment-column)))

(defun comment-choose-indent (&optional indent)
  "Choose the indentation to use for a right-hand-side comment.
The criteria are (in this order):
- try to keep the comment's text within `comment-fill-column'.
- try to align with surrounding comments.
- prefer INDENT (or `comment-column' if nil).
Point is expected to be at the start of the comment."
  (unless indent (setq indent comment-column))
  (let ((other nil)
        min max)
    (if (consp indent)
        (progn (setq min (car indent)) (setq max (cdr indent))
               (setq indent comment-column))
      ;; Avoid moving comments past the fill-column.
      (setq max (+ (current-column)
                   (- (or comment-fill-column fill-column)
                      (save-excursion (end-of-line) (current-column)))))
      (setq min (save-excursion
                  (skip-chars-backward " \t")
                  ;; Leave at least `comment-inline-offset' space after
                  ;; other nonwhite text on the line.
                  (if (bolp) 0 (+ comment-inline-offset (current-column))))))
    ;; Fix up the range.
    (if (< max min) (setq max min))
    ;; Don't move past the fill column.
    (if (<= max indent) (setq indent max))
    ;; We can choose anywhere between min..max.
    ;; Let's try to align to a comment on the previous line.
    (save-excursion
      (when (and (zerop (forward-line -1))
                 (setq other (comment-search-forward
                              (line-end-position) t)))
        (goto-char other) (setq other (current-column))))
    (if (and other (<= other max) (>= other min))
        ;; There is a comment and it's in the range: bingo!
        other
      ;; Can't align to a previous comment: let's try to align to comments
      ;; on the following lines, then.  These have not been re-indented yet,
      ;; so we can't directly align ourselves with them.  All we do is to try
      ;; and choose an indentation point with which they will be able to
      ;; align themselves.
      (save-excursion
        (while (and (zerop (forward-line 1))
                    (setq other (comment-search-forward
                                 (line-end-position) t)))
          (goto-char other)
          (let ((omax (+ (current-column)
                         (- (or comment-fill-column fill-column)
                            (save-excursion (end-of-line) (current-column)))))
                (omin (save-excursion (skip-chars-backward " \t")
                                      (1+ (current-column)))))
            (if (and (>= omax min) (<= omin max))
                (progn (setq min (max omin min))
                       (setq max (min omax max)))
              ;; Can't align with this anyway, so exit the loop.
              (goto-char (point-max))))))
      ;; Return the closest point to indent within min..max.
      (max min (min max indent)))))

(defun comment-indent (&optional continue)
  "Indent this line's comment to `comment-column', or insert an empty comment.
If CONTINUE is non-nil, use the `comment-continue' markers if any."
  (interactive "*")
  (comment-normalize-vars)
  (beginning-of-line)
  (let ((starter (or (and continue comment-continue)
                     comment-start
                     (error "No comment syntax defined")))
	(ender (or (and continue comment-continue "")
                   comment-end))
	(begpos (comment-search-forward (line-end-position) t))
	cpos indent)
    (cond
     ;; If we couldn't find a comment *starting* on this line, see if we
     ;; are already within a multiline comment at BOL (bug#78003).
     ((and (not begpos) (not continue)
           comment-use-syntax comment-use-global-state
           (save-excursion (nth 4 (syntax-ppss (line-beginning-position)))))
      ;; We don't know anything about the nature of the multiline
      ;; construct, so immediately delegate to the mode.
      (indent-according-to-mode))
     ((and (not begpos) comment-insert-comment-function)
      ;; If no comment and c-i-c-f is set, let it do everything.
      (funcall comment-insert-comment-function))
     (t
      ;; An existing comment?
      (if begpos
	  (progn
	    (if (and (not (looking-at "[\t\n ]"))
		     (looking-at comment-end-skip))
		;; The comment is empty and we have skipped all its space
		;; and landed right before the comment-ender:
		;; Go back to the middle of the space.
		(forward-char (/ (skip-chars-backward " \t") -2)))
	    (setq cpos (point-marker)))
	;; If none, insert one.
	(save-excursion
	  ;; Some `comment-indent-function's insist on not moving
	  ;; comments that are in column 0, so we first go to the
	  ;; likely target column.
	  (indent-to comment-column)
	  ;; Ensure there's a space before the comment for things
	  ;; like sh where it matters (as well as being neater).
	  (unless (memq (char-before) '(nil ?\n ?\t ?\s))
	    (insert ?\s))
	  (setq begpos (point))
	  (insert starter)
	  (setq cpos (point-marker))
	  (insert ender)))
      (goto-char begpos)
      ;; Compute desired indent.
      (setq indent (save-excursion (funcall comment-indent-function)))
      ;; If `indent' is nil and there's code before the comment, we can't
      ;; use `indent-according-to-mode', so we default to comment-column.
      (unless (or indent (save-excursion (skip-chars-backward " \t") (bolp)))
	(setq indent comment-column))
      (if (not indent)
	  ;; comment-indent-function refuses: delegate to line-indent.
	  (indent-according-to-mode)
	;; If the comment is at the right of code, adjust the indentation.
	(unless (save-excursion (skip-chars-backward " \t") (bolp))
	  (setq indent (comment-choose-indent indent)))
	;; If that's different from comment's current position, change it.
	(unless (= (current-column) indent)
	  (delete-region (point) (progn (skip-chars-backward " \t") (point)))
	  (indent-to indent)))
      (goto-char cpos)
      (set-marker cpos nil)))))

(defun comment-set-column (arg)
  "Set the comment column based on point.
With no ARG, set the comment column to the current column.
With just minus as arg, kill any comment on this line.
With any other arg, set comment column to indentation of the previous comment
 and then align or create a comment on this line at that column."
  (interactive "P")
  (cond
   ((eq arg '-) (comment-kill nil))
   (arg
    (comment-normalize-vars)
    (save-excursion
      (beginning-of-line)
      (comment-search-backward)
      (beginning-of-line)
      (goto-char (comment-search-forward (line-end-position)))
      (setq comment-column (current-column))
      (message "Comment column set to %d" comment-column))
    (comment-indent))
   (t (setq comment-column (current-column))
      (message "Comment column set to %d" comment-column))))

(defun comment-kill (arg)
  "Kill the first comment on this line, if any.
With prefix ARG, kill comments on that many lines starting with this one."
  (interactive "P")
  (comment-normalize-vars)
  (dotimes (_i (prefix-numeric-value arg))
    (save-excursion
      (beginning-of-line)
      (let ((cs (comment-search-forward (line-end-position) t)))
	(when cs
	  (goto-char cs)
	  (skip-syntax-backward " ")
	  (setq cs (point))
	  (comment-forward)
	  (kill-region cs (if (bolp) (1- (point)) (point)))
	  (indent-according-to-mode))))
    (if arg (forward-line 1))))

(defun comment-padright (str &optional n)
  "Construct a string composed of STR plus `comment-padding'.
It also adds N copies of the last non-whitespace chars of STR.
If STR already contains padding, the corresponding amount is
ignored from `comment-padding'.
N defaults to 0.
If N is `re', a regexp is returned instead, that would match
the string for any N.

Ensure that `comment-normalize-vars' has been called before you use this."
  (setq n (or n 0))
  (when (and (stringp str) (string-match "\\S-" str))
    ;; Separate the actual string from any leading/trailing padding
    (string-match "\\`\\s-*\\(.*?\\)\\s-*\\'" str)
    (let ((s (match-string 1 str))                     ;actual string
	  (lpad (substring str 0 (match-beginning 1))) ;left padding
	  (rpad (concat
                 (substring str (match-end 1)) ;original right padding
                 (if (numberp comment-padding)
                     (make-string (min comment-padding
                                       (- (match-end 0) (match-end 1)))
                                  ?\s)
                   (if (not (string-match-p "\\`\\s-" comment-padding))
                       ;; If the padding isn't spaces, then don't
                       ;; shorten the padding.
                       comment-padding
		     (substring comment-padding ;additional right padding
			        (min (- (match-end 0) (match-end 1))
				     (length comment-padding)))))))
	  ;; We can only duplicate C if the comment-end has multiple chars
	  ;; or if comments can be nested, else the comment-end `}' would
	  ;; be turned into `}}}' where only the first ends the comment
	  ;; and the rest becomes bogus junk.
	  (multi (not (and comment-quote-nested
			   ;; comment-end is a single char
			   (string-match "\\`\\s-*\\S-\\s-*\\'" comment-end)))))
      (if (not (symbolp n))
	  (concat lpad s (when multi (make-string n (aref str (1- (match-end 1))))) rpad)
	;; construct a regexp that would match anything from just S
	;; to any possible output of this function for any N.
	(concat (mapconcat (lambda (c) (concat (regexp-quote (string c)) "?"))
			   lpad "")	;padding is not required
		(regexp-quote s)
		(when multi "+") ;the last char of S might be repeated
		(mapconcat (lambda (c) (concat (regexp-quote (string c)) "?"))
			   rpad ""))))))

(defun comment-padleft (str &optional n)
  "Construct a string composed of `comment-padding' plus STR.
It also adds N copies of the first non-whitespace chars of STR.
If STR already contains padding, the corresponding amount is
ignored from `comment-padding'.
N defaults to 0.
If N is `re', a regexp is returned instead, that would match the
string for any N.

Ensure that `comment-normalize-vars' has been called before you use this."
  (setq n (or n 0))
  (when (and (stringp str) (not (string= "" str)))
    ;; Only separate the left pad because we assume there is no right pad.
    (string-match "\\`\\s-*" str)
    (let ((s (substring str (match-end 0)))
	  (pad (concat (if (not (string-match-p "\\`\\s-" comment-padding))
                           ;; If the padding isn't spaces, then don't
                           ;; shorten the padding.
                           comment-padding
                         (substring comment-padding
				    (min (- (match-end 0) (match-beginning 0))
				         (length comment-padding))))
		       (match-string 0 str)))
	  (c (aref str (match-end 0)))	;the first non-space char of STR
	  ;; We can only duplicate C if the comment-end has multiple chars
	  ;; or if comments can be nested, else the comment-end `}' would
	  ;; be turned into `}}}' where only the first ends the comment
	  ;; and the rest becomes bogus junk.
	  (multi (not (and comment-quote-nested
			   ;; comment-end is a single char
			   (string-match "\\`\\s-*\\S-\\s-*\\'" comment-end)))))
      (if (not (symbolp n))
	  (concat pad (when multi (make-string n c)) s)
	;; Construct a regexp that would match anything from just S
	;; to any possible output of this function for any N.
	;; We match any number of leading spaces because this regexp will
	;; be used for uncommenting where we might want to remove
	;; uncomment markers with arbitrary leading space (because
	;; they were aligned).
	(concat "\\s-*"
		(if multi (concat (regexp-quote (string c)) "*"))
		(regexp-quote s))))))

(defun uncomment-region (beg end &optional arg)
  "Uncomment each line in the BEG .. END region.
The numeric prefix ARG can specify a number of chars to remove from the
comment delimiters."
  (interactive "*r\nP")
  (comment-normalize-vars)
  (when (> beg end) (setq beg (prog1 end (setq end beg))))
  ;; Bind `comment-use-global-state' to nil.  While uncommenting a region
  ;; (which works a line at a time), a comment can appear to be
  ;; included in a multi-line string, but it is actually not.
  (let ((comment-use-global-state nil))
    (save-excursion
      (funcall uncomment-region-function beg end arg))))

(defun uncomment-region-default-1 (beg end &optional arg)
  "Uncomment each line in the BEG .. END region.
The numeric prefix ARG can specify a number of chars to remove from the
comment delimiters.
This function is the default value of `uncomment-region-function'."
  (goto-char beg)
  (setq end (copy-marker end))
  (let* ((numarg (prefix-numeric-value arg))
	 (ccs comment-continue)
	 (srei (or (comment-padright ccs 're)
		   (and (stringp comment-continue) comment-continue)))
	 (csre (comment-padright comment-start 're))
	 (sre (and srei (concat "^\\s-*?\\(" srei "\\)")))
	 spt)
    (while (and (< (point) end)
		(setq spt (comment-search-forward end t)))
      (let ((ipt (point))
	    ;; Find the end of the comment.
	    (ept (progn
		   (goto-char spt)
		   (unless (or (comment-forward)
			       ;; Allow non-terminated comments.
			       (eobp))
		     (error "Can't find the comment end"))
		   (point)))
	    (box nil)
	    (box-equal nil))	   ;Whether we might be using `=' for boxes.
	(save-restriction
	  (narrow-to-region spt ept)

	  ;; Remove the comment-start.
	  (goto-char ipt)
	  (skip-syntax-backward " ")
	  ;; A box-comment starts with a looong comment-start marker.
	  (when (and (or (and (= (- (point) (point-min)) 1)
			      (setq box-equal t)
			      (looking-at "=\\{7\\}")
			      (not (eq (char-before (point-max)) ?\n))
			      (skip-chars-forward "="))
			 (> (- (point) (point-min) (length comment-start)) 7))
		     (> (count-lines (point-min) (point-max)) 2))
	    (setq box t))
	  ;; Skip the padding.  Padding can come from comment-padding and/or
	  ;; from comment-start, so we first check comment-start.
	  (if (or (save-excursion (goto-char (point-min)) (looking-at csre))
		  (looking-at (regexp-quote comment-padding)))
	      (goto-char (match-end 0)))
	  (when (and sre (looking-at (concat "\\s-*\n\\s-*" srei)))
	    (goto-char (match-end 0)))
	  (if (null arg) (delete-region (point-min) (point))
            (let ((opoint (point-marker)))
              (skip-syntax-backward " ")
              (delete-char (- numarg))
              (unless (and (not (bobp))
                           (save-excursion (goto-char (point-min))
                                           (looking-at comment-start-skip)))
                ;; If there's something left but it doesn't look like
                ;; a comment-start any more, just remove it.
                (delete-region (point-min) opoint))))

	  ;; Remove the end-comment (and leading padding and such).
	  (goto-char (point-max)) (comment-enter-backward)
	  ;; Check for special `=' used sometimes in comment-box.
	  (when (and box-equal (not (eq (char-before (point-max)) ?\n)))
	    (let ((pos (point)))
	      ;; skip `=' but only if there are at least 7.
	      (when (> (skip-chars-backward "=") -7) (goto-char pos))))
	  (unless (looking-at "\\(\n\\|\\s-\\)*\\'")
	    (when (and (bolp) (not (bobp))) (backward-char))
	    (if (null arg) (delete-region (point) (point-max))
	      (skip-syntax-forward " ")
	      (delete-char numarg)
	      (unless (or (eobp) (looking-at comment-end-skip))
		;; If there's something left but it doesn't look like
		;; a comment-end any more, just remove it.
		(delete-region (point) (point-max)))))

	  ;; Unquote any nested end-comment.
	  (comment-quote-nested comment-start comment-end t)

	  ;; Eliminate continuation markers as well.
	  (when sre
	    (let* ((cce (comment-string-reverse (or comment-continue
						    comment-start)))
		   (erei (and box (comment-padleft cce 're)))
		   (ere (and erei (concat "\\(" erei "\\)\\s-*$"))))
	      (goto-char (point-min))
	      (while (progn
		       (if (and ere (re-search-forward
				     ere (line-end-position) t))
			   (replace-match "" t t nil (if (match-end 2) 2 1))
			 (setq ere nil))
		       (forward-line 1)
		       (re-search-forward sre (line-end-position) t))
		(replace-match "" t t nil (if (match-end 2) 2 1)))))
	  ;; Go to the end for the next comment.
	  (goto-char (point-max)))
        ;; Remove any obtrusive spaces left preceding a tab at `spt'.
        (when (and (eq (char-after spt) ?\t) (eq (char-before spt) ? )
                   (> tab-width 0))
          (save-excursion
            (goto-char spt)
            (let* ((fcol (current-column))
                   (slim (- (point) (mod fcol tab-width))))
              (delete-char (- (skip-chars-backward " " slim)))))))))
  (set-marker end nil))

(defun uncomment-region-default (beg end &optional arg)
  "Uncomment each line in the BEG .. END region.
The numeric prefix ARG can specify a number of chars to remove from the
comment markers."
  (if comment-combine-change-calls
      (combine-change-calls beg end (uncomment-region-default-1 beg end arg))
    (uncomment-region-default-1 beg end arg)))

(defun comment-make-bol-ws (len)
  "Make a white-space string of width LEN for use at BOL.
When `indent-tabs-mode' is non-nil, tab characters will be used."
  (if (and indent-tabs-mode (> tab-width 0))
      (concat (make-string (/ len tab-width) ?\t)
	      (make-string (% len tab-width) ? ))
    (make-string len ? )))

(defun comment-make-extra-lines (cs ce ccs cce min-indent max-indent &optional block)
  "Make the leading and trailing extra lines.
This is used for `extra-line' style (or `box' style if BLOCK is specified)."
  (let ((eindent 0))
    (if (not block)
	;; Try to match CS and CE's content so they align aesthetically.
	(progn
	  (setq ce (comment-string-strip ce t t))
	  (when (string-match "\\(.+\\).*\n\\(.*?\\)\\1" (concat ce "\n" cs))
	    (setq eindent
		  (max (- (match-end 2) (match-beginning 2) (match-beginning 0))
		       0))))
      ;; box comment
      (let* ((width (- max-indent min-indent))
	     (s (concat cs "a=m" cce))
	     (e (concat ccs "a=m" ce))
	     (c (if (string-match ".*\\S-\\S-" cs)
		    (aref cs (1- (match-end 0)))
		  (if (and (equal comment-end "") (string-match ".*\\S-" cs))
		      (aref cs (1- (match-end 0))) ?=)))
	     (re "\\s-*a=m\\s-*")
	     (_ (string-match re s))
	     (lcs (length cs))
	     (fill
	      (make-string (+ width (- (match-end 0)
				       (match-beginning 0) lcs 3)) c)))
	(setq cs (replace-match fill t t s))
	(when (and (not (string-match comment-start-skip cs))
		   (string-match "a=m" s))
	  ;; The whitespace around CS cannot be ignored: put it back.
	  (setq re "a=m")
	  (setq fill (make-string (- width lcs) c))
	  (setq cs (replace-match fill t t s)))
	(string-match re e)
	(setq ce (replace-match fill t t e))))
    (cons (concat cs "\n" (comment-make-bol-ws min-indent) ccs)
	  (concat cce "\n" (comment-make-bol-ws (+ min-indent eindent)) ce))))

(defmacro comment-with-narrowing (beg end &rest body)
  "Execute BODY with BEG..END narrowing.
Space is added (and then removed) at the beginning for the text's
indentation to be kept as it was before narrowing."
  (declare (debug t) (indent 2))
  (let ((bindent (make-symbol "bindent")))
    `(let ((,bindent (save-excursion (goto-char ,beg) (current-column))))
       (save-restriction
	 (narrow-to-region ,beg ,end)
	 (goto-char (point-min))
	 (insert (make-string ,bindent ? ))
	 (prog1
	     (progn ,@body)
	   ;; remove the bindent
	   (save-excursion
	     (goto-char (point-min))
	     (when (looking-at " *")
	       (let ((n (min (- (match-end 0) (match-beginning 0)) ,bindent)))
		 (delete-char n)
		 (setq ,bindent (- ,bindent n))))
	     (end-of-line)
	     (let ((e (point)))
	       (beginning-of-line)
	       (while (and (> ,bindent 0) (re-search-forward "   *" e t))
		 (let ((n (min ,bindent (- (match-end 0) (match-beginning 0) 1))))
		   (goto-char (match-beginning 0))
		   (delete-char n)
		   (setq ,bindent (- ,bindent n)))))))))))

(defvar comment-add 0
  "How many more comment chars should be inserted by `comment-region'.
This determines the default value of the numeric argument of `comment-region'.
The `plain' comment style doubles this value.

This should generally stay 0, except for a few modes like Lisp where
it is 1 so that regions are commented with two or three semi-colons.")

(defun comment-add (arg)
  "Compute the number of extra comment starter characters.
\(Extra semicolons in Lisp mode, extra stars in C mode, etc.)
If ARG is non-nil, just follow ARG.
If the comment starter is multi-char, just follow ARG.
Otherwise obey `comment-add'."
  (if (and (null arg) (= (string-match "[ \t]*\\'" comment-start) 1))
      (* comment-add 1)
    (1- (prefix-numeric-value arg))))

(defun comment-region-internal (beg end cs ce
                                &optional ccs cce block lines indent)
  "Comment region BEG .. END.
CS and CE are the comment start string and comment end string,
respectively.  CCS and CCE are the comment continuation strings
for the start and end of lines, respectively (default to CS and CE).
BLOCK indicates that end of lines should be marked with either CCE,
CE or CS \(if CE is empty) and that those markers should be aligned.
LINES indicates that an extra lines will be used at the beginning
and end of the region for CE and CS.
INDENT indicates to put CS and CCS at the current indentation of
the region rather than at left margin."
  ;;(assert (< beg end))
  (let ((no-empty (not (or (eq comment-empty-lines t)
			   (and comment-empty-lines (zerop (length ce))))))
	ce-sanitized)
    ;; Sanitize CE and CCE.
    (if (and (stringp ce) (string= "" ce)) (setq ce nil))
    (setq ce-sanitized ce)
    (if (and (stringp cce) (string= "" cce)) (setq cce nil))
    ;; If CE is empty, multiline cannot be used.
    (unless ce (setq ccs nil cce nil))
    ;; Should we mark empty lines as well ?
    (if (or ccs block lines) (setq no-empty nil))
    ;; Make sure we have end-markers for BLOCK mode.
    (when block (unless ce (setq ce (comment-string-reverse cs))))
    ;; If BLOCK is not requested, we don't need CCE.
    (unless block (setq cce nil))
    ;; Continuation defaults to the same as CS and CE.
    (unless ccs (setq ccs cs cce ce))

    (save-excursion
      (goto-char end)
      ;; If the end is not at the end of a line and the comment-end
      ;; is implicit (i.e. a newline), explicitly insert a newline.
      (unless (or ce-sanitized (eolp)) (insert "\n") (indent-according-to-mode))
      (comment-with-narrowing beg end
	(let ((min-indent (point-max))
	      (max-indent 0))
	  (goto-char (point-min))
	  ;; Quote any nested comment marker
	  (comment-quote-nested comment-start comment-end nil)

	  ;; Loop over all lines to find the needed indentations.
	  (goto-char (point-min))
	  (while
	      (progn
		(unless (looking-at "[ \t]*$")
		  (setq min-indent (min min-indent (current-indentation))))
		(end-of-line)
		(setq max-indent (max max-indent (current-column)))
		(not (or (eobp) (progn (forward-line) nil)))))

	  (setq max-indent
		(+ max-indent (max (length cs) (length ccs))
                   ;; Inserting ccs can change max-indent by (1- tab-width)
                   ;; but only if there are TABs in the boxed text, of course.
                   (if (save-excursion (goto-char beg)
                                       (search-forward "\t" end t))
                       (1- tab-width) 0)))
	  (unless indent (setq min-indent 0))

	  ;; make the leading and trailing lines if requested
	  (when lines
            ;; Trim trailing whitespace from cs if there's some.
            (setq cs (string-trim-right cs))

	    (let ((csce
		   (comment-make-extra-lines
		    cs ce ccs cce min-indent max-indent block)))
	      (setq cs (car csce))
	      (setq ce (cdr csce))))

	  (goto-char (point-min))
	  ;; Loop over all lines from BEG to END.
	  (while
	      (progn
		(unless (and no-empty (looking-at "[ \t]*$"))
		  (move-to-column min-indent t)
		  (insert cs) (setq cs ccs) ;switch to CCS after the first line
		  (end-of-line)
		  (if (eobp) (setq cce ce))
		  (when cce
		    (when block (move-to-column max-indent t))
		    (insert cce)))
		(end-of-line)
		(not (or (eobp) (progn (forward-line) nil))))))))))

(defun comment-region (beg end &optional arg)
  "Comment or uncomment each line in the region.
With just \\[universal-argument] prefix arg, uncomment each line in region BEG .. END.
Numeric prefix ARG means use ARG comment characters.
If ARG is negative, delete that many comment characters instead.

The strings used as comment starts are built from `comment-start'
and `comment-padding'; the strings used as comment ends are built
from `comment-end' and `comment-padding'.

By default, the `comment-start' markers are inserted at the
current indentation of the region, and comments are terminated on
each line (even for syntaxes in which newline does not end the
comment and blank lines do not get comments).  This can be
changed with `comment-style'."
  (interactive "*r\nP")
  (comment-normalize-vars)
  (if (> beg end) (let (mid) (setq mid beg beg end end mid)))
  (save-excursion
    ;; FIXME: maybe we should call uncomment depending on ARG.
    (funcall comment-region-function beg end arg)))

(defun comment-region-default-1 (beg end &optional arg)
  (let* ((numarg (prefix-numeric-value arg))
	 (style (cdr (assoc comment-style comment-styles)))
	 (lines (nth 2 style))
	 (block (nth 1 style))
	 (multi (nth 0 style)))

    ;; We use `chars' instead of `syntax' because `\n' might be
    ;; of end-comment syntax rather than of whitespace syntax.
    ;; sanitize BEG and END
    (goto-char beg) (skip-chars-forward " \t\n\r") (beginning-of-line)
    (setq beg (max beg (point)))
    (goto-char end) (skip-chars-backward " \t\n\r") (end-of-line)
    (setq end (min end (point)))
    (if (>= beg end) (error "Nothing to comment"))

    ;; sanitize LINES
    (setq lines
	  (and
	   lines ;; multi
	   (progn (goto-char beg) (beginning-of-line)
		  (skip-syntax-forward " ")
		  (>= (point) beg))
	   (progn (goto-char end) (end-of-line) (skip-syntax-backward " ")
		  (<= (point) end))
	   (or block (not (string= "" comment-end)))
           (or block (progn (goto-char beg) (re-search-forward "$" end t)))))

    ;; don't add end-markers just because the user asked for `block'
    (unless (or lines (string= "" comment-end)) (setq block nil))

    (cond
     ((consp arg) (uncomment-region beg end))
     ((< numarg 0) (uncomment-region beg end (- numarg)))
     (t
      (let ((multi-char (/= (string-match "[ \t]*\\'" comment-start) 1))
	    indent triple)
	(if (eq (nth 3 style) 'multi-char)
	    (save-excursion
	      (goto-char beg)
	      (setq indent multi-char
		    ;; Triple if we will put the comment starter at the margin
		    ;; and the first line of the region isn't indented
		    ;; at least two spaces.
		    triple (and (not multi-char) (looking-at "\t\\|  "))))
	  (setq indent (nth 3 style)))

	;; In Lisp and similar modes with one-character comment starters,
	;; double it by default if `comment-add' says so.
	;; If it isn't indented, triple it.
	(if (and (null arg) (not multi-char))
	    (setq numarg (* comment-add (if triple 2 1)))
	  (setq numarg (1- (prefix-numeric-value arg))))

	(comment-region-internal
	 beg end
	 (let ((s (comment-padright comment-start numarg)))
	   (if (string-match comment-start-skip s) s
	     (comment-padright comment-start)))
	 (let ((s (comment-padleft comment-end numarg)))
	   (and s (if (string-match comment-end-skip s) s
		    (comment-padright comment-end))))
	 (if multi
             (or (comment-padright comment-continue numarg)
                 ;; `comment-padright' returns nil when
                 ;; `comment-continue' contains only whitespace
                 (and (stringp comment-continue) comment-continue)))
	 (if multi
	     (comment-padleft (comment-string-reverse comment-continue) numarg))
	 block
	 lines
	 indent))))))

(defun comment-region-default (beg end &optional arg)
  (if comment-combine-change-calls
      (combine-change-calls beg
          ;; A new line might get inserted and whitespace deleted
          ;; after END for line comments.  Ensure the next argument is
          ;; after any and all changes.
          (save-excursion
            (goto-char end)
            (forward-line)
            (point))
        (comment-region-default-1 beg end arg))
    (comment-region-default-1 beg end arg)))

(defun comment-box (beg end &optional arg)
  "Comment out the BEG .. END region, putting it inside a box.
The numeric prefix ARG specifies how many characters to add to begin- and
end- comment markers additionally to what variable `comment-add' already
specifies."
  (interactive "*r\np")
  (comment-normalize-vars)
  (let ((comment-style (if (cadr (assoc comment-style comment-styles))
			   'box-multi 'box)))
    (comment-region beg end (+ comment-add arg))))

(defun comment-only-p (beg end)
  "Return non-nil if the text between BEG and END is all comments."
  (save-excursion
    (goto-char beg)
    (comment-forward (point-max))
    (<= end (point))))

(defun comment-or-uncomment-region (beg end &optional arg)
  "Call `comment-region', unless the region only consists of comments,
in which case call `uncomment-region'.  If a prefix arg is given, it
is passed on to the respective function."
  (interactive "*r\nP")
  (comment-normalize-vars)
  (funcall (if (comment-only-p beg end)
	       'uncomment-region 'comment-region)
	   beg end arg))

(defun comment-dwim (arg)
  "Call the comment command you want (Do What I Mean).
If the region is active and `transient-mark-mode' is on, call
`comment-region' (unless it only consists of comments, in which
case it calls `uncomment-region'); in this case, prefix numeric
argument ARG specifies how many characters to remove from each
comment delimiter (so don't specify a prefix argument whose value
is greater than the total length of the comment delimiters).
Else, if the current line is empty, call `comment-insert-comment-function'
if it is defined, otherwise insert a comment and indent it.
Else, if a prefix ARG is specified, call `comment-kill'; in this
case, prefix numeric argument ARG specifies on how many lines to kill
the comments.
Else, call `comment-indent'.
You can configure `comment-style' to change the way regions are commented."
  (interactive "*P")
  (comment-normalize-vars)
  (if (use-region-p)
      (comment-or-uncomment-region (region-beginning) (region-end) arg)
    (if (save-excursion (beginning-of-line) (not (looking-at "\\s-*$")))
	;; FIXME: If there's no comment to kill on this line and ARG is
	;; specified, calling comment-kill is not very clever.
	(if arg (comment-kill (and (integerp arg) arg)) (comment-indent))
      ;; Inserting a comment on a blank line. comment-indent calls
      ;; c-i-c-f if needed in the non-blank case.
      (if comment-insert-comment-function
          (funcall comment-insert-comment-function)
        (let ((add (comment-add arg)))
          ;; Some modes insist on keeping column 0 comment in column 0
          ;; so we need to move away from it before inserting the comment.
          (indent-according-to-mode)
          (insert (comment-padright comment-start add))
          (save-excursion
            (unless (string= "" comment-end)
              (insert (comment-padleft comment-end add)))
            (indent-according-to-mode)))))))

(defun comment-valid-prefix-p (prefix compos)
    "Check that the adaptive fill prefix is consistent with the context.
PREFIX is the prefix (presumably guessed by `adaptive-fill-mode').
COMPOS is the position of the beginning of the comment we're in, or nil
if we're not inside a comment."
  ;; This consistency checking is mostly needed to workaround the limitation
  ;; of auto-fill-mode whose paragraph-determination doesn't pay attention
  ;; to comment boundaries.
  (if (null compos)
      ;; We're not inside a comment: the prefix shouldn't match
      ;; a comment-starter.
      (not (and comment-start comment-start-skip
                (string-match comment-start-skip prefix)))
    (or
     ;; Accept any prefix if the current comment is not EOL-terminated.
     (save-excursion (goto-char compos) (comment-forward) (not (bolp)))
     ;; Accept any prefix that starts with the same comment-start marker
     ;; as the current one.
     (when (string-match (concat "\\`[ \t]*\\(?:" comment-start-skip "\\)")
                         prefix)
       (let ((prefix-com (comment-string-strip (match-string 0 prefix) nil t)))
         (string-match "\\`[ \t]*" prefix-com)
         (let* ((prefix-space (match-string 0 prefix-com))
                (prefix-indent (string-width prefix-space))
                (prefix-comstart (substring prefix-com (match-end 0))))
           (save-excursion
             (goto-char compos)
             ;; The comstart marker is the same.
             (and (looking-at (regexp-quote prefix-comstart))
                  ;; The indentation as well.
                  (or (= prefix-indent
                         (- (current-column) (current-left-margin)))
                      ;; Check the indentation in two different ways, just
                      ;; to try and avoid most of the potential funny cases.
                      (equal prefix-space
                             (buffer-substring (point)
                                               (progn (move-to-left-margin)
                                                      (point)))))))))))))

(defun comment-indent-new-line (&optional soft)
  "Break line at point and indent, continuing comment if within one.
This indents the body of the continued comment
under the previous comment line.

This command is intended for styles where you write a comment per line,
starting a new comment (and terminating it if necessary) on each line.
If you want to continue one comment across several lines, use \\[newline-and-indent].

If a fill column is specified, it overrides the use of the comment column
or comment indentation.

The inserted newline is marked hard if variable `use-hard-newlines' is true,
unless optional argument SOFT is non-nil."
  (interactive)
  (comment-normalize-vars t)
  (let (compos comin)
    ;; If we are not inside a comment and we only auto-fill comments,
    ;; don't do anything (unless no comment syntax is defined).
    (unless (and comment-start
		 comment-auto-fill-only-comments
		 (not (called-interactively-p 'interactive))
		 (not (save-excursion
			(prog1 (setq compos (comment-beginning))
			  (setq comin (point))))))

      ;; Now we know we should auto-fill.
      ;; Insert the newline before removing empty space so that markers
      ;; get preserved better.
      (if soft (insert-and-inherit ?\n) (newline 1))
      (save-excursion (forward-char -1) (delete-horizontal-space))
      (delete-horizontal-space)

      (if (and fill-prefix (not adaptive-fill-mode))
	  ;; Blindly trust a non-adaptive fill-prefix.
	  (progn
	    (indent-to-left-margin)
	    (insert-before-markers-and-inherit fill-prefix))

	;; If necessary check whether we're inside a comment.
	(unless (or compos (null comment-start))
	  (save-excursion
	    (backward-char)
	    (setq compos (comment-beginning))
	    (setq comin (point))))

	(cond
	 ;; If there's an adaptive prefix, use it unless we're inside
	 ;; a comment and the prefix is not a comment starter.
	 ((and fill-prefix
               (comment-valid-prefix-p fill-prefix compos))
	  (indent-to-left-margin)
	  (insert-and-inherit fill-prefix))
	 ;; If we're not inside a comment, just try to indent.
	 ((not compos) (indent-according-to-mode))
	 (t
	  (let* ((comstart (buffer-substring compos comin))
		 (normalp
		  (string-match (regexp-quote (comment-string-strip
					       comment-start t t))
				comstart))
		 (comend
		  (if normalp comment-end
		    ;; The comment starter is not the normal comment-start
		    ;; so we can't just use comment-end.
		    (save-excursion
		      (goto-char compos)
		      (if (not (comment-forward)) comment-end
			(comment-string-strip
			 (buffer-substring
			  (save-excursion (comment-enter-backward) (point))
			  (point))
			 nil t))))))
	    (if (and comment-multi-line (> (length comend) 0))
		(indent-according-to-mode)
	      (insert-and-inherit ?\n)
	      (forward-char -1)
              (let* ((comment-column
                      ;; The continuation indentation should be somewhere
                      ;; between the current line's indentation (plus 2 for
                      ;; good measure) and the current comment's indentation,
                      ;; with a preference for comment-column.
                      (save-excursion
                        ;; FIXME: use prev line's info rather than first
                        ;; line's.
                        (goto-char compos)
                        (min (current-column)
                             (max comment-column
                                  (+ 2 (current-indentation))))))
                     (comment-indent-function
                      ;; If the previous comment is on its own line, then
                      ;; reuse its indentation unconditionally.
                      ;; Important for modes like Python/Haskell where
                      ;; auto-indentation is unreliable.
                      (if (save-excursion (goto-char compos)
                                          (skip-chars-backward " \t")
                                          (bolp))
                          (lambda () comment-column) comment-indent-function))
                     (comment-start comstart)
                     (comment-end comend)
                     (continuep (or comment-multi-line
                                    (cadr (assoc comment-style
                                                 comment-styles))))
                     ;; Recreate comment-continue from comment-start.
                     ;; FIXME: wrong if comment-continue was set explicitly!
                     ;; FIXME: use prev line's continuation if available.
                     (comment-continue nil))
                (comment-indent continuep))
	      (save-excursion
		(let ((pt (point)))
		  (end-of-line)
		  (let ((comend (buffer-substring pt (point))))
		    ;; The 1+ is to make sure we delete the \n inserted above.
		    (delete-region pt (1+ (point)))
		    (end-of-line 0)
		    (insert comend))))))))))))

(defun comment-line (n)
  "Comment or uncomment current line and leave point after it.
With positive prefix, apply to N lines including current one.
With negative prefix, apply to -N lines above.  Also, further
consecutive invocations of this command will inherit the negative
argument.

If region is active, comment lines in active region instead.
Unlike `comment-dwim', this always comments whole lines."
  (interactive "p")
  (if (use-region-p)
      (comment-or-uncomment-region
       (save-excursion
         (goto-char (region-beginning))
         (line-beginning-position))
       (save-excursion
         (goto-char (region-end))
         (line-end-position)))
    (when (and (eq last-command 'comment-line-backward)
               (natnump n))
      (setq n (- n)))
    (let ((range
           (list (line-beginning-position)
                 (goto-char (line-end-position n)))))
      (comment-or-uncomment-region
       (apply #'min range)
       (apply #'max range)))
    (forward-line 1)
    (back-to-indentation)
    (unless (natnump n) (setq this-command 'comment-line-backward))))

(defvar comment-use-syntax 'undecided
  "Non-nil if syntax-tables can be used instead of regexps.
Can also be `undecided' which means that a somewhat expensive test will
be used to try to determine whether syntax-tables should be trusted
to understand comments or not in the given buffer.
Major modes should set this variable.")

(defvar comment-fill-column nil
  "Column to use for `comment-indent'.  If nil, use `fill-column' instead."
  :type '(choice (const nil) integer)
  :group 'comment)

(defvar comment-end-skip nil
  "Regexp to match the end of a comment plus everything back to its body.")

(defvar comment-indent-function 'comment-indent-default
  "Function to compute desired indentation for a comment.
This function is called with no args with point at the beginning
of the comment's starting delimiter and should return either the
desired column indentation, a range of acceptable
indentation (MIN . MAX), or nil.
If nil is returned, indentation is delegated to `indent-according-to-mode'.")

(defvar comment-insert-comment-function nil
  "Function to insert a comment when a line doesn't contain one.
The function has no args.

Applicable at least in modes for languages like fixed-format Fortran where
comments always start in column zero.")

(defvar comment-region-function 'comment-region-default
  "Function to comment a region.
Its args are the same as those of `comment-region', but BEG and END are
guaranteed to be correctly ordered.  It is called within `save-excursion'.

Applicable at least in modes for languages like fixed-format Fortran where
comments always start in column zero.")

(defvar uncomment-region-function 'uncomment-region-default
  "Function to uncomment a region.
Its args are the same as those of `uncomment-region', but BEG and END are
guaranteed to be correctly ordered.  It is called within `save-excursion'.

Applicable at least in modes for languages like fixed-format Fortran where
comments always start in column zero.")

(defvar comment-quote-nested-function #'comment-quote-nested-default
  "Function to quote nested comments in a region.
It takes the same arguments as `comment-quote-nested-default',
and is called with the buffer narrowed to a single comment.")

(defvar comment-continue nil
  "Continuation string to insert for multiline comments.
This string will be added at the beginning of each line except the very
first one when commenting a region with a commenting style that allows
comments to span several lines.
It should generally have the same length as `comment-start' in order to
preserve indentation.
If it is nil a value will be automatically derived from `comment-start'
by replacing its first character with a space.")

(defconst comment-styles
  '((plain      nil nil nil nil
                "Start in column 0 (do not indent), as in Emacs-20")
    (indent-or-triple nil nil nil multi-char
              "Start in column 0, but only for single-char starters")
    (indent     nil nil nil t
                "Full comment per line, ends not aligned")
    (aligned	nil t   nil t
                "Full comment per line, ends aligned")
    (box	nil t   t   t
                "Full comment per line, ends aligned, + top and bottom")
    (extra-line	t   nil t   t
                "One comment for all lines, end on a line by itself")
    (multi-line	t   nil nil t
                "One comment for all lines, end on last commented line")
    (box-multi	t   t   t   t
                "One comment for all lines, + top and bottom"))
  "Comment region style definitions.
Each style is defined with a form (STYLE . (MULTI ALIGN EXTRA INDENT DOC)).
DOC should succinctly describe the style.
STYLE should be a mnemonic symbol.
MULTI specifies that comments are allowed to span multiple lines.
  e.g. in C it comments regions as
     /* blabla
      * bli */
  rather than
     /* blabla */
     /* bli */
  if `comment-end' is empty, this has no effect.

ALIGN specifies that the `comment-end' markers should be aligned.
  e.g. in C it comments regions as
     /* blabla */
     /* bli    */
  rather than
     /* blabla */
     /* bli */
  if `comment-end' is empty, this has no effect, unless EXTRA is also set,
  in which case the comment gets wrapped in a box.

EXTRA specifies that an extra line should be used before and after the
  region to comment (to put the `comment-end' and `comment-start').
  e.g. in C it comments regions as
     /*
      * blabla
      * bli
      */
  rather than
     /* blabla
      * bli */
  if the comment style is not multi line, this has no effect, unless ALIGN
  is also set, in which case the comment gets wrapped in a box.

INDENT specifies that the `comment-start' markers should not be put at the
  left margin but at the current indentation of the region to comment.
If INDENT is `multi-char', that means indent multi-character
  comment starters, but not one-character comment starters.")

(defvar comment-style 'indent
  "Style to be used for `comment-region'.
See `comment-styles' for a list of available styles."
  :type (if (boundp 'comment-styles)
	    `(choice
              ,@(mapcar (lambda (s)
                          `(const :tag ,(format "%s: %s" (car s) (nth 5 s))
                                  ,(car s)))
                        comment-styles))
	  'symbol)
  :version "23.1"
  :group 'comment)

(defvar comment-padding " "
  "Padding string that `comment-region' puts between comment chars and text.
Can also be an integer which will be automatically turned into a string
of the corresponding number of spaces.

Extra spacing between the comment characters and the comment text
makes the comment easier to read.  Default is 1.  nil means 0."
  :type '(choice string integer (const nil))
  :group 'comment)

(defvar comment-inline-offset 1
  "Inline comments have to be preceded by at least this many spaces.
This is useful when style-conventions require a certain minimal offset.
Python's PEP8 for example recommends two spaces, so you could do:

\(add-hook \\='python-mode-hook
   (lambda () (setq-local comment-inline-offset 2)))

See `comment-padding' for whole-line comments."
  :version "24.3"
  :type 'integer
  :group 'comment)

(defvar comment-multi-line nil
  "Non-nil means `comment-indent-new-line' continues comments.
That is, it inserts no new terminator or starter.
This affects `auto-fill-mode', which is the main reason to
customize this variable.

It also affects \\[indent-new-comment-line].  However, if you want this
behavior for explicit filling, you might as well use \\[newline-and-indent]."
  :type 'boolean
  :safe #'booleanp
  :group 'comment)

(defvar comment-empty-lines nil
  "If nil, `comment-region' does not comment out empty lines.
If t, it always comments out empty lines.
If `eol', it only comments out empty lines if comments are
terminated by the end of line (i.e., `comment-end' is empty)."
  :type '(choice (const :tag "Never" nil)
                 (const :tag "Always" t)
                 (const :tag "EOL-terminated" eol))
  :group 'comment)

(defvar comment-setup-function #'ignore
  "Function to set up variables needed by commenting functions.")

(defvar comment-use-global-state t
  "Non-nil means that the global syntactic context is used.
More specifically, it means that `syntax-ppss' is used to find out whether
point is within a string or not.  Major modes whose syntax is not faithfully
described by the syntax-tables (or where `font-lock-syntax-table' is radically
different from the main syntax table) can set this to nil,
then `syntax-ppss' cache won't be used in comment-related routines.")

(defvar comment-auto-fill-only-comments nil
  "Non-nil means to only auto-fill inside comments.
This has no effect in modes that do not define a comment syntax."
  :type 'boolean
  :group 'comment)

(defvar comment-combine-change-calls t
  "If non-nil (the default), use `combine-change-calls' around
calls of `comment-region-function' and
`uncomment-region-function'.  This Substitutes a single call to
each of the hooks `before-change-functions' and
`after-change-functions' in place of those hooks being called
for each individual buffer change.")

(defun set-fill-prefix (&optional arg)
  "Set the fill prefix to the current line up to point.
Filling expects lines to start with the fill prefix and
reinserts the fill prefix in each resulting line.
With a prefix argument, cancel the fill prefix."
  (interactive "P")
  (if arg
      (setq fill-prefix nil)
    (let ((left-margin-pos (save-excursion (move-to-left-margin) (point))))
      (if (> (point) left-margin-pos)
	  (progn
	    (setq fill-prefix (buffer-substring left-margin-pos (point)))
	    (when (equal fill-prefix "")
	      (setq fill-prefix nil)))
        (setq fill-prefix nil))))
  (if fill-prefix
      (message "fill-prefix: \"%s\"" fill-prefix)
    (message "fill-prefix cancelled")))

(defun current-fill-column ()
  "Return the fill-column to use for this line.
The fill-column to use for a buffer is stored in the variable `fill-column',
but can be locally modified by the `right-margin' text property, which is
subtracted from `fill-column'.

The fill column to use for a line is the first column at which the column
number equals or exceeds the local fill-column - right-margin difference."
  (save-excursion
    (if fill-column
	(let* ((here (line-beginning-position))
	       (here-col 0)
	       (eol (progn (end-of-line) (point)))
	       margin fill-col change col)
	  ;; Look separately at each region of line with a different
	  ;; right-margin.
	  (while (and (setq margin (get-text-property here 'right-margin)
			    fill-col (- fill-column (or margin 0))
			    change (text-property-not-all
				    here eol 'right-margin margin))
		      (progn (goto-char (1- change))
			     (setq col (current-column))
			     (< col fill-col)))
	    (setq here change
		  here-col col))
	  (max here-col fill-col))
      ;; This warning was added in 28.1.  It should be removed later,
      ;; and this function changed to never return nil.
      (unless current-fill-column--has-warned
        (lwarn '(fill-column) :warning
               "Setting this variable to nil is obsolete; use `(auto-fill-mode -1)' instead")
        (setq current-fill-column--has-warned t))
      most-positive-fixnum)))

(defun canonically-space-region (beg end)
  "Remove extra spaces between words in region.
Leave one space between words, two at end of sentences or after colons
\(depending on values of `sentence-end-double-space', `colon-double-space',
and `sentence-end-without-period').
Remove indentation from each line."
  (interactive "*r")
  ;; Ideally, we'd want to scan the text from the end, so that changes to
  ;; text don't affect the boundary, but the regexp we match against does
  ;; not match as eagerly when matching backward, so we instead use
  ;; a marker.
  (unless (markerp end) (setq end (copy-marker end t)))
  (let ((end-spc-re (concat "\\(" (sentence-end) "\\) *\\|  +")))
    (save-excursion
      (goto-char beg)
      ;; Nuke tabs; they get screwed up in a fill.
      ;; This is quick, but loses when a tab follows the end of a sentence.
      ;; Actually, it is difficult to tell that from "Mr.\tSmith".
      ;; Blame the typist.
      (subst-char-in-region beg end ?\t ?\s)
      (while (and (< (point) end)
		  (re-search-forward end-spc-re end t))
	(delete-region
	 (cond
	  ;; `sentence-end' matched and did not match all spaces.
	  ;; I.e. it only matched the number of spaces it needs: drop the rest.
	  ((and (match-end 1) (> (match-end 0) (match-end 1)))  (match-end 1))
	  ;; `sentence-end' matched but with nothing left.  Either that means
	  ;; nothing should be removed, or it means it's the "old-style"
	  ;; sentence-end which matches all it can.  Keep only 2 spaces.
	  ;; We probably don't even need to check `sentence-end-double-space'.
	  ((match-end 1)
	   (min (match-end 0)
		(+ (if sentence-end-double-space 2 1)
		   (save-excursion (goto-char (match-end 0))
				   (skip-chars-backward " ")
				   (point)))))
	  (t ;; It's not an end of sentence.
	   (+ (match-beginning 0)
	      ;; Determine number of spaces to leave:
	      (save-excursion
		(skip-chars-backward " ]})\"'")
		(cond ((and sentence-end-double-space
			    (or (memq (preceding-char) '(?. ?? ?!))
				(and sentence-end-without-period
				     (= (char-syntax (preceding-char)) ?w)))) 2)
		      ((and colon-double-space
			    (= (preceding-char) ?:))  2)
		      ((char-equal (preceding-char) ?\n)  0)
		      (t 1))))))
	 (match-end 0))))))

(defun fill-common-string-prefix (s1 s2)
  "Return the longest common prefix of strings S1 and S2, or nil if none."
  (let ((cmp (compare-strings s1 nil nil s2 nil nil)))
    (if (eq cmp t)
	s1
      (setq cmp (1- (abs cmp)))
      (unless (zerop cmp)
	(substring s1 0 cmp)))))

(defun fill-match-adaptive-prefix ()
  (let ((str (or
              (and adaptive-fill-function (funcall adaptive-fill-function))
              (and adaptive-fill-regexp (looking-at adaptive-fill-regexp)
                   (match-string 0)))))
    (if (>= (+ (current-left-margin) (length str)) (current-fill-column))
        ;; Death to insanely long prefixes.
        nil
      str)))

(defun fill-context-prefix (from to &optional first-line-regexp)
  "Compute a fill prefix from the text between FROM and TO.
This uses the variables `adaptive-fill-regexp' and `adaptive-fill-function'
and `adaptive-fill-first-line-regexp'.  `paragraph-start' also plays a role;
we reject a prefix based on a one-line paragraph if that prefix would
act as a paragraph-separator."
  (or first-line-regexp
      (setq first-line-regexp adaptive-fill-first-line-regexp))
  (save-excursion
    (goto-char from)
    (if (eolp) (forward-line 1))
    ;; Move to the second line unless there is just one.
    (move-to-left-margin)
    (let (first-line-prefix
	  ;; Non-nil if we are on the second line.
	  second-line-prefix)
      (setq first-line-prefix
	    ;; We don't need to consider `paragraph-start' here since it
	    ;; will be explicitly checked later on.
	    ;; Also setting first-line-prefix to nil prevents
	    ;; second-line-prefix from being used.
	    ;; ((looking-at paragraph-start) nil)
	    (fill-match-adaptive-prefix))
      (forward-line 1)
      (if (< (point) to)
          (progn
            (move-to-left-margin)
            (setq second-line-prefix
                  (cond ((looking-at paragraph-start) nil) ;Can it happen? -Stef
                        (t (fill-match-adaptive-prefix))))
            ;; If we get a fill prefix from the second line,
            ;; make sure it or something compatible is on the first line too.
            (when second-line-prefix
              (unless first-line-prefix (setq first-line-prefix ""))
              ;; If the non-whitespace chars match the first line,
              ;; just use it (this subsumes the 2 checks used previously).
              ;; Used when first line is `/* ...' and second-line is
              ;; ` * ...'.
              (let ((tmp second-line-prefix)
                    (re "\\`"))
                (while (string-match "\\`[ \t]*\\([^ \t]+\\)" tmp)
                  (setq re (concat re ".*" (regexp-quote (match-string 1 tmp))))
                  (setq tmp (substring tmp (match-end 0))))
                ;; (assert (string-match "\\`[ \t]*\\'" tmp))

                (if (string-match re first-line-prefix)
                    second-line-prefix

                  ;; Use the longest common substring of both prefixes,
                  ;; if there is one.
                  (fill-common-string-prefix first-line-prefix
                                             second-line-prefix)))))
	;; If we get a fill prefix from a one-line paragraph,
	;; maybe change it to whitespace,
	;; and check that it isn't a paragraph starter.
	(if first-line-prefix
	    (let ((result
		   ;; If first-line-prefix comes from the first line,
		   ;; see if it seems reasonable to use for all lines.
		   ;; If not, replace it with whitespace.
		   (if (or (and first-line-regexp
				(string-match first-line-regexp
					      first-line-prefix))
			   (and comment-start-skip
				(string-match comment-start-skip
					      first-line-prefix)))
		       first-line-prefix
		     (make-string (string-width first-line-prefix) ?\s))))
	      ;; But either way, reject it if it indicates the start
	      ;; of a paragraph when text follows it.
	      (if (not (eq 0 (string-match paragraph-start
					   (concat result "a"))))
		  result)))))))

(defun fill-single-word-nobreak-p ()
  "Don't break a line after the first or before the last word of a sentence."
  ;; Actually, allow breaking before the last word of a sentence, so long as
  ;; it's not the last word of the paragraph.
  (or (looking-at (concat "[ \t]*\\sw+" "\\(?:" (sentence-end) "\\)[ \t]*$"))
      (save-excursion
	(skip-chars-backward " \t")
	(and (/= (skip-syntax-backward "w") 0)
	     (/= (skip-chars-backward " \t") 0)
	     (/= (skip-chars-backward ".?!:") 0)
	     (looking-at (sentence-end))))))

(defun fill-french-nobreak-p ()
  "Return nil if French style allows breaking the line at point.
This is used in `fill-nobreak-predicate' to prevent breaking lines just
after an opening paren or just before a closing paren or a punctuation
mark such as `?' or `:'.  It is common in French writing to put a space
at such places, which would normally allow breaking the line at those
places."
  (or (looking-at "[ \t]*[])}»?!;:-]")
      (save-excursion
	(skip-chars-backward " \t")
	(unless (bolp)
	  (backward-char 1)
	  (or (looking-at "[([{«]")
	      ;; Don't cut right after a single-letter word.
	      (and (memq (preceding-char) '(?\t ?\s))
		   (eq (char-syntax (following-char)) ?w)))))))

(defun fill-polish-nobreak-p ()
  "Return nil if Polish style allows breaking the line at point.
This function may be used in the `fill-nobreak-predicate' hook.
It is almost the same as `fill-single-char-nobreak-p', with the
exception that it does not require the one-letter word to be
preceded by a space.  This blocks line-breaking in cases like
\"(a jednak)\"."
  (save-excursion
    (skip-chars-backward " \t")
    (backward-char 2)
    (looking-at "[^[:alpha:]]\\cl")))

(defun fill-single-char-nobreak-p ()
  "Return non-nil if a one-letter word is before point.
This function is suitable for adding to the hook `fill-nobreak-predicate',
to prevent the breaking of a line just after a one-letter word,
which is an error according to some typographical conventions."
  (save-excursion
    (skip-chars-backward " \t")
    (backward-char 2)
    (looking-at "[[:space:]][[:alpha:]]")))

(defun fill-nobreak-p ()
  "Return nil if breaking the line at point is allowed.
Can be customized with the variables `fill-nobreak-predicate'
and `fill-nobreak-invisible'."
  (or
   (and fill-nobreak-invisible (invisible-p (point)))
   (unless (bolp)
    (or
     ;; Don't break after a period followed by just one space.
     ;; Move back to the previous place to break.
     ;; The reason is that if a period ends up at the end of a
     ;; line, further fills will assume it ends a sentence.
     ;; If we now know it does not end a sentence, avoid putting
     ;; it at the end of the line.
     (and sentence-end-double-space
	  (save-excursion
	    (skip-chars-backward " ")
	    (and (eq (preceding-char) ?.)
                 ;; There's something more after the space.
		 (looking-at " [^ \n]"))))
     ;; Don't split a line if the rest would look like a new paragraph.
     (unless use-hard-newlines
       (save-excursion
	 (skip-chars-forward " \t")
	 ;; If this break point is at the end of the line,
	 ;; which can occur for auto-fill, don't consider the newline
	 ;; which follows as a reason to return t.
	 (and (not (eolp))
	      (looking-at paragraph-start))))
     (run-hook-with-args-until-success 'fill-nobreak-predicate)))))

(defun fill-find-break-point (limit)
  "Move point to a proper line breaking position of the current line.
Don't move back past the buffer position LIMIT.

This function is called when we are going to break the current line
after or before a non-ASCII character.  If the charset of the
character has the property `fill-find-break-point-function', this
function calls the property value as a function with one arg LIMIT.
If the charset has no such property, do nothing."
  (let ((func (or
	       (aref fill-find-break-point-function-table (following-char))
	       (aref fill-find-break-point-function-table (preceding-char)))))
    (if (and func (fboundp func))
	(funcall func limit))))

(defun fill-delete-prefix (from to prefix)
  "Delete the fill prefix from every line except the first.
The first line may not even have a fill prefix.
Point is moved to just past the fill prefix on the first line."
  (let ((fpre (if (and prefix (not (string-match "\\`[ \t]*\\'" prefix)))
		  (concat "[ \t]*\\("
			  (replace-regexp-in-string
			   "[ \t]+" "[ \t]*"
			   (regexp-quote prefix))
			  "\\)?[ \t]*")
		"[ \t]*")))
    (goto-char from)
    ;; Why signal an error here?  The problem needs to be caught elsewhere.
    ;; (if (>= (+ (current-left-margin) (length prefix))
    ;;         (current-fill-column))
    ;;     (error "fill-prefix too long for specified width"))
    (forward-line 1)
    (while (< (point) to)
      (if (looking-at fpre)
          (delete-region (point) (match-end 0)))
      (forward-line 1))
    (goto-char from)
    (if (looking-at fpre)
	(goto-char (match-end 0)))
    (point)))

(defun fill-delete-newlines (from to justify nosqueeze squeeze-after)
  (goto-char from)
  ;; Make sure sentences ending at end of line get an extra space.
  ;; loses on split abbrevs ("Mr.\nSmith")
  (let ((eol-double-space-re
	 (cond
	  ((not colon-double-space) (concat (sentence-end) "$"))
	  ;; Try to add the : inside the `sentence-end' regexp.
	  ((string-match "\\[[^][]*\\(\\.\\)[^][]*\\]" (sentence-end))
	   (concat (replace-match ".:" nil nil (sentence-end) 1) "$"))
	  ;; Can't find the right spot to insert the colon.
	  (t "[.?!:][])}\"']*$")))
	(sentence-end-without-space-list
	 (string-to-list sentence-end-without-space)))
    (while (re-search-forward eol-double-space-re to t)
      (or (>= (point) to) (memq (char-before) '(?\t ?\s))
	  (memq (char-after (match-beginning 0))
		sentence-end-without-space-list)
	  (insert-and-inherit ?\s))))

  (goto-char from)
  (if enable-multibyte-characters
      ;; Delete unnecessary newlines surrounded by words.  The
      ;; character category `|' means that we can break a line at the
      ;; character.  And, char-table
      ;; `fill-nospace-between-words-table' tells how to concatenate
      ;; words.  If a character has non-nil value in the table, never
      ;; put spaces between words, thus delete a newline between them.
      ;; Otherwise, delete a newline only when a character preceding a
      ;; newline has non-nil value in that table.
      (while (search-forward "\n" to t)
	(if (get-text-property (match-beginning 0) 'fill-space)
	    (replace-match (get-text-property (match-beginning 0) 'fill-space))
	  (let ((prev (char-before (match-beginning 0)))
		(next (following-char)))
	    (if (and (if fill-separate-heterogeneous-words-with-space
			 (and (aref (char-category-set next) ?|)
			      (aref (char-category-set prev) ?|))
		       (or (aref (char-category-set next) ?|)
			   (aref (char-category-set prev) ?|)))
		     (or (aref fill-nospace-between-words-table next)
			 (aref fill-nospace-between-words-table prev)))
		(delete-char -1))))))

  (goto-char from)
  (skip-chars-forward " \t")
  ;; Then change all newlines to spaces.
  (subst-char-in-region from to ?\n ?\s)
  (if (and nosqueeze (not (eq justify 'full)))
      nil
    (canonically-space-region (or squeeze-after (point)) to)
    ;; Remove trailing whitespace.
    ;; Maybe canonically-space-region should do that.
    (goto-char to) (delete-char (- (skip-chars-backward " \t"))))
  (goto-char from))

(defun fill-move-to-break-point (linebeg)
  "Move to the position where the line should be broken.
The break position will be always after LINEBEG and generally before point."
  ;; If the fill column is before linebeg, move to linebeg.
  (if (> linebeg (point)) (goto-char linebeg))
  ;; Move back to the point where we can break the line
  ;; at.  We break the line between word or after/before
  ;; the character which has character category `|'.  We
  ;; search space, \c| followed by a character, or \c|
  ;; following a character.  If not found, place
  ;; the point at linebeg.
  (while
      (when (re-search-backward "[ \t]\\|\\c|.\\|.\\c|" linebeg 0)
	;; In case of space, we place the point at next to
	;; the point where the break occurs actually,
	;; because we don't want to change the following
	;; logic of original Emacs.  In case of \c|, the
	;; point is at the place where the break occurs.
	(forward-char 1)
	(when (fill-nobreak-p) (skip-chars-backward " \t" linebeg))))

  ;; Move back over the single space between the words.
  (skip-chars-backward " \t")

  ;; If the left margin and fill prefix by themselves
  ;; pass the fill-column. or if they are zero
  ;; but we have no room for even one word,
  ;; keep at least one word or a character which has
  ;; category `|' anyway.
  (if (>= linebeg (point))
      ;; Ok, skip at least one word or one \c| character.
      ;; Meanwhile, don't stop at a period followed by one space.
      (let ((to (line-end-position))
	    (first t))
	(goto-char linebeg)
	(while (and (< (point) to) (or first (fill-nobreak-p)))
	  ;; Find a breakable point while ignoring the
	  ;; following spaces.
	  (skip-chars-forward " \t")
	  (if (looking-at "\\c|")
	      (forward-char 1)
	    (let ((pos (save-excursion
			 (skip-chars-forward "^ \n\t")
			 (point))))
	      (if (re-search-forward "\\c|" pos t)
		  (forward-char -1)
		(goto-char pos))))
	  (setq first nil)))

    (if enable-multibyte-characters
	;; If we are going to break the line after or
	;; before a non-ascii character, we may have to
	;; run a special function for the charset of the
	;; character to find the correct break point.
	(if (not (and (eq (charset-after (1- (point))) 'ascii)
		      (eq (charset-after (point)) 'ascii)))
	    ;; Make sure we take SOMETHING after the fill prefix if any.
	    (fill-find-break-point linebeg)))))

(defun fill-text-properties-at (pos)
  (let ((l (text-properties-at pos))
	prop-list)
    (while l
      (unless (eq (car l) 'composition)
	(setq prop-list
	      (cons (car l) (cons (cadr l) prop-list))))
      (setq l (cddr l)))
    prop-list))

(defun fill-newline ()
  ;; Replace whitespace here with one newline, then
  ;; indent to left margin.
  (skip-chars-backward " \t")
  (insert ?\n)
  ;; Give newline the properties of the space(s) it replaces
  (set-text-properties (1- (point)) (point)
		       (fill-text-properties-at (point)))
  (and (looking-at "\\( [ \t]*\\)\\(\\c|\\)?")
       (or (aref (char-category-set (or (char-before (1- (point))) ?\000)) ?|)
	   (match-end 2))
       ;; When refilling later on, this newline would normally not be replaced
       ;; by a space, so we need to mark it specially to re-install the space
       ;; when we unfill.
       (put-text-property (1- (point)) (point) 'fill-space (match-string 1)))
  ;; If we don't want breaks in invisible text, don't insert
  ;; an invisible newline.
  (if fill-nobreak-invisible
      (remove-text-properties (1- (point)) (point)
			      '(invisible t)))
  (if (or fill-prefix
	  (not fill-indent-according-to-mode))
      (fill-indent-to-left-margin)
    (indent-according-to-mode))
  ;; Insert the fill prefix after indentation.
  (and fill-prefix (not (equal fill-prefix ""))
       ;; Markers that were after the whitespace are now at point: insert
       ;; before them so they don't get stuck before the prefix.
       (insert-before-markers-and-inherit fill-prefix)))

(defun fill-indent-to-left-margin ()
  "Indent current line to the column given by `current-left-margin'."
  (let ((beg (point)))
    (indent-line-to (current-left-margin))
    (put-text-property beg (point) 'face 'default)))

(defun fill-region-as-paragraph-default (from to &optional justify
				              nosqueeze squeeze-after)
  "Fill the region as if it were a single paragraph.
This command removes any paragraph breaks in the region and
extra newlines at the end, and indents and fills lines between the
margins given by the `current-left-margin' and `current-fill-column'
functions.  (In most cases, the variable `fill-column' controls the
width.)  It leaves point at the beginning of the line following the
region.

Note that how paragraph breaks are removed in text that includes
characters from different scripts is affected by the value
of `fill-separate-heterogeneous-words-with-space', which see.

Normally, the command performs justification according to
the `current-justification' function, but with a prefix arg, it
does full justification instead.

When called from Lisp, optional third arg JUSTIFY can specify any
type of justification; see `default-justification' for the possible
values.
Optional fourth arg NOSQUEEZE non-nil means not to make spaces
between words canonical before filling.
Fifth arg SQUEEZE-AFTER, if non-nil, should be a buffer position; it
means canonicalize spaces only starting from that position.
See `canonically-space-region' for the meaning of canonicalization
of spaces.

Return the `fill-prefix' used for filling.

If `sentence-end-double-space' is non-nil, then period followed by one
space does not end a sentence, so don't break a line there."
  (interactive (progn
		 (barf-if-buffer-read-only)
		 (list (region-beginning) (region-end)
		       (if current-prefix-arg 'full))))
  (unless (memq justify '(t nil none full center left right))
    (setq justify 'full))

  ;; Make sure "to" is the endpoint.
  (goto-char (min from to))
  (setq to   (max from to))
  ;; Ignore blank lines at beginning of region.
  (skip-chars-forward " \t\n")

  (let ((from-plus-indent (point))
	(oneleft nil))

    (beginning-of-line)
    ;; We used to round up to whole line, but that prevents us from
    ;; correctly handling filling of mixed code-and-comment where we do want
    ;; to fill the comment but not the code.  So only use (point) if it's
    ;; further than `from', which means that `from' is followed by some
    ;; number of empty lines.
    (setq from (max (point) from))

    ;; Delete all but one soft newline at end of region.
    ;; And leave TO before that one.
    (goto-char to)
    (while (and (> (point) from) (eq ?\n (char-after (1- (point)))))
      (if (and oneleft
	       (not (and use-hard-newlines
			 (get-text-property (1- (point)) 'hard))))
	  (delete-char -1)
	(backward-char 1)
	(setq oneleft t)))
    (setq to (copy-marker (point) t))
    ;; ;; If there was no newline, and there is text in the paragraph, then
    ;; ;; create a newline.
    ;; (if (and (not oneleft) (> to from-plus-indent))
    ;; 	(newline))
    (goto-char from-plus-indent))

  (if (not (> to (point)))
      ;; There is no paragraph, only whitespace: exit now.
      (progn
        (set-marker to nil)
        nil)

    (or justify (setq justify (current-justification)))

    ;; Don't let Adaptive Fill mode alter the fill prefix permanently.
    (let ((fill-prefix fill-prefix))
      ;; Figure out how this paragraph is indented, if desired.
      (when (and adaptive-fill-mode
		 (or (null fill-prefix) (string= fill-prefix "")))
	(setq fill-prefix (fill-context-prefix from to))
	;; Ignore a white-space only fill-prefix
	;; if we indent-according-to-mode.
	(when (and fill-prefix fill-indent-according-to-mode
		   (string-match "\\`[ \t]*\\'" fill-prefix))
	  (setq fill-prefix nil)))

      (goto-char from)
      (beginning-of-line)

      (if (not justify)     ; filling disabled: just check indentation
	  (progn
	    (goto-char from)
	    (while (< (point) to)
	      (if (and (not (eolp))
		       (< (current-indentation) (current-left-margin)))
		  (fill-indent-to-left-margin))
	      (forward-line 1)))

	(if use-hard-newlines
	    (remove-list-of-text-properties from to '(hard)))
	;; Make sure first line is indented (at least) to left margin...
	(if (or (memq justify '(right center))
		(< (current-indentation) (current-left-margin)))
	    (fill-indent-to-left-margin))
	;; Delete the fill-prefix from every line.
	(fill-delete-prefix from to fill-prefix)
	(setq from (point))

	;; FROM, and point, are now before the text to fill,
	;; but after any fill prefix on the first line.

	(fill-delete-newlines from to justify nosqueeze squeeze-after)

	;; This is the actual filling loop.
	(goto-char from)
	(let (linebeg)
          (while (< (point) to)
	    (setq linebeg (point))
	    (move-to-column (current-fill-column))
	    (if (when (and (< (point) to) (< linebeg to))
		  ;; Find the position where we'll break the line.
		  ;; Use an immediately following space, if any.
		  ;; However, note that `move-to-column' may overshoot
		  ;; if there are wide characters (Bug#3234).
		  (unless (> (current-column) (current-fill-column))
		    (forward-char 1))
		  (fill-move-to-break-point linebeg)
		  ;; Check again to see if we got to the end of
		  ;; the paragraph.
		  (skip-chars-forward " \t")
		  (< (point) to))
		;; Found a place to cut.
		(progn
		  (fill-newline)
		  (when justify
		    ;; Justify the line just ended, if desired.
		    (save-excursion
		      (forward-line -1)
		      (justify-current-line justify nil t))))

	      (goto-char to)
	      ;; Justify this last line, if desired.
	      (if justify (justify-current-line justify t t))))))
      ;; Leave point after final newline.
      (goto-char to)
      (unless (eobp) (forward-char 1))
      (set-marker to nil)
      ;; Return the fill-prefix we used
      fill-prefix)))

(defun fill-region-as-paragraph (from to &optional justify
				      nosqueeze squeeze-after)
  "Fill the region as if it were a single paragraph.
The behavior of this command is controlled by the variable
`fill-region-as-paragraph-function', with the default implementation
being `fill-region-as-paragraph-default'.

The arguments FROM and TO define the boundaries of the region.

The optional third argument JUSTIFY, when called interactively with a
prefix arg, is assigned the value `full'.
When called from Lisp, JUSTIFY can specify any type of justification;
see `default-justification' for the possible values.
Optional fourth arg NOSQUEEZE non-nil means not to make spaces between
words canonical before filling.
Fifth arg SQUEEZE-AFTER, if non-nil, should be a buffer position; it
means canonicalize spaces only starting from that position.
See `canonically-space-region' for the meaning of canonicalization of
spaces.

It returns the `fill-prefix' used for filling."
  (interactive (progn
		 (barf-if-buffer-read-only)
		 (list (region-beginning) (region-end)
		       (if current-prefix-arg 'full))))
  (funcall fill-region-as-paragraph-function
           from to justify nosqueeze squeeze-after))

(defun skip-line-prefix (prefix)
  "If point is inside the string PREFIX at the beginning of line, move past it."
  (when (and prefix
	     (< (- (point) (line-beginning-position)) (length prefix))
	     (save-excursion
	       (beginning-of-line)
	       (looking-at (regexp-quote prefix))))
    (goto-char (match-end 0))))

(defun fill-minibuffer-function (arg)
  "Fill a paragraph in the minibuffer, ignoring the prompt."
  (save-restriction
    (narrow-to-region (minibuffer-prompt-end) (point-max))
    (fill-paragraph arg)))

(defun fill-forward-paragraph (arg)
  (funcall fill-forward-paragraph-function arg))

(defun fill-paragraph (&optional justify region)
  "Fill paragraph at or after point.

If JUSTIFY is non-nil (interactively, with prefix argument), justify as well.
If `sentence-end-double-space' is non-nil, then period followed by one
space does not end a sentence, so don't break a line there.
The variable `fill-column' controls the width for filling.

If `fill-paragraph-function' is non-nil, we call it (passing our
argument to it), and if it returns non-nil, we simply return its value.

If `fill-paragraph-function' is nil, return the `fill-prefix' used for filling.

The REGION argument is non-nil if called interactively; in that
case, if Transient Mark mode is enabled and the mark is active,
call `fill-region' to fill each of the paragraphs in the active
region, instead of just filling the current paragraph."
  (interactive (progn
		 (barf-if-buffer-read-only)
		 (list (if current-prefix-arg 'full) t)))
  (with-buffer-unmodified-if-unchanged
    (or
     ;; 1. Fill the region if it is active when called interactively.
     (and region transient-mark-mode mark-active
          (not (eq (region-beginning) (region-end)))
          (or (fill-region (region-beginning) (region-end) justify) t))
     ;; 2. Try fill-paragraph-function.
     (and (not (eq fill-paragraph-function t))
          (or fill-paragraph-function
              (and (minibufferp (current-buffer))
                   (= 1 (point-min))))
          (let ((function (or fill-paragraph-function
                              ;; In the minibuffer, don't count
                              ;; the width of the prompt.
                              'fill-minibuffer-function))
                ;; If fill-paragraph-function is set, it probably
                ;; takes care of comments and stuff.  If not, it
                ;; will have to set fill-paragraph-handle-comment
                ;; back to t explicitly or return nil.
                (fill-paragraph-handle-comment nil)
                (fill-paragraph-function t))
            (funcall function justify)))
     ;; 3. Try our syntax-aware filling code.
     (and fill-paragraph-handle-comment
          ;; Our code only handles \n-terminated comments right now.
          comment-start (equal comment-end "")
          (let ((fill-paragraph-handle-comment nil))
            (fill-comment-paragraph justify)))
     ;; 4. If it all fails, default to the good ol' text paragraph filling.
     (let ((before (point))
           (paragraph-start-orig paragraph-start)
           (paragraph-start paragraph-start)
           ;; Fill prefix used for filling the paragraph.
           fill-pfx)
       ;; Try to prevent code sections and comment sections from being
       ;; filled together.
       (when (and fill-paragraph-handle-comment comment-start-skip)
         (setq paragraph-start
               (concat paragraph-start "\\|[ \t]*\\(?:"
                       comment-start-skip "\\)")))
       (save-excursion
         ;; To make sure the return value of forward-paragraph is
         ;; meaningful, we have to start from the beginning of
         ;; line, otherwise skipping past the last few chars of a
         ;; paragraph-separator would count as a paragraph (and
         ;; not skipping any chars at EOB would not count as a
         ;; paragraph even if it is).
         (move-to-left-margin)
         (if (not (zerop (fill-forward-paragraph 1)))
             ;; There's no paragraph at or after point: give up.
             (setq fill-pfx "")
           (let ((end (point))
                 (beg (progn (fill-forward-paragraph -1) (point))))
             ;; If the paragraph starts with a comment line preceding point
             ;; on a non-comment line, skip such comment lines, so they
             ;; are not filled together (bug#80449).
             (when (and fill-paragraph-handle-comment comment-start-skip
                        (< beg before))
               (save-excursion
                 (goto-char beg)
                 (when (looking-at paragraph-start-orig)
                   (goto-char (1+ (match-end 0))))
                 (when (looking-at comment-start-skip)
                   (forward-line 1)
                   (setq beg (point)))))
             (goto-char before)
             (setq fill-pfx
                   (if use-hard-newlines
                       ;; Can't use fill-region-as-paragraph, since this
                       ;; paragraph may still contain hard newlines.  See
                       ;; fill-region.
                       (fill-region beg end justify)
                     (fill-region-as-paragraph beg end justify))))))
       fill-pfx))))

(defun unfill-paragraph (arg &optional beg end)
  "Join lines of this paragraph and fix up whitespace at joins.
Interactively, if the region is active, join lines of each paragraph in
the region.  A numeric prefix argument means join the lines of the
following ARG paragraphs.  In this case an active region is ignored.

With an active region and no prefix argument this is roughly the same as
`delete-indentation' with that active region, except that this command
only joins lines within paragraphs, preserving the paragraphs
themselves.

When called from Lisp, ARG is the number of following paragraphs to join
lines within, or if ARG is nil, optional arguments BEG and END non-nil
means to join the lines of each paragraph in the region delimited by BEG
and END."
  (interactive "P\nR")
  (when (or arg (not beg))
    (let ((arg (prefix-numeric-value arg)))
      (when (zerop arg)
        (user-error "Invalid numeric argument to `unfill-paragraph'"))
      (save-excursion
        (fill-forward-paragraph 1)
        (fill-forward-paragraph -1)
        (setq beg (point))
        (fill-forward-paragraph arg)
        (setq end (point)))))
  ;; FIXME: It would be better to use
  ;;
  ;;    (let ((fill-column (* (max 2 tab-width) (point-max))))
  ;;      (fill-region beg end))
  ;;
  ;; multiplying by at least 2 to account for any wide characters in the
  ;; region to be filled and by at least `tab-width' to account for any
  ;; tab characters in the region to be filled.  Then we can easily
  ;; prove that filling the region will actually unfill it.
  ;; However, `fill-region' fails if `fill-column' is not a fixnum.
  (let ((fill-column most-positive-fixnum))
    (fill-region beg end)))

(defun fill-comment-paragraph (&optional justify)
  "Fill current comment.
If we're not in a comment, just return nil so that the caller
can take care of filling.  JUSTIFY is used as in `fill-paragraph'."
  (comment-normalize-vars)
  (let (has-code-and-comment ; Non-nil if it contains code and a comment.
	comin comstart)
    ;; Figure out what kind of comment we are looking at.
    (save-excursion
      (beginning-of-line)
      (when (setq comstart (comment-search-forward (line-end-position) t))
	(setq comin (point))
	(goto-char comstart) (skip-chars-backward " \t")
	(setq has-code-and-comment (not (bolp)))))

    (if (not (and comstart
                  ;; Make sure the comment-start mark we found is accepted by
                  ;; comment-start-skip.  If not, all bets are off, and
                  ;; we'd better not mess with it.
                  (string-match comment-start-skip
                                (buffer-substring comstart comin))))

	;; Return nil, so the normal filling will take place.
	nil

      ;; Narrow to include only the comment, and then fill the region.
      (let* ((fill-prefix fill-prefix)
	     (commark
	      (comment-string-strip (buffer-substring comstart comin) nil t))
	     (comment-re
              ;; A regexp more specialized than comment-start-skip, that only
              ;; matches the current commark rather than any valid commark.
              ;;
              ;; The specialized regexp only works for "normal" comment
              ;; syntax, not for Texinfo's "@c" (which can't be immediately
              ;; followed by word-chars) or Fortran's "C" (which needs to be
              ;; at bol), so check that comment-start-skip indeed allows the
              ;; commark to appear in the middle of the line and followed by
              ;; word chars.  The choice of "\0" and "a" is mostly arbitrary.
              (if (string-match comment-start-skip (concat "\0" commark "a"))
                  (concat "[ \t]*" (regexp-quote commark)
                          ;; Make sure we only match comments that
                          ;; use the exact same comment marker.
                          "[^" (substring commark -1) "]")
                (concat "[ \t]*\\(?:" comment-start-skip "\\)")))
             (comment-fill-prefix	; Compute a fill prefix.
	      (save-excursion
		(goto-char comstart)
		(if has-code-and-comment
		    (concat
		     (if (not indent-tabs-mode)
			 (make-string (current-column) ?\s)
		       (concat
			(make-string (/ (current-column) tab-width) ?\t)
			(make-string (% (current-column) tab-width) ?\s)))
		     (buffer-substring (point) comin))
		  (buffer-substring (line-beginning-position) comin))))
	     beg end)
	(save-excursion
	  (save-restriction
	    (beginning-of-line)
	    (narrow-to-region
	     ;; Find the first line we should include in the region to fill.
	     (if has-code-and-comment
		 (line-beginning-position)
	       (save-excursion
		 (while (and (zerop (forward-line -1))
			     (looking-at comment-re)))
		 ;; We may have gone too far.  Go forward again.
		 (line-beginning-position
		  (if (progn
			(goto-char
			 (or (comment-search-forward (line-end-position) t)
			     (point)))
			(looking-at comment-re))
		      (progn (setq comstart (point)) 1)
		    (progn (setq comstart (point)) 2)))))
	     ;; Find the beginning of the first line past the region to fill.
	     (save-excursion
	       (while (progn (forward-line 1)
			     (looking-at comment-re)))
	       (point)))
	    ;; Obey paragraph starters and boundaries within comments.
	    (let* ((paragraph-separate
		    ;; Use the default values since they correspond to
		    ;; the values to use for plain text.
		    (concat paragraph-separate "\\|[ \t]*\\(?:"
			    comment-start-skip "\\)\\(?:"
			    (default-value 'paragraph-separate) "\\)"))
		   (paragraph-start
		    (concat paragraph-start "\\|[ \t]*\\(?:"
			    comment-start-skip "\\)\\(?:"
			    (default-value 'paragraph-start) "\\)"))
		   ;; We used to rely on fill-prefix to break paragraph at
		   ;; comment-starter changes, but it did not work for the
		   ;; first line (mixed comment&code).
		   ;; We now use comment-re instead to "manually" make sure
		   ;; we treat comment-marker changes as paragraph boundaries.
		   ;; (paragraph-ignore-fill-prefix nil)
		   ;; (fill-prefix comment-fill-prefix)
		   (after-line (if has-code-and-comment
				   (line-beginning-position 2))))
	      (setq end (progn (forward-paragraph) (point)))
	      ;; If this comment starts on a line with code,
	      ;; include that line in the filling.
	      (setq beg (progn (backward-paragraph)
			       (if (eq (point) after-line)
				   (forward-line -1))
			       (point)))))

	  ;; Find the fill-prefix to use.
	  (cond
	   (fill-prefix)	  ; Use the user-provided fill prefix.
	   ((and adaptive-fill-mode	; Try adaptive fill mode.
		 (setq fill-prefix (fill-context-prefix beg end))
		 (string-match comment-start-skip fill-prefix)))
	   (t
	    (setq fill-prefix comment-fill-prefix)))

	  ;; Don't fill with narrowing.
	  (or
	   (fill-region-as-paragraph
	    (max comstart beg) end justify nil
	    ;; Don't canonicalize spaces within the code just before
	    ;; the comment.
	    (save-excursion
	      (goto-char beg)
	      (if (looking-at fill-prefix)
		  nil
		(re-search-forward comment-start-skip))))
	   ;; Make sure we don't return nil.
	   t))))))

(defun fill-region (from to &optional justify nosqueeze to-eop)
  "Fill each of the paragraphs in the region.
A prefix arg means justify as well.
The `fill-column' variable controls the width.

Noninteractively, the third argument JUSTIFY specifies which
kind of justification to do: `full', `left', `right', `center',
or `none' (equivalent to nil).  A value of t means handle each
paragraph as specified by its text properties.

The fourth arg NOSQUEEZE non-nil means to leave whitespace other
than line breaks untouched, and fifth arg TO-EOP non-nil means
to keep filling to the end of the paragraph (or next hard newline,
if variable `use-hard-newlines' is on).

Return the `fill-prefix' used for filling the last paragraph.

If `sentence-end-double-space' is non-nil, then period followed by one
space does not end a sentence, so don't break a line there.

The variable `fill-region-as-paragraph-function' can be used to override
how paragraphs are filled."
  (interactive (progn
		 (barf-if-buffer-read-only)
		 (list (region-beginning) (region-end)
		       (if current-prefix-arg 'full))))
  (unless (memq justify '(t nil none full center left right))
    (setq justify 'full))
  (let ((start-point (point-marker))
	max beg fill-pfx)
    (goto-char (max from to))
    (when to-eop
      (skip-chars-backward "\n")
      (fill-forward-paragraph 1))
    (setq max (copy-marker (point) t))
    (goto-char (setq beg (min from to)))
    (beginning-of-line)
    (while (< (point) max)
      (let ((initial (point))
	    end)
	;; If using hard newlines, break at every one for filling
	;; purposes rather than using paragraph breaks.
	(if use-hard-newlines
	    (progn
	      (while (and (setq end (text-property-any (point) max
						       'hard t))
			  (not (= ?\n (char-after end)))
			  (not (>= end max)))
		(goto-char (1+ end)))
	      (setq end (if end (min max (1+ end)) max))
	      (goto-char initial))
	  (fill-forward-paragraph 1)
	  (setq end (min max (point)))
	  (fill-forward-paragraph -1))
	(if (< (point) beg)
	    (goto-char beg))
	(if (and (>= (point) initial) (< (point) end))
	    (setq fill-pfx
		  (fill-region-as-paragraph (point) end justify nosqueeze))
	  (goto-char end))))
    (goto-char start-point)
    (set-marker start-point nil)
    fill-pfx))

(defun current-justification ()
  "How should we justify this line?
This returns the value of the text-property `justification',
or the variable `default-justification' if there is no text-property.
However, it returns nil rather than `none' to mean \"don't justify\"."
  (let ((j (or (get-text-property
		;; Make sure we're looking at paragraph body.
		(save-excursion (skip-chars-forward " \t")
				(if (and (eobp) (not (bobp)))
				    (1- (point)) (point)))
		'justification)
	       default-justification)))
    (if (eq 'none j)
	nil
      j)))

(defun set-justification (begin end style &optional whole-par)
  "Set the region's justification style to STYLE.
This commands prompts for the kind of justification to use.
See `default-justification' for the possible values and their meaning.
If the mark is not active, this command operates on the current paragraph.
If the mark is active, it operates on the region.  However, if the
beginning and end of the region are not at paragraph breaks, they are
moved to the beginning and end \(respectively) of the paragraphs they
are in.

If variable `use-hard-newlines' is true, all hard newlines are
taken to be paragraph breaks.

When calling from a program, operates just on region between BEGIN and END,
unless optional fourth arg WHOLE-PAR is non-nil.  In that case bounds are
extended to include entire paragraphs as in the interactive command."
  (interactive (list (if mark-active (region-beginning) (point))
		     (if mark-active (region-end) (point))
		     (let ((s (completing-read
			       "Set justification to: "
			       '(("left") ("right") ("full")
				 ("center") ("none"))
			       nil t)))
		       (if (equal s "") (error ""))
		       (intern s))
		     t))
  (save-excursion
    (save-restriction
      (if whole-par
	  (let ((paragraph-start (if use-hard-newlines "." paragraph-start))
		(paragraph-ignore-fill-prefix (if use-hard-newlines t
						paragraph-ignore-fill-prefix)))
	    (goto-char begin)
	    (while (and (bolp) (not (eobp))) (forward-char 1))
	    (backward-paragraph)
	    (setq begin (point))
	    (goto-char end)
	    (skip-chars-backward " \t\n" begin)
	    (forward-paragraph)
	    (setq end (point))))

      (narrow-to-region (point-min) end)
      (unjustify-region begin (point-max))
      (put-text-property begin (point-max) 'justification style)
      (fill-region begin (point-max) nil t))))

(defun set-justification-none (b e)
  "Disable automatic filling for paragraphs in the region.
If the mark is not active, this applies to the current paragraph."
  (interactive (list (if mark-active (region-beginning) (point))
		     (if mark-active (region-end) (point))))
  (set-justification b e 'none t))

(defun set-justification-left (b e)
  "Make paragraphs in the region left-justified.
This means lines are flush (lined up) at the left margin and ragged
on the right.
This is usually the default, but see the variable `default-justification'.
If the mark is not active, this applies to the current paragraph."
  (interactive (list (if mark-active (region-beginning) (point))
		     (if mark-active (region-end) (point))))
  (set-justification b e 'left t))

(defun set-justification-right (b e)
  "Make paragraphs in the region right-justified.
This means lines are flush (lined up) at the right margin and ragged
on the left.
If the mark is not active, this applies to the current paragraph."
  (interactive (list (if mark-active (region-beginning) (point))
		     (if mark-active (region-end) (point))))
  (set-justification b e 'right t))

(defun set-justification-full (b e)
  "Make paragraphs in the region fully justified.
This makes lines be lined up on both margins by inserting spaces between words.
If the mark is not active, this applies to the current paragraph."
  (interactive (list (if mark-active (region-beginning) (point))
		     (if mark-active (region-end) (point))))
  (set-justification b e 'full t))

(defun set-justification-center (b e)
  "Make paragraphs in the region centered.
If the mark is not active, this applies to the current paragraph."
  (interactive (list (if mark-active (region-beginning) (point))
		     (if mark-active (region-end) (point))))
  (set-justification b e 'center t))

(defun justify-current-line (&optional how eop nosqueeze)
  "Do some kind of justification on this line.
Normally does full justification: adds spaces to the line to make it end at
the column given by `current-fill-column'.
Optional first argument HOW specifies alternate type of justification:
it can be `left', `right', `full', `center', or `none'; for their
meaning, see `default-justification'.
If HOW is t, will justify however the `current-justification' function says to.
If HOW is nil or missing, full justification is done by default.
Second arg EOP non-nil means that this is the last line of the paragraph, so
it will not be stretched by full justification.
Third arg NOSQUEEZE non-nil means to leave interior whitespace unchanged,
otherwise it is made canonical."
  (interactive "*")
  (if (eq t how) (setq how (or (current-justification) 'none))
    (if (null how) (setq how 'full)
      (or (memq how '(none left right center))
	  (setq how 'full))))
  (or (memq how '(none left))  ; No action required for these.
      (let ((fc (current-fill-column))
	    (pos (point-marker))
	    fp-end			; point at end of fill prefix
	    beg				; point at beginning of line's text
	    end				; point at end of line's text
	    indent			; column of `beg'
	    endcol			; column of `end'
	    ncols			; new indent point or offset
	    (nspaces 0)			; number of spaces between words
					; in line (not space characters)
	    (curr-fracspace 0)		; current fractional space amount
	    count)
	(end-of-line)
	;; Check if this is the last line of the paragraph.
	(if (and use-hard-newlines (null eop)
		 (get-text-property (point) 'hard))
	    (setq eop t))
	(skip-chars-backward " \t")
	;; Quick exit if it appears to be properly justified already
	;; or there is no text.
	(if (or (bolp)
		(and (memq how '(full right))
		     (= (current-column) fc)))
	    nil
	  (setq end (point))
	  (beginning-of-line)
	  (skip-chars-forward " \t")
	  ;; Skip over fill-prefix.
	  (if (and fill-prefix
		   (not (string-equal fill-prefix ""))
		   (equal fill-prefix
			  (buffer-substring
			   (point) (min (point-max) (+ (length fill-prefix)
						       (point))))))
	      (forward-char (length fill-prefix))
	    (if (and adaptive-fill-mode
		     (looking-at adaptive-fill-regexp))
		(goto-char (match-end 0))))
	  (setq fp-end (point))
	  (skip-chars-forward " \t")
	  ;; This is beginning of the line's text.
	  (setq indent (current-column))
	  (setq beg (point))
	  (goto-char end)
	  (setq endcol (current-column))

	  ;; HOW can't be null or left--we would have exited already
	  (cond ((eq 'right how)
		 (setq ncols (- fc endcol))
		 (if (< ncols 0)
		     ;; Need to remove some indentation
		     (delete-region
		      (progn (goto-char fp-end)
			     (if (< (current-column) (+ indent ncols))
				 (move-to-column (+ indent ncols) t))
			     (point))
		      (progn (move-to-column indent) (point)))
		   ;; Need to add some
		   (goto-char beg)
		   (indent-to (+ indent ncols))
		   ;; If point was at beginning of text, keep it there.
		   (if (= beg pos)
		       (move-marker pos (point)))))

		((eq 'center how)
		 ;; Figure out how much indentation is needed
		 (setq ncols (+ (current-left-margin)
				(/ (- fc (current-left-margin) ;avail. space
				      (- endcol indent)) ;text width
				   2)))
		 (if (< ncols indent)
		     ;; Have too much indentation - remove some
		     (delete-region
		      (progn (goto-char fp-end)
			     (if (< (current-column) ncols)
				 (move-to-column ncols t))
			     (point))
		      (progn (move-to-column indent) (point)))
		   ;; Have too little - add some
		   (goto-char beg)
		   (indent-to ncols)
		   ;; If point was at beginning of text, keep it there.
		   (if (= beg pos)
		       (move-marker pos (point)))))

		((eq 'full how)
		 ;; Insert extra spaces between words to justify line
		 (save-restriction
		   (narrow-to-region beg end)
		   (or nosqueeze
		       (canonically-space-region beg end))
		   (goto-char (point-max))
		   ;; count word spaces in line
		   (while (search-backward " " nil t)
		     (setq nspaces (1+ nspaces))
		     (skip-chars-backward " "))
		   (setq ncols (- fc endcol))
		   ;; Ncols is number of additional space chars needed
		   (when (and (> ncols 0) (> nspaces 0) (not eop))
                     (setq curr-fracspace (+ ncols (/ nspaces 2))
                           count nspaces)
                     (while (> count 0)
                       (skip-chars-forward " ")
                       (insert-char ?\s (/ curr-fracspace nspaces) t)
                       (search-forward " " nil t)
                       (setq count (1- count)
                             curr-fracspace
                             (+ (% curr-fracspace nspaces) ncols))))))
		(t (error "Unknown justification value"))))
	(goto-char pos)
	(move-marker pos nil)))
  nil)

(defun unjustify-current-line ()
  "Remove justification whitespace from current line.
If the line is centered or right-justified, this function removes any
indentation past the left margin.  If the line is full-justified, it removes
extra spaces between words.  It does nothing in other justification modes."
  (let ((justify (current-justification)))
    (cond ((eq 'left justify) nil)
	  ((eq  nil  justify) nil)
	  ((eq 'full justify)		; full justify: remove extra spaces
	   (beginning-of-line-text)
	   (canonically-space-region (point) (line-end-position)))
	  ((memq justify '(center right))
	   (save-excursion
	     (move-to-left-margin nil t)
	     ;; Position ourselves after any fill-prefix.
	     (if (and fill-prefix
		      (not (string-equal fill-prefix ""))
		      (equal fill-prefix
			     (buffer-substring
			      (point) (min (point-max) (+ (length fill-prefix)
							  (point))))))
		 (forward-char (length fill-prefix)))
	     (delete-region (point) (progn (skip-chars-forward " \t")
					   (point))))))))

(defun unjustify-region (&optional begin end)
  "Remove justification whitespace from region.
For centered or right-justified regions, this function removes any indentation
past the left margin from each line.  For full-justified lines, it removes
extra spaces between words.  It does nothing in other justification modes.
Arguments BEGIN and END are optional; default is the whole buffer."
  (save-excursion
    (save-restriction
      (if end (narrow-to-region (point-min) end))
      (goto-char (or begin (point-min)))
      (while (not (eobp))
	(unjustify-current-line)
	(forward-line 1)))))

(defun fill-nonuniform-paragraphs (min max &optional justifyp citation-regexp)
  "Fill paragraphs within the region, allowing varying indentation within each.
This command divides the region into \"paragraphs\",
only at paragraph-separator lines, then fills each paragraph
using as the fill prefix the smallest indentation of any line
in the paragraph.

When calling from a program, pass range to fill as first two arguments.

Optional third and fourth arguments JUSTIFYP and CITATION-REGEXP:
JUSTIFYP to justify paragraphs (prefix arg).
When filling a mail message, pass a regexp for CITATION-REGEXP
which will match the prefix of a line which is a citation marker
plus whitespace, but no other kind of prefix.
Also, if CITATION-REGEXP is non-nil, don't fill header lines."
  (interactive (progn
		 (barf-if-buffer-read-only)
		 (list (region-beginning) (region-end)
		       (if current-prefix-arg 'full))))
  (let ((fill-individual-varying-indent t))
    (fill-individual-paragraphs min max justifyp citation-regexp)))

(defun fill-individual-paragraphs (min max &optional justify citation-regexp)
  "Fill paragraphs of uniform indentation within the region.
This command divides the region into \"paragraphs\",
treating every change in indentation level or prefix as a paragraph boundary,
then fills each paragraph using its indentation level as the fill prefix.

There is one special case where a change in indentation does not start
a new paragraph.  This is for text of this form:

   foo>    This line with extra indentation starts
   foo> a paragraph that continues on more lines.

These lines are filled together.

When calling from a program, pass the range to fill
as the first two arguments.

Optional third and fourth arguments JUSTIFY and CITATION-REGEXP:
JUSTIFY to justify paragraphs (prefix arg).
When filling a mail message, pass a regexp for CITATION-REGEXP
which will match the prefix of a line which is a citation marker
plus whitespace, but no other kind of prefix.
Also, if CITATION-REGEXP is non-nil, don't fill header lines."
  (interactive (progn
		 (barf-if-buffer-read-only)
		 (list (region-beginning) (region-end)
		       (if current-prefix-arg 'full))))
  (save-restriction
    (save-excursion
      (goto-char min)
      (beginning-of-line)
      (narrow-to-region (point) max)
      (if citation-regexp
	  (while (and (not (eobp))
		      (or (looking-at "[ \t]*[^ \t\n]+:")
			  (looking-at "[ \t]*$")))
	    (if (looking-at "[ \t]*[^ \t\n]+:")
		(search-forward "\n\n" nil 'move)
	      (forward-line 1))))
      (narrow-to-region (point) max)
      ;; Loop over paragraphs.
      (while (progn
	       ;; Skip over all paragraph-separating lines
	       ;; so as to not include them in any paragraph.
               (while (and (not (eobp))
			   (progn (move-to-left-margin)
				  (and (not (eobp))
				       (looking-at paragraph-separate))))
                 (forward-line 1))
               (skip-chars-forward " \t\n") (not (eobp)))
	(move-to-left-margin)
	(let ((start (point))
	      fill-prefix fill-prefix-regexp)
	  ;; Find end of paragraph, and compute the smallest fill-prefix
	  ;; that fits all the lines in this paragraph.
	  (while (progn
		   ;; Update the fill-prefix on the first line
		   ;; and whenever the prefix good so far is too long.
		   (if (not (and fill-prefix
				 (looking-at fill-prefix-regexp)))
		       (setq fill-prefix
			     (fill-individual-paragraphs-prefix
			      citation-regexp)
			     fill-prefix-regexp (regexp-quote fill-prefix)))
		   (forward-line 1)
		   (if (bolp)
		       ;; If forward-line went past a newline,
		       ;; move further to the left margin.
		       (move-to-left-margin))
		   ;; Now stop the loop if end of paragraph.
		   (and (not (eobp))
			(if fill-individual-varying-indent
			    ;; If this line is a separator line, with or
			    ;; without prefix, end the paragraph.
			    (and
			     (not (looking-at paragraph-separate))
			     (save-excursion
			       (not (and (looking-at fill-prefix-regexp)
					 (progn (forward-char
						 (length fill-prefix))
						(looking-at
						 paragraph-separate))))))
			  ;; If this line has more or less indent
			  ;; than the fill prefix wants, end the paragraph.
			  (and (looking-at fill-prefix-regexp)
			       ;; If fill prefix is shorter than a new
			       ;; fill prefix computed here, end paragraph.
 			       (let ((this-line-fill-prefix
				      (fill-individual-paragraphs-prefix
				       citation-regexp)))
 				 (>= (length fill-prefix)
 				     (length this-line-fill-prefix)))
			       (save-excursion
				 (not (progn (forward-char
					      (length fill-prefix))
					     (or (looking-at "[ \t]")
						 (looking-at paragraph-separate)
						 (looking-at paragraph-start)))))
			       (not (and (equal fill-prefix "")
					 citation-regexp
					 (looking-at citation-regexp))))))))
	  ;; Fill this paragraph, but don't add a newline at the end.
	  (let ((had-newline (bolp)))
	    (fill-region-as-paragraph start (point) justify)
	    (if (and (bolp) (not had-newline))
		(delete-char -1))))))))

(defun fill-individual-paragraphs-prefix (citation-regexp)
  (let* ((adaptive-fill-first-line-regexp ".*")
	 (just-one-line-prefix
	  ;; Accept any prefix rather than just the ones matched by
	  ;; adaptive-fill-first-line-regexp.
	  (fill-context-prefix (point) (line-beginning-position 2)))
	 (two-lines-prefix
	  (fill-context-prefix (point) (line-beginning-position 3))))
    (if (not just-one-line-prefix)
	(buffer-substring
	 (point) (save-excursion (skip-chars-forward " \t") (point)))
	;; See if the citation part of JUST-ONE-LINE-PREFIX
	;; is the same as that of TWO-LINES-PREFIX,
	;; except perhaps with longer whitespace.
      (if (and just-one-line-prefix two-lines-prefix
	       (let* ((one-line-citation-part
		       (fill-individual-paragraphs-citation
			just-one-line-prefix citation-regexp))
		      (two-lines-citation-part
		       (fill-individual-paragraphs-citation
			two-lines-prefix citation-regexp))
		      (adjusted-two-lines-citation-part
		       (substring two-lines-citation-part 0
				  (string-match "[ \t]*\\'"
						two-lines-citation-part))))
		 (and
		 (string-match (concat "\\`"
				       (regexp-quote
					adjusted-two-lines-citation-part)
				       "[ \t]*\\'")
			       one-line-citation-part)
		 (>= (string-width one-line-citation-part)
		      (string-width two-lines-citation-part)))))
	    two-lines-prefix
	just-one-line-prefix))))

(defun fill-individual-paragraphs-citation (string citation-regexp)
  (if citation-regexp
      (if (string-match citation-regexp string)
	  (match-string 0 string)
	"")
    string))

(defun fill-region-as-paragraph-semlf (from to &optional justify
                                            nosqueeze squeeze-after)
  "Fill the region using semantic linefeeds as if it were a single paragraph.
This command removes any paragraph breaks in the region and extra
newlines at the end, and fills lines within the region.  Text is
refilled putting a newline character after each sentence, calling
`forward-sentence' to find the ends of sentences.  If
`sentence-end-double-space' is non-nil, period followed by one space is
not the end of a sentence.

If JUSTIFY is non-nil (interactively, with prefix argument), justify as
well.  If NOSQUEEZE is non-nil, do not to make spaces between words
canonical before filling.  SQUEEZE-AFTER, if non-nil, should be a buffer
position; it means canonicalize spaces only starting from that position.
See `canonically-space-region' for the meaning of canonicalization of
spaces.  The variable `fill-column' controls the width for filling.

Return the `fill-prefix' used for filling.

This function can be assigned to `fill-region-as-paragraph-function' to
override how functions like `fill-paragraph' and `fill-region' fill
text.

For more details about semantic linefeeds, see URL `https://sembr.org/'
and URL `https://rhodesmill.org/brandon/2012/one-sentence-per-line/'."
  (interactive (progn
		 (barf-if-buffer-read-only)
		 (list (region-beginning)
                       (region-end)
		       (if current-prefix-arg 'full))))

  (let ((from (min from to))
        (to (copy-marker (max from to) t))
        pfx)
    (goto-char from)
    (let ((fill-column most-positive-fixnum))
      (setq pfx (or (save-excursion
                      (fill-region-as-paragraph-default (point)
                                                        to
                                                        nil
                                                        nosqueeze
                                                        squeeze-after))
                    "")))
    (while (< (point) to)
      (let ((fill-to (copy-marker
                      (min to
                           (save-excursion
                             (forward-sentence)
                             (point)))
                      t))
            (fill-prefix pfx))
	(fill-region-as-paragraph-default (point)
				          fill-to
				          justify
                                          t)
        (goto-char fill-to))
      (when (and (> (point) (line-beginning-position))
		 (< (point) (line-end-position))
                 (< (point) to))
	(delete-horizontal-space)
	(insert "\n")
	(insert pfx)))
    pfx))

(defvar comment-start-skip nil
  "Regexp to match the start of a comment plus everything up to its body.")

(defvar mail-citation-prefix nil)

(defvar fill-individual-varying-indent nil)
(defvar colon-double-space nil)
(defvar fill-separate-heterogeneous-words-with-space nil)
(defvar fill-paragraph-function nil)
(defvar fill-paragraph-handle-comment t)
(defvar enable-kinsoku t)
(defvar fill-indent-according-to-mode nil)
(defvar current-fill-column--has-warned nil)
(defvar fill-nobreak-predicate nil)
(defvar fill-nobreak-invisible nil)
(defvar fill-find-break-point-function-table (make-char-table nil))
(defvar fill-nospace-between-words-table (make-char-table nil))
(defvar fill-region-as-paragraph-function #'fill-region-as-paragraph-default)
(defvar fill-forward-paragraph-function 'forward-paragraph)


;; GNU keymap shape: (keymap E1 ... En . PARENT) — the parent is the
;; improper tail of the cdr spine, so its elements print merged flat
;; after the map's own bindings.

(defvar prog-mode-map
  (list 'keymap
        (cons 27 (list 'keymap
                       (cons 113 'prog-fill-reindent-defun)
                       (cons 17 'prog-indent-sexp))))
  "Keymap for Prog mode.")

(defvar lisp-mode-shared-map
  (let ((m (list 'keymap
                 (cons 127 'backward-delete-char-untabify)
                 (cons 27 (list 'keymap (cons 17 'indent-sexp))))))
    (set-keymap-parent m prog-mode-map)
    m)
  "Keymap shared between Lisp mode variants.")

(defvar lisp-interaction-mode-map
  (let* ((inner
          (list 'keymap "Lisp-Interaction"
                (list 'Complete\ Lisp\ Symbol 'menu-item "Complete Lisp Symbol"
                      'completion-at-point
                      :help "Perform completion on Lisp symbol preceding point")
                (list 'Indent\ or\ Pretty-Print 'menu-item "Indent or Pretty-Print"
                      'indent-pp-sexp
                      :help "Indent each line of the list starting just after point, or prettyprint it")
                (list 'Instrument\ Function\ for\ Debugging
                      'menu-item "Instrument Function for Debugging"
                      'edebug-defun :keys "C-u C-M-x"
                      :help "Evaluate the top level form point is in, stepping through with Edebug")
                (list 'Evaluate\ and\ Print 'menu-item "Evaluate and Print"
                      'eval-print-last-sexp
                      :help "Evaluate sexp before point; print value into current buffer")
                (list 'Evaluate\ Defun 'menu-item "Evaluate Defun"
                      'eval-defun
                      :help "Evaluate the top-level form containing point, or after point")))
         (m (list 'keymap
                  (cons 'menu-bar
                        (list 'keymap
                              (list 'lisp-interaction 'menu-item
                                    "Lisp-Interaction" inner)))
                  (cons 10 'eval-print-last-sexp)
                  (cons 3 (list 'keymap
                                (cons 2 'elisp-byte-compile-buffer)
                                (cons 5 'elisp-eval-region-or-buffer)))
                  (cons 27 (list 'keymap
                                 (cons 9 'completion-at-point)
                                 (cons 17 'indent-pp-sexp)
                                 (cons 24 'eval-defun))))))
    (set-keymap-parent m lisp-mode-shared-map)
    m)
  "Keymap for Lisp Interaction mode.")

(defvar emacs-lisp-mode-map
  (let ((m (list 'keymap
                 (cons 'menu-bar
                       (list 'keymap
                             (list 'emacs-lisp 'menu-item "Emacs-Lisp"
                                   (list 'keymap "Emacs-Lisp"))))
                 (cons 10 'eval-print-last-sexp)
                 (cons 3 (list 'keymap
                               (cons 2 'elisp-byte-compile-buffer)
                               (cons 6 'elisp-byte-compile-file)
                               (cons 5 'elisp-eval-region-or-buffer)))
                 (cons 27 (list 'keymap
                                (cons 17 'indent-pp-sexp)
                                (cons 24 'eval-defun)
                                (cons 9 'completion-at-point))))))
    (set-keymap-parent m lisp-mode-shared-map)
    m)
  "Keymap for Emacs Lisp mode.")

(define-derived-mode minibuffer-inactive-mode fundamental-mode
  "InactiveMinibuffer"
  "Major mode for the minibuffer when it is inactive.
This is only used when the minibuffer area has no active minibuffer.")

;; ---------- Lisp indentation (lisp-mode.el subset) ----------

(defvar lisp-indent-offset nil
  "If non-nil, indent Lisp code by this many columns.")
(defvar lisp-body-indent 2
  "Number of columns to indent the second line of a `(def...)' form.")
(defvar-local lisp-indent-local-overrides nil
  "Alist of per-buffer indent overrides.")
;; `lisp-indent-function' (the variable) already defaults to the
;; function of the same name; declared by the C side.
(defvar-local lisp-indent-function 'lisp-indent-function)
(defvar calculate-lisp-indent-last-sexp nil
  "Dynamically bound during `calculate-lisp-indent'.")

(defun lisp-ppss (&optional pos)
  "Return the parse-partial-sexp state at POS (or point)."
  (syntax-ppss pos))

(defun calculate-lisp-indent (&optional parse-start)
  "Return appropriate indentation for current line as Lisp code.
Faithful port of GNU's algorithm in lisp-mode.el."
  (save-excursion
    (beginning-of-line)
    (let ((indent-point (point))
          state
          (desired-indent nil)
          (retry t)
          whitespace-after-open-paren
          calculate-lisp-indent-last-sexp containing-sexp)
      (cond ((or (markerp parse-start) (integerp parse-start))
             (goto-char parse-start))
            ((null parse-start) (beginning-of-defun))
            (t (setq state parse-start)))
      (unless state
        ;; Find outermost containing sexp.
        (while (< (point) indent-point)
          (setq state (parse-partial-sexp (point) indent-point 0))))
      ;; Find innermost containing sexp.
      (while (and retry
                  state
                  (> (elt state 0) 0))
        (setq retry nil)
        (setq calculate-lisp-indent-last-sexp (elt state 2))
        (setq containing-sexp (elt state 1))
        ;; Position following last unclosed open.
        (goto-char (1+ containing-sexp))
        ;; Is there a complete sexp since then?
        (if (and calculate-lisp-indent-last-sexp
                 (> calculate-lisp-indent-last-sexp (point)))
            ;; Yes, but is there a containing sexp after that?
            (let ((peek (parse-partial-sexp calculate-lisp-indent-last-sexp
                                            indent-point 0)))
              (if (setq retry (car (cdr peek))) (setq state peek)))))
      (if retry
          nil
        ;; Innermost containing sexp found.
        (goto-char (1+ containing-sexp))
        (setq whitespace-after-open-paren (looking-at "\\s-"))
        (if (not calculate-lisp-indent-last-sexp)
            ;; indent-point immediately follows open paren.
            (setq desired-indent (current-column))
          ;; Find the start of first element of containing sexp.
          (parse-partial-sexp (point) calculate-lisp-indent-last-sexp 0 t)
          (cond ((looking-at "\\s(")
                 ;; First element is itself a list — indent under it.
                 nil)
                ((> (save-excursion (forward-line 1) (point))
                    calculate-lisp-indent-last-sexp)
                 ;; First line to start within the containing sexp.
                 (if (or (= (point) calculate-lisp-indent-last-sexp)
                         whitespace-after-open-paren)
                     nil
                   ;; Skip first element; indent under second.
                   (forward-sexp 1)
                   (parse-partial-sexp (point)
                                       calculate-lisp-indent-last-sexp
                                       0 t))
                 (backward-prefix-chars))
                (t
                 ;; Indent beneath first sexp on the same line as
                 ;; calculate-lisp-indent-last-sexp.
                 (goto-char calculate-lisp-indent-last-sexp)
                 (beginning-of-line)
                 (parse-partial-sexp (point)
                                     calculate-lisp-indent-last-sexp 0 t)
                 (backward-prefix-chars)))))
      (let ((normal-indent (current-column)))
        (cond ((elt state 3)
               ;; Inside a string: don't change indentation.
               nil)
              ((and (integerp lisp-indent-offset) containing-sexp)
               (goto-char containing-sexp)
               (+ (current-column) lisp-indent-offset))
              (calculate-lisp-indent-last-sexp
               (or (and lisp-indent-function
                        (not retry)
                        (funcall lisp-indent-function indent-point state))
                   ;; Align a keyword arg under a preceding keyword.
                   (and (save-excursion
                          (goto-char indent-point)
                          (skip-chars-forward " \t")
                          (looking-at ":"))
                        (save-excursion
                          (goto-char calculate-lisp-indent-last-sexp)
                          (backward-prefix-chars)
                          (while (not (save-excursion
                                        (skip-chars-backward " \t")
                                        (or (= (point)
                                               (line-beginning-position))
                                            (and containing-sexp
                                                 (= (point)
                                                    (1+ containing-sexp))))))
                            (forward-sexp -1)
                            (backward-prefix-chars))
                          (setq calculate-lisp-indent-last-sexp (point)))
                        (> calculate-lisp-indent-last-sexp
                           (save-excursion
                             (goto-char (1+ containing-sexp))
                             (parse-partial-sexp
                              (point) calculate-lisp-indent-last-sexp 0 t)
                             (point)))
                        (let ((parse-sexp-ignore-comments t)
                              indent)
                          (goto-char calculate-lisp-indent-last-sexp)
                          (or (and (looking-at ":")
                                   (setq indent (current-column)))
                              (and (< (line-beginning-position)
                                      (prog2 (backward-sexp) (point)))
                                   (looking-at ":")
                                   (setq indent (current-column))))
                          indent))
                   normal-indent))
              (desired-indent)
              (t normal-indent))))))

(defun lisp--local-defform-body-p (state)
  "Non-nil when at a local definition body per STATE (subset)."
  (condition-case nil
      (let ((start (nth 1 state)))
        (when start
          (let* ((parents (nth 9 state))
                 (first-cons-after (cdr parents))
                 (second-cons-after (cdr first-cons-after))
                 first-order-parent second-order-parent)
            (while second-cons-after
              (when (= start (car second-cons-after))
                (setq second-order-parent (pop parents)
                      first-order-parent (pop parents)
                      second-cons-after nil))
              (pop second-cons-after)
              (pop parents))
            (when second-order-parent
              (let (local-definitions-starting-point)
                (and (save-excursion
                       (goto-char (1+ second-order-parent))
                       (let ((head (ignore-errors (read (current-buffer)))))
                         (when (memq head '(cl-flet cl-labels cl-macrolet
                                            cl-flet* cl-symbol-macrolet))
                           (setq local-definitions-starting-point
                                 (progn
                                   (parse-partial-sexp
                                    (point) first-order-parent nil t)
                                   (point)))
                           local-definitions-starting-point)))
                     (save-excursion
                       (when (ignore-errors (backward-up-list 2) t)
                         (= local-definitions-starting-point
                            (point))))))))))
    (error nil)))

(defun lisp-indent-function (indent-point state)
  "This function is the normal value of `lisp-indent-function'.
Port of GNU's lisp-mode.el indentation dispatch."
  (let ((normal-indent (current-column)))
    (goto-char (1+ (elt state 1)))
    (parse-partial-sexp (point) calculate-lisp-indent-last-sexp 0 t)
    (if (and (elt state 2)
             (not (looking-at "\\sw\\|\\s_")))
        ;; Car of form doesn't seem to be a symbol.
        (if (lisp--local-defform-body-p state)
            (lisp-indent-defform state indent-point)
          (if (not (> (save-excursion (forward-line 1) (point))
                      calculate-lisp-indent-last-sexp))
              (progn (goto-char calculate-lisp-indent-last-sexp)
                     (beginning-of-line)
                     (parse-partial-sexp (point)
                                         calculate-lisp-indent-last-sexp
                                         0 t)))
          ;; Indent under the list or under the first sexp on the same
          ;; line as calculate-lisp-indent-last-sexp.
          (backward-prefix-chars)
          (current-column))
      (let* ((function (intern-soft
                        (buffer-substring (point)
                                          (progn (forward-sexp 1)
                                                 (point)))))
             (local (assq function lisp-indent-local-overrides))
             (method (if local
                         (cdr local)
                       (or (function-get function 'lisp-indent-function
                                         'macro)
                           (get function 'lisp-indent-hook)))))
        (cond ((or (eq method 'defun)
                   (lisp--local-defform-body-p state))
               (lisp-indent-defform state indent-point))
              ((integerp method)
               (lisp-indent-specform method state
                                     indent-point normal-indent))
              (method
               (funcall method indent-point state)))))))

(defun lisp-indent-specform (count state indent-point normal-indent)
  "Indent a specform with COUNT distinguished arguments."
  (let ((containing-form-start (elt state 1))
        (i count)
        body-indent containing-form-column)
    (goto-char containing-form-start)
    (setq containing-form-column (current-column))
    (setq body-indent (+ lisp-body-indent containing-form-column))
    (forward-char 1)
    (forward-sexp 1)
    ;; Find the start of the last form.
    (parse-partial-sexp (point) indent-point 1 t)
    (while (and (< (point) indent-point)
                (condition-case ()
                    (progn
                      (setq count (1- count))
                      (forward-sexp 1)
                      (parse-partial-sexp (point) indent-point 1 t))
                  (error nil))))
    ;; Point is on first character of last (or count) sexp.
    (if (> count 0)
        ;; A distinguished form.
        (if (<= (- i count) 1)
            (list (+ containing-form-column (* 2 lisp-body-indent))
                  containing-form-start)
          (list normal-indent containing-form-start))
      ;; A non-distinguished form.
      (if (or (and (= i 0) (= count 0))
              (and (= count 0) (<= body-indent normal-indent)))
          body-indent
        normal-indent))))

(defun lisp-indent-defform (state _indent-point)
  "Indent a `defun'-style form."
  (goto-char (car (cdr state)))
  (forward-line 1)
  (if (> (point) (car (cdr (cdr state))))
      (progn
        (goto-char (car (cdr state)))
        (+ lisp-body-indent (current-column)))))

(defun lisp-indent-line (&optional indent)
  "Indent current line as Lisp code."
  (interactive)
  (let ((pos (- (point-max) (point)))
        (indent (progn (beginning-of-line)
                       (or indent (calculate-lisp-indent (lisp-ppss))))))
    (skip-chars-forward " \t")
    (if (or (null indent) (looking-at "\\s<\\s<\\s<"))
        ;; Don't alter indentation of a ;;; comment line or a line
        ;; that starts in a string.
        (goto-char (- (point-max) pos))
      (if (and (looking-at "\\s<") (not (looking-at "\\s<\\s<")))
          ;; Single-semicolon comment lines indent as comments.
          (progn (indent-for-comment) (forward-char -1))
        (if (listp indent) (setq indent (car indent)))
        (indent-line-to indent))
      ;; If initial point was within line's indentation, position
      ;; after the indentation.  Else stay at same point in text.
      (if (> (- (point-max) pos) (point))
          (goto-char (- (point-max) pos))))))

;; ---------- major modes ----------

(defvar parse-sexp-ignore-comments nil
  "Non-nil means `forward-sexp', etc., should treat comments as whitespace.")

(define-derived-mode prog-mode nil "ProgMode"
  "Major mode for editing programming languages.
Typically the base mode for language-specific modes."
  (setq-local parse-sexp-ignore-comments t))

;; ---------- syntax-propertize (GNU syntax.el) ----------

(defvar syntax-propertize-function nil
  "Mode-specific function to apply `syntax-table' text properties.
Called with two arguments (START END) covering the text to propertize.")
(defvar parse-sexp-lookup-properties nil
  "Non-nil means `forward-sexp', etc., obey `syntax-table' property.")
(defvar syntax-propertize--done -1
  "Position up to which syntax-table properties have been set.")
(make-variable-buffer-local 'syntax-propertize--done)
(defvar syntax-propertize-chunks 2000
  "Minimal size of a syntax-propertize chunk.")
(defvar syntax-ppss-table nil
  "Syntax table used by `syntax-ppss' and `syntax-propertize'.")

(defun syntax-propertize (pos)
  "Ensure that syntax-table properties are set until POS (a buffer point)."
  (when (< syntax-propertize--done pos)
    (if (memq syntax-propertize-function '(nil ignore))
        (setq syntax-propertize--done (max (point-max) pos))
      (setq-local parse-sexp-lookup-properties t)
      (when (< syntax-propertize--done (point-min))
        (setq syntax-propertize--done (point-min)))
      (with-silent-modifications
        (let* ((start (max (min syntax-propertize--done (point-max))
                           (point-min)))
               (end (max pos
                         (min (point-max)
                              (+ start syntax-propertize-chunks)))))
          (remove-text-properties
           start end '(syntax-table nil syntax-multiline nil))
          ;; Move the limit before calling the function, so it's done
          ;; in case of errors (as in GNU).
          (setq syntax-propertize--done end)
          ;; Bind `syntax-propertize--done' to avoid recursion.
          (let ((syntax-propertize--done most-positive-fixnum))
            (funcall syntax-propertize-function start end)))))))

(defun internal--syntax-propertize (charpos)
  "Propertize text through at least CHARPOS (called from the scanner)."
  (save-match-data
    (syntax-propertize
     (min (+ syntax-propertize-chunks charpos) (point-max)))))

(defun elisp-mode-syntax-propertize (start end)
  ;; Port of GNU `elisp-mode-syntax-propertize' (elisp-mode.el): the
  ;; same four rules as its `syntax-propertize-rules' expansion.
  (save-excursion
    (goto-char start)
    (let ((case-fold-search nil))
      (while (< (point) end)
        (cond
       ;; Empty symbol.
       ((looking-at (string ?# ?#))
        (unless (nth 8 (syntax-ppss))
          (put-text-property (point) (match-end 0)
                             'syntax-table (string-to-syntax "_")))
        (goto-char (match-end 0)))
       ;; Prevent the @ from becoming part of a following symbol.
       ((looking-at ",@")
        (unless (nth 8 (syntax-ppss))
          (put-text-property (point) (match-end 0)
                             'syntax-table (string-to-syntax "'")))
        (goto-char (match-end 0)))
       ;; Unicode character names.
       ((looking-at "\\?\\\\N{[-A-Za-z0-9 ]\\{,100\\}}")
        (unless (nth 8 (syntax-ppss))
          (put-text-property (point) (match-end 0)
                             'syntax-table (string-to-syntax "_")))
        (goto-char (match-end 0)))
       ;; Bool-vectors, records, char-tables.
       ((looking-at (concat (string ?#) "\\(&[0-9]+\\|s\\|\\^+\\)\\(\"\\|(\\|\\[\\)"))
        (unless (save-excursion (nth 8 (syntax-ppss (match-beginning 0))))
          (put-text-property (match-beginning 1) (match-end 1)
                             'syntax-table (string-to-syntax "'")))
        (goto-char (match-end 1)))
       (t (forward-char 1)))))))

(define-derived-mode special-mode nil "Special"
  "Major mode for buffers containing read-only text.")

(define-derived-mode messages-buffer-mode special-mode "Messages"
  "Major mode for the *Messages* buffer.")

(define-derived-mode text-mode nil "Text"
  "Major mode for editing text intended for humans to read.")

(defvar lisp-mode-map
  (let ((m (make-sparse-keymap))) m)
  "Keymap for Lisp mode.")

;; GNU 31's lisp-mode.el: `lisp-data-mode-syntax-table' holds the
;; Lisp-dialect overrides; `lisp-mode-syntax-table' and
;; `emacs-lisp-mode-syntax-table' are `make-syntax-table' copies of
;; it (empty char-tables parented to it) plus their own tweaks.
(defvar lisp-data-mode-syntax-table
  (let ((table (make-syntax-table))
        (i 0))
    (while (< i ?0)
      (modify-syntax-entry i "_   " table)
      (setq i (1+ i)))
    (setq i (1+ ?9))
    (while (< i ?A)
      (modify-syntax-entry i "_   " table)
      (setq i (1+ i)))
    (setq i (1+ ?Z))
    (while (< i ?a)
      (modify-syntax-entry i "_   " table)
      (setq i (1+ i)))
    (setq i (1+ ?z))
    (while (< i 128)
      (modify-syntax-entry i "_   " table)
      (setq i (1+ i)))
    (modify-syntax-entry ?\s "    " table)
    ;; Non-break space acts as whitespace.
    (modify-syntax-entry ?\xa0 "    " table)
    (modify-syntax-entry ?\t "    " table)
    (modify-syntax-entry ?\f "    " table)
    (modify-syntax-entry ?\n ">   " table)
    (modify-syntax-entry ?\; "<   " table)
    (modify-syntax-entry ?` "'   " table)
    (modify-syntax-entry ?' "'   " table)
    (modify-syntax-entry ?, "'   " table)
    (modify-syntax-entry ?@ "_ p" table)
    ;; Used to be singlequote; changed for flonums.
    (modify-syntax-entry ?. "_   " table)
    (modify-syntax-entry ?# "'   " table)
    (modify-syntax-entry ?\" "\"    " table)
    (modify-syntax-entry ?\\ "\\   " table)
    (modify-syntax-entry ?\( "()  " table)
    (modify-syntax-entry ?\) ")(  " table)
    (modify-syntax-entry ?\[ "(]  " table)
    (modify-syntax-entry ?\] ")[  " table)
    table)
  "Parent syntax table used in Lisp modes.")

(defvar lisp-mode-syntax-table
  (let ((table (make-syntax-table lisp-data-mode-syntax-table)))
    (modify-syntax-entry ?\[ "_   " table)
    (modify-syntax-entry ?\] "_   " table)
    (modify-syntax-entry ?# "' 14" table)
    (modify-syntax-entry ?| "\" 23bn" table)
    table)
  "Syntax table used in `lisp-mode'.")

(defvar emacs-lisp-mode-syntax-table
  (let ((table (make-syntax-table lisp-data-mode-syntax-table)))
    ;; Remove the "p" flag from the entry of `@' because we use instead
    ;; `syntax-propertize' to take care of `,@', which is more precise.
    ;; FIXME: We should maybe do the same in other Lisp modes?  (bug#24542)
    (modify-syntax-entry ?@ "_" table)
    table)
  "Syntax table used in `emacs-lisp-mode'.")

;; `lisp-indent-function' properties, matching GNU (set via `declare'
;; and lisp-mode.el dolists upstream).
(dolist (x '((defun . 2) (defmacro . 2) (defsubst . 2)
             (defvar . defun) (defconst . defun) (defcustom . defun)
             (defalias . defun) (lambda . defun)
             (let . 1) (let* . 1) (if . 2) (when . 1) (unless . 1)
             (while . 1) (dolist . 1) (dotimes . 1) (condition-case . 2)
             (prog1 . 1) (prog2 . 2) (progn . 0)
             (save-excursion . 0) (save-restriction . 0)
             (save-current-buffer . 0) (save-match-data . 0)
             (with-current-buffer . 1) (with-temp-buffer . 0)
             (with-temp-file . 1) (unwind-protect . 1) (catch . 1)
             (track-mouse . 0) (with-silent-modifications . 0)
             (ignore-errors . 0) (eval-after-load . 1)
             (with-output-to-temp-buffer . 1) (with-syntax-table . 1)
             (combine-after-change-calls . 0)))
  (put (car x) 'lisp-indent-function (cdr x)))

(define-derived-mode lisp-data-mode prog-mode "Lisp-Data"
  "Major mode for editing Lisp data (as opposed to code)."
  (setq-local comment-start ";")
  (setq-local comment-start-skip ";+ *"))

(define-derived-mode lisp-mode lisp-data-mode "Lisp"
  "Major mode for editing Lisp code."
  (setq-local indent-line-function #'lisp-indent-line)
  (setq-local comment-start ";")
  (setq-local comment-start-skip ";+ *"))

(define-derived-mode emacs-lisp-mode lisp-data-mode "Emacs-Lisp"
  "Major mode for editing Emacs Lisp code."
  (setq-local indent-line-function #'lisp-indent-line)
  (setq-local comment-start ";")
  (setq-local comment-start-skip ";+ *")
  (setq-local syntax-propertize-function #'elisp-mode-syntax-propertize)
  ;; GNU ends up with this enabled in elisp buffers (set lazily by
  ;; `syntax-propertize'); setting it eagerly matches the observable
  ;; state.
  (setq-local parse-sexp-lookup-properties t))

(define-derived-mode lisp-interaction-mode emacs-lisp-mode
  "Lisp Interaction"
  "Major mode for typing and evaluating Emacs Lisp code."
  :abbrev-table nil)

;; ---------- remaining navigation/indentation commands ----------

(defvar widen-automatically t
  "Non-nil means widen automatically for various commands.")
(defvar defun-prompt-regexp nil
  "Non-nil means a regexp to skip before a defun's opening paren.")
(defvar open-paren-in-column-0-is-defun-start t
  "Non-nil means an open paren in column 0 starts a defun.")
(defvar comment-use-syntax-ppss t
  "Non-nil means comment-related functions use `syntax-ppss'.")
(defvar indent-region-function nil
  "Function to indent a region, or nil to indent each line.")

;; ---------- defun navigation (GNU lisp.el) ----------

(defvar beginning-of-defun-function nil
  "If non-nil, function for `beginning-of-defun-raw' to call.")

(defun beginning-of-defun (&optional arg)
  "Move backward to the beginning of a defun.
With ARG, do it that many times.  Negative ARG means move forward
to the ARGth following beginning of defun."
  (interactive "^p")
  (or (not (eq this-command 'beginning-of-defun))
      (eq last-command 'beginning-of-defun)
      (and transient-mark-mode mark-active)
      (push-mark))
  (and (beginning-of-defun-raw arg)
       (progn (beginning-of-line) t)))

(defun syntax-ppss-toplevel-pos (ppss)
  "Outermost position found by the scan that produced PPSS."
  (or (car (nth 9 ppss))
      (nth 8 ppss)))

(defun beginning-of-defun-raw (&optional arg)
  "Move point to the character that starts a defun."
  (interactive "^p")
  (unless arg (setq arg 1))
  (cond
   (beginning-of-defun-function
    (condition-case nil
        (funcall beginning-of-defun-function arg)
      (wrong-number-of-arguments
       (if (> arg 0)
           (dotimes (_ arg)
             (funcall beginning-of-defun-function))
         (dotimes (_ (- arg))
           (funcall end-of-defun-function))))))

   ((or defun-prompt-regexp open-paren-in-column-0-is-defun-start)
    (and (< arg 0) (not (eobp)) (forward-char 1))
    (and (let (found)
           (while
               (and (setq found
                          (re-search-backward
                           (if defun-prompt-regexp
                               (concat (if open-paren-in-column-0-is-defun-start
                                           "^\\s(\\|" "")
                                       "\\(?:" defun-prompt-regexp "\\)\\s(")
                             "^\\s(")
                           nil 'move arg))
                    (save-match-data
                      (nth 8 (syntax-ppss)))))
           found)
         (progn (goto-char (1- (match-end 0)))
                t)))

   ((eq arg 0))
   (t
    (let ((floor (point-min))
          (ceiling (point-max))
          (arg-+ve (> arg 0)))
      (save-restriction
        (widen)
        (let ((ppss (syntax-ppss))
              encl-pos)
          (when (nth 8 ppss)
            (goto-char (nth 8 ppss))
            (setq ppss (syntax-ppss)))
          (setq encl-pos (syntax-ppss-toplevel-pos ppss))
          (if encl-pos (goto-char encl-pos))
          (and encl-pos arg-+ve (setq arg (1- arg)))
          (and (not encl-pos) (not arg-+ve) (not (looking-at "\\s("))
               (setq arg (1+ arg)))
          (condition-case nil
              (progn
                (goto-char (scan-lists (point) (- arg) 0))
                (if arg-+ve
                    (if (>= (point) floor)
                        t
                      (goto-char floor)
                      nil)
                  (goto-char (1- (scan-lists (point) 1 -1)))
                  (if (<= (point) ceiling)
                      t
                    (goto-char ceiling)
                    nil)))
            (error
             (goto-char (if arg-+ve floor ceiling))
             nil))))))))

(defun beginning-of-defun--in-emptyish-line-p ()
  "Whether point is in a line of only comments and/or whitespace."
  (save-excursion
    (forward-line 0)
    (let ((ppss (syntax-ppss)))
      (and (null (nth 3 ppss))
           (< (line-end-position)
              (progn (when (nth 4 ppss)
                       (goto-char (nth 8 ppss)))
                     (forward-comment (point-max))
                     (point)))))))

(defun beginning-of-defun-comments (&optional arg)
  "Move to the beginning of ARGth defun, including comments."
  (interactive "^p")
  (unless arg (setq arg 1))
  (beginning-of-defun arg)
  (let (first-line-p)
    (while (let ((ppss (progn (setq first-line-p (= (forward-line -1) -1))
                              (syntax-ppss (line-end-position)))))
             (while (and (nth 4 ppss)
                         (< (nth 8 ppss) (line-beginning-position)))
               (goto-char (nth 8 ppss))
               (setq ppss (syntax-ppss (line-end-position))))
             (and (not first-line-p)
                  (progn (skip-syntax-backward
                          "-" (line-beginning-position))
                         (not (bolp)))
                  (beginning-of-defun--in-emptyish-line-p))))
    (forward-line (if first-line-p 0 1))))

(defvar end-of-defun-function
  (lambda () (forward-sexp 1))
  "Function for `end-of-defun' to call.")
(defvar end-of-defun-moves-to-eol t
  "Whether `end-of-defun' moves to eol before doing anything else.")

(defun end-of-defun (&optional arg interactive)
  "Move forward to next end of defun."
  (interactive "^p\nd")
  (if interactive
      (condition-case e
          (end-of-defun arg nil)
        (scan-error (user-error (cadr e))))
    (or (not (eq this-command 'end-of-defun))
        (eq last-command 'end-of-defun)
        (and transient-mark-mode mark-active)
        (push-mark))
    (if (or (null arg) (= arg 0)) (setq arg 1))
    (let ((pos (point))
          (success nil)
          (beg (progn (when end-of-defun-moves-to-eol
                        (end-of-line 1))
                      (beginning-of-defun-raw 1) (point)))
          (skip (lambda ()
                  (unless (bolp)
                    (skip-chars-forward " \t")
                    (if (looking-at "\\s<\\|\n")
                        (forward-line 1))))))
      (funcall end-of-defun-function)
      (when (<= arg 1)
        (funcall skip))
      (cond
       ((> arg 0)
        (if (> (point) pos)
            (setq arg (1- arg))
          (goto-char pos))
        (unless (zerop arg)
          (when (setq success (beginning-of-defun-raw (- arg)))
            (funcall end-of-defun-function))))
       ((< arg 0)
        (if (< (point) pos)
            (setq arg (1+ arg))
          (goto-char beg))
        (unless (zerop arg)
          (when (setq success (beginning-of-defun-raw (- arg)))
            (setq beg (point))
            (funcall end-of-defun-function)))))
      (funcall skip)
      (while (and (< arg 0) (>= (point) pos) success)
        (goto-char beg)
        (setq success (beginning-of-defun-raw (- arg)))
        (if (or (>= (point) beg) (not success))
            (setq arg 0)
          (setq beg (point))
          (funcall end-of-defun-function)
          (funcall skip))))))

(defun mark-defun (&optional arg interactive)
  "Put mark at end of this defun, point at beginning."
  (interactive "p\nd")
  (if interactive
      (condition-case e
          (mark-defun arg nil)
        (scan-error (user-error (cadr e))))
    (setq arg (or arg 1))
    (when (eq last-command 'mark-defun-back)
      (setq arg (- arg)))
    (when (< arg 0)
      (setq this-command 'mark-defun-back))
    (cond ((use-region-p)
           (if (>= arg 0)
               (set-mark
                (save-excursion
                  (goto-char (mark))
                  (dotimes (_ignore arg)
                    (end-of-defun))
                  (point)))
             (beginning-of-defun-comments (- arg))))
          (t
           (let ((opoint (point))
                 beg end)
             (push-mark opoint)
             (beginning-of-defun-comments)
             (setq beg (point))
             (end-of-defun)
             (setq end (point))
             (when (or (and (<= (point) opoint)
                            (> arg 0))
                       (= beg (point-min)))
               (goto-char opoint)
               (end-of-defun)
               (setq end (point))
               (beginning-of-defun-comments)
               (setq beg (point)))
             (goto-char beg)
             (cond ((> arg 0)
                    (dotimes (_ignore arg)
                      (end-of-defun))
                    (setq end (point))
                    (push-mark end nil t)
                    (goto-char beg))
                   (t
                    (goto-char beg)
                    (unless (= arg -1)
                      (beginning-of-defun (1- (- arg))))
                    (push-mark end nil t))))))
    (skip-chars-backward "[:space:]\n")
    (unless (bobp)
      (forward-line 1))))

(defvar narrow-to-defun-include-comments nil
  "If non-nil, `narrow-to-defun' shows comments preceding the defun.")

(defun narrow-to-defun (&optional include-comments)
  "Make text outside current defun invisible."
  (interactive (list narrow-to-defun-include-comments))
  (save-excursion
    (widen)
    (let ((opoint (point))
          beg end)
      (let ((here (point)))
        (unless (eolp)
          (forward-char))
        (beginning-of-defun)
        (when (< (point) here)
          (goto-char here)
          (beginning-of-defun)))
      (setq beg (point))
      (end-of-defun)
      (setq end (point))
      (while (looking-at "^\n")
        (forward-line 1))
      (unless (> (point) opoint)
        (goto-char opoint)
        (end-of-defun)
        (setq end (point))
        (beginning-of-defun)
        (setq beg (point)))
      (when include-comments
        (goto-char beg)
        (when (forward-comment -1)
          (while (forward-comment -1))
          (when (and page-delimiter (not (string= page-delimiter "")))
            (while (re-search-forward page-delimiter beg t)))
          (skip-chars-forward "[:space:]\n")
          (beginning-of-line)
          (setq beg (point))))
      (goto-char end)
      (re-search-backward "^\n" (- (point) 1) t)
      (narrow-to-region beg end))))

(defun split-line (&optional arg)
  "Split current line at point into two lines, first indenting the
second so it aligns with the text that follows point."
  (interactive "*P")
  (let ((col (current-column))
        (pos (point)))
    (newline (prefix-numeric-value arg))
    (goto-char pos)
    (indent-to col)
    (goto-char (1+ pos))))

(defun indent-next-tab-stop (column &optional prev)
  "Return the next tab stop after COLUMN.
If PREV is non-nil, return the previous one instead."
  (let ((tabs tab-stop-list))
    (while (and tabs (>= column (car tabs)))
      (setq tabs (cdr tabs)))
    (if tabs
        (if (not prev)
            (car tabs)
          (let ((prevtabs (cdr (memq (car tabs) (reverse tab-stop-list)))))
            (if (null prevtabs) 0
              (if (= column (car prevtabs))
                  (or (nth 1 prevtabs) 0)
                (car prevtabs)))))
      ;; We passed the end of tab-stop-list: guess a continuation.
      (let* ((last2 (last tab-stop-list 2))
             (step (if (cdr last2) (- (cadr last2) (car last2)) tab-width))
             (last (or (cadr last2) (car last2) 0)))
        ;; Repeat the last tab's length.
        (+ last (* step (if prev
                            (if (<= column last) -1 (/ (- column last 1) step))
                          (1+ (/ (- column last) step)))))))))

;;; Abbrev mode — port of GNU's obarray.el + abbrev.el.

;; From GNU's obarray.el (these are Lisp defuns there).

(defconst obarray-default-size 4
  "The default size of all obarrays.")

(defun obarray-size (_ob)
  (declare (obsolete "obarrays now grow automatically." "30.1"))
  obarray-default-size)

(defun obarray-get (ob name)
  "Return symbol named NAME if it is contained in obarray OB.
Return nil otherwise."
  (intern-soft name ob))

(defun obarray-put (ob name)
  "Return symbol named NAME from obarray OB.
Creates and adds the symbol if it doesn't exist."
  (intern name ob))

(defun obarray-remove (ob name)
  "Remove symbol named NAME if it is contained in obarray OB.
Return t on success, nil otherwise."
  (unintern name ob))

(defun obarray-map (fn ob)
  "Call function FN on every symbol in obarray OB and return nil."
  (mapatoms fn ob))

;;; From GNU's abbrev.el.

(defcustom abbrev-file-name
  "~/.emacs.d/abbrev_defs"
  "Default name of file from which to read and where to save abbrevs."
  :type 'file)

(defcustom only-global-abbrevs nil
  "Non-nil means user plans to use only global abbrevs.
This makes the commands that normally define mode-specific abbrevs
define global abbrevs instead."
  :type 'boolean)

(defun abbrev-mode (&optional arg)
  "Toggle Abbrev mode in the current buffer.

In Abbrev mode, inserting an abbreviation causes it to expand and
be replaced by its expansion."
  ;; GNU's `define-minor-mode' with `:variable abbrev-mode' generates
  ;; exactly this body.
  (interactive (list (or current-prefix-arg 'toggle)))
  (let ((enable (if (eq arg 'toggle)
                    (not abbrev-mode)
                  (> (prefix-numeric-value arg) 0))))
    (setq-local abbrev-mode enable)
    (when (called-interactively-p 'any)
      (message "Abbrev mode %sabled in current buffer"
               (if enable "en" "dis")))
    abbrev-mode))

(put 'abbrev-mode 'safe-local-variable #'booleanp)

(defvar-keymap edit-abbrevs-mode-map
  :doc "Keymap used in `edit-abbrevs'."
  "C-x C-s" #'abbrev-edit-save-buffer
  "C-x C-w" #'abbrev-edit-save-to-file
  "C-c C-c" #'edit-abbrevs-redefine)
(defvaralias 'edit-abbrevs-map 'edit-abbrevs-mode-map)

(defun kill-all-abbrevs ()
  "Undefine all defined abbrevs."
  (interactive)
  (dolist (tablesym abbrev-table-name-list)
    (clear-abbrev-table (symbol-value tablesym))))

(defun insert-abbrevs ()
  "Insert the description of all defined abbrevs after point.
Set mark after the inserted text."
  (interactive)
  (push-mark
   (save-excursion
     (dolist (tablesym abbrev-table-name-list)
       (insert-abbrev-table-description tablesym t))
     (point))))

(defun copy-abbrev-table (table)
  "Make a new abbrev-table with the same abbrevs as TABLE.
This function does not copy property lists of the abbrevs.
See `define-abbrev' for the documentation of abbrev properties."
  (let ((new-table (make-abbrev-table)))
    (obarray-map
     (lambda (symbol)
       (define-abbrev new-table
         (symbol-name symbol)
         (symbol-value symbol)
         (symbol-function symbol)))
     table)
    new-table))

(defun list-abbrevs (&optional local)
  "Display a list of the defined abbrevs.
If LOCAL is non-nil (interactively, when invoked with a
prefix arg), display only local, i.e. mode-specific, abbrevs.
Otherwise display all the abbrevs."
  (interactive "P")
  (display-buffer (prepare-abbrev-list-buffer local)))

(defun abbrev-table-name (table)
  "Return the name of the specified abbrev TABLE."
  (let ((tables abbrev-table-name-list)
        found)
    (while (and (not found) tables)
      (when (eq (symbol-value (car tables)) table)
        (setq found (car tables)))
      (setq tables (cdr tables)))
    found))

(defun prepare-abbrev-list-buffer (&optional local)
  "Return buffer listing abbreviations and expansions for each abbrev table.

If LOCAL is non-nil, include in the buffer only the local abbrevs."
  (let ((local-table local-abbrev-table))
    (with-current-buffer (get-buffer-create "*Abbrevs*")
      (erase-buffer)
      (if local
          (insert-abbrev-table-description
           (abbrev-table-name local-table) t)
        (let (empty-tables)
          (dolist (table abbrev-table-name-list)
            (if (abbrev-table-empty-p (symbol-value table))
                (push table empty-tables)
              (insert-abbrev-table-description table t)))
          (dolist (table (nreverse empty-tables))
            (insert-abbrev-table-description table t)))
        ;; Note: `list-abbrevs' can display only local abbrevs, in
        ;; which case editing could lose abbrevs of other tables.
        ;; Thus enter `edit-abbrevs-mode' only if LOCAL is nil.
        (edit-abbrevs-mode))
      (goto-char (point-min))
      (set-buffer-modified-p nil)
      (current-buffer))))

(defun edit-abbrevs ()
  "Alter abbrev definitions by editing the list of abbrevs."
  (interactive)
  (let ((table-name (abbrev-table-name local-abbrev-table)))
    (switch-to-buffer (prepare-abbrev-list-buffer))
    (when (and table-name
               (search-forward
                (concat "(" (symbol-name table-name) ")\n\n") nil t))
      (goto-char (match-end 0)))))

(defun edit-abbrevs-redefine ()
  "Redefine abbrevs according to current buffer contents."
  (interactive nil edit-abbrevs-mode)
  (save-restriction
    (widen)
    (define-abbrevs t)
    (set-buffer-modified-p nil)))

(defun define-abbrevs (&optional arg)
  "Define abbrevs according to current visible buffer contents.
See documentation of `edit-abbrevs' for info on the format of the
text you must have in the buffer.
If ARG is non-nil (interactively, when invoked with a prefix
argument), eliminate all abbrev definitions except the ones
defined by the current buffer contents."
  (interactive "P")
  (if arg (kill-all-abbrevs))
  (save-excursion
    (goto-char (point-min))
    (while (and (not (eobp)) (re-search-forward "^(" nil t))
      (let* ((buf (current-buffer))
             (table (read buf))
             abbrevs name hook exp count sys)
        (forward-line 1)
        (while (and (not (eobp))
                    ;; Advance as long as we're looking at blank lines
                    ;; or we have an abbrev.
                    (looking-at "[ \t\n]\\|\\(\"\\)"))
          (when (match-string 1)
            (setq name (read buf) count (read buf))
            (if (equal count '(sys))
                (setq sys t count (read buf))
              (setq sys nil))
            (setq exp (read buf))
            (skip-chars-backward " \t\n\f")
            (setq hook (if (not (eolp)) (read buf)))
            (skip-chars-backward " \t\n\f")
            (setq abbrevs (cons (list name exp hook count sys) abbrevs)))
          (forward-line 1))
        (define-abbrev-table table abbrevs)))))

(defun read-abbrev-file (&optional file quietly)
  "Read abbrev definitions from file written with `write-abbrev-file'.
Optional argument FILE is the name of the file to read;
it defaults to the value of `abbrev-file-name'.
Optional second argument QUIETLY non-nil means don't display a message
about loading the abbrevs."
  (interactive
   (list
    (read-file-name (format "Read abbrev file (default %s): " abbrev-file-name)
                    nil abbrev-file-name t)))
  (let ((warning-inhibit-types '((files missing-lexbind-cookie))))
    (load (or file abbrev-file-name) nil quietly))
  (setq abbrevs-changed nil))

(defun quietly-read-abbrev-file (&optional file)
  "Quietly read abbrev definitions from file written with `write-abbrev-file'.
Optional argument FILE is the name of the file to read;
it defaults to the value of `abbrev-file-name'.
Do not display any messages about loading the abbrevs."
					;(interactive "fRead abbrev file: ")
  (read-abbrev-file file t))

(defun write-abbrev-file (&optional file verbose)
  "Write all user-level abbrev definitions to a file of Lisp code.
This does not include system abbrevs; it includes only the abbrev tables
listed in `abbrev-table-name-list'.
The file written can be loaded in another session to define the same abbrevs.
The argument FILE is the file name to write.  If omitted or nil, it defaults
to the value of `abbrev-file-name'.
If VERBOSE is non-nil, display a message indicating the file where the
abbrevs have been saved."
  (interactive
   (list
    (read-file-name "Write abbrev file: "
                    (file-name-directory (expand-file-name abbrev-file-name))
                    abbrev-file-name)))
  (or (and file (> (length file) 0))
      (setq file abbrev-file-name))
  (let ((coding-system-for-write 'utf-8))
    (with-temp-buffer
      (dolist (table
               ;; We sort the table in order to ease the automatic
               ;; merging of different versions of the user's abbrevs
               ;; file.  This is useful, for example, when the
               ;; user keeps their home directory in a revision
               ;; control system, and therefore keeps multiple
               ;; slightly-differing loosely synchronized copies.
               (sort (copy-sequence abbrev-table-name-list)
                     (lambda (s1 s2)
                       (string< (symbol-name s1)
                                (symbol-name s2)))))
        (if (abbrev--table-symbols table)
            (insert-abbrev-table-description table nil)))
      (when (unencodable-char-position (point-min) (point-max) 'utf-8)
        (setq coding-system-for-write 'utf-8-emacs))
      (goto-char (point-min))
      (insert (format ";; -*- coding: %S; lexical-binding: t -*-\n"
                      coding-system-for-write))
      (write-region nil nil file nil (and (not verbose) 0)))))

(defun abbrev-edit-save-to-file (file)
  "Save to FILE all the user-level abbrev definitions in current buffer."
  (interactive
   (list (read-file-name "Save abbrevs to file: "
                         (file-name-directory
                          (expand-file-name abbrev-file-name))
                         abbrev-file-name))
   edit-abbrevs-mode)
  (edit-abbrevs-redefine)
  (write-abbrev-file file t))

(defun abbrev-edit-save-buffer ()
  "Save all the user-level abbrev definitions in current buffer.
The saved abbrevs are written to the file specified by
`abbrev-file-name'."
  (interactive nil edit-abbrevs-mode)
  (abbrev-edit-save-to-file abbrev-file-name)
  (setq abbrevs-changed nil))

(defun add-mode-abbrev (arg)
  "Define a mode-specific abbrev whose expansion is the last word before point.
If there's an active region, use that as the expansion.

Prefix argument ARG says how many words before point to use for the expansion;
zero means the entire region is the expansion.

A negative ARG means to undefine the specified abbrev.

This command reads the abbreviation from the minibuffer.

See also `inverse-add-mode-abbrev', which performs the opposite task:
if the abbreviation is already in the buffer, use that command to define
a mode-specific abbrev by specifying its expansion in the minibuffer.

Don't use this function in a Lisp program; use `define-abbrev' instead."
  (interactive "P")
  (add-abbrev
   (if only-global-abbrevs
       global-abbrev-table
     (or local-abbrev-table
         (error "No per-mode abbrev table")))
   "Mode" arg))

(defun add-global-abbrev (arg)
  "Define a global (all modes) abbrev whose expansion is last word before point.
If there's an active region, use that as the expansion.

Prefix argument ARG says how many words before point to use for the expansion;
zero means the entire region is the expansion.

A negative ARG means to undefine the specified abbrev.

This command reads the abbreviation from the minibuffer.

See also `inverse-add-global-abbrev', which performs the opposite task:
if the expansion is already in the buffer, use that command to define
a global abbrev by specifying its expansion in the minibuffer.

Don't use this function in a Lisp program; use `define-abbrev' instead."
  (interactive "P")
  (add-abbrev global-abbrev-table "Global" arg))

(defun add-abbrev (table type arg)
  "Define abbrev in TABLE, whose expansion is ARG words before point.
Read the abbreviation from the minibuffer, with prompt TYPE.

ARG of zero means the entire region is the expansion.

A negative ARG means to undefine the specified abbrev.

TYPE is an arbitrary string used to prompt user for the kind of
abbrev, such as \"Global\", \"Mode\".  (This has no influence on the
choice of the actual TABLE).

See `inverse-add-abbrev' for the opposite task.

Don't use this function in a Lisp program; use `define-abbrev' instead."
  (let ((exp
         (cond
          ((or (and (null arg) (use-region-p))
               (zerop (prefix-numeric-value arg)))
           (buffer-substring-no-properties (region-beginning) (region-end)))
          ((> (prefix-numeric-value arg) 0)
           (buffer-substring-no-properties
            (point)
            (save-excursion
              (forward-word (- (prefix-numeric-value arg)))
              (point))))))
        name)
    (setq name
          (read-string (format (if exp "%s abbrev that expands into \"%s\": "
                                 "Undefine %s abbrev: ")
                               type exp)))
    (set-text-properties 0 (length name) nil name)
    (if (or (null exp)
            (not (abbrev-expansion name table))
            (y-or-n-p (format "%s expands into \"%s\"; redefine? "
                              name (abbrev-expansion name table))))
        (define-abbrev table (downcase name) exp))))

(defun inverse-add-mode-abbrev (n)
  "Define the word before point as a mode-specific abbreviation.
With prefix argument N, define the Nth word before point as the
abbreviation.  Negative N means use the Nth word after point.

If `only-global-abbrevs' is non-nil, this command defines a
global (mode-independent) abbrev instead of a mode-specific one.

This command reads the expansion from the minibuffer, defines the
abbrev, and then expands the abbreviation in the current buffer.

See also `add-mode-abbrev', which performs the opposite task:
if the expansion is already in the buffer, use that command
to define an abbrev by specifying the abbreviation in the minibuffer."
  (interactive "p")
  (inverse-add-abbrev
   (if only-global-abbrevs
       global-abbrev-table
     (or local-abbrev-table
         (error "No per-mode abbrev table")))
   "Mode" n))

(defun inverse-add-global-abbrev (n)
  "Define the word before point as a global (mode-independent) abbreviation.
With prefix argument N, define the Nth word before point as the
abbreviation.  Negative N means use the Nth word after point.

This command reads the expansion from the minibuffer, defines the
abbrev, and then expands the abbreviation in the current buffer.

See also `add-global-abbrev', which performs the opposite task:
if the expansion is already in the buffer, use that command
to define an abbrev by specifying the abbreviation in the minibuffer."
  (interactive "p")
  (inverse-add-abbrev global-abbrev-table "Global" n))

(defun inverse-add-abbrev (table type arg)
  "Define the word before point as an abbrev in TABLE.
Read the expansion from the minibuffer, using prompt TYPE, define
the abbrev, and then expand the abbreviation in the current
buffer.

ARG means use the ARG-th word before point as the abbreviation.
Negative ARG means use the ARG-th word after point.

TYPE is an arbitrary string used to prompt user for the kind of
abbrev, such as \"Global\", \"Mode\".  (This has no influence on the
choice of the actual TABLE).

See also `add-abbrev', which performs the opposite task."
  (let (name exp start end)
    (save-excursion
      (forward-word (1+ (- arg)))
      (skip-syntax-backward "^w")
      (setq end (point))
      (backward-word 1)
      (setq start (point)
            name (buffer-substring-no-properties start end)))

    (setq exp (read-string (format "Expansion for %s abbrev \"%s\": " type name)
                           nil nil nil t))
    (when (or (not (abbrev-expansion name table))
              (y-or-n-p (format "%s expands into \"%s\"; redefine? "
                                name (abbrev-expansion name table))))
      (define-abbrev table (downcase name) exp)
      (save-excursion
        (goto-char end)
        (expand-abbrev)))))

(defun abbrev-prefix-mark (&optional arg)
  "Mark point as the beginning of an abbreviation.
The abbrev to be expanded starts at point rather than at the
beginning of a word.  This way, you can expand an abbrev with
a prefix: insert the prefix, use this command, then insert the
abbrev.

This command inserts a hyphen after the prefix, and if the abbrev
is subsequently expanded, this hyphen will be removed.

If the prefix is itself an abbrev, this command expands it,
unless ARG is non-nil.  Interactively, ARG is the prefix
argument."
  (interactive "P")
  (or arg (expand-abbrev))
  (setq abbrev-start-location (point-marker)
        abbrev-start-location-buffer (current-buffer))
  (insert "-"))

(defun expand-region-abbrevs (start end &optional noquery)
  "For each abbrev occurrence in the region, offer to expand it.
Ask the user to type `y' or `n' for each occurrence.
A prefix argument means don't query; expand all abbrevs."
  (interactive "r\nP")
  (save-excursion
    (goto-char start)
    (let ((lim (- (point-max) end))
          pnt string)
      (while (and (not (eobp))
                  (progn (forward-word 1)
                         (<= (setq pnt (point)) (- (point-max) lim))))
        (if (abbrev-expansion
             (setq string
                   (buffer-substring-no-properties
                    (save-excursion (forward-word -1) (point))
                    pnt)))
            (if (or noquery (y-or-n-p (format-message "Expand `%s'? " string)))
                (expand-abbrev)))))))

;;; Abbrev properties.

(defun abbrev-table-get (table prop)
  "Get the property PROP of abbrev table TABLE."
  (let ((sym (obarray-get table "")))
    (if sym (get sym prop))))

(defun abbrev-table-put (table prop val)
  "Set the property PROP of abbrev table TABLE to VAL."
  (let ((sym (obarray-put table "")))
    (set sym nil)          ; Make sure it won't be confused for an abbrev.
    (put sym prop val)))

(defalias 'abbrev-get #'get
  "Get the property PROP of abbrev ABBREV
See `define-abbrev' for the effect of some special properties.

\(fn ABBREV PROP)")

(defalias 'abbrev-put #'put
  "Set the property PROP of abbrev ABBREV to value VAL.
See `define-abbrev' for the effect of some special properties.

\(fn ABBREV PROP VAL)")

;;; Code that used to be implemented in src/abbrev.c

(defvar abbrev-table-name-list '(fundamental-mode-abbrev-table
                                 global-abbrev-table)
  "List of symbols whose values are abbrev tables.")

(defun make-abbrev-table (&optional props)
  "Create a new, empty abbrev table object.
PROPS is a list of properties."
  (let ((table (obarray-make)))
    ;; Each abbrev-table has a `modiff' counter which can be used to detect
    ;; when an abbreviation was added.  An example of use would be to
    ;; construct :regexp dynamically as the union of all abbrev names, so
    ;; `modiff' can let us detect that an abbrev was added and hence :regexp
    ;; needs to be refreshed.
    ;; The presence of `modiff' entry is also used as a tag indicating this
    ;; vector is really an abbrev-table.
    (abbrev-table-put table :abbrev-table-modiff 0)
    (while (consp props)
      (abbrev-table-put table (pop props) (pop props)))
    table))

(defun abbrev-table-p (object)
  "Return non-nil if OBJECT is an abbrev table."
  (and (obarrayp object)
       (numberp (ignore-error wrong-type-argument
                  (abbrev-table-get object :abbrev-table-modiff)))))

(defun abbrev-table-empty-p (object &optional ignore-system)
  "Return nil if there are no abbrev symbols in OBJECT.
If IGNORE-SYSTEM is non-nil, system definitions are ignored."
  (unless (abbrev-table-p object)
    (error "Non abbrev table object"))
  (not (catch 'some
         (obarray-map (lambda (abbrev)
                        (unless (or (zerop (length (symbol-name abbrev)))
                                    (and ignore-system
                                         (abbrev-get abbrev :system)))
                          (throw 'some t)))
                      object))))

(defvar global-abbrev-table (make-abbrev-table)
  "The abbrev table whose abbrevs affect all buffers.
Each buffer may also have a local abbrev table.
If it does, the local table overrides the global one
for any particular abbrev defined in both.")

(defvar abbrev-minor-mode-table-alist nil
  "Alist of abbrev tables to use for minor modes.
Each element looks like (VARIABLE . ABBREV-TABLE);
ABBREV-TABLE is active whenever VARIABLE's value is non-nil;
VARIABLE is supposed to be a minor-mode variable.
ABBREV-TABLE can also be a list of abbrev tables.")

(defvar fundamental-mode-abbrev-table
  (let ((table (make-abbrev-table)))
    ;; Set local-abbrev-table's default to be fundamental-mode-abbrev-table.
    (setq-default local-abbrev-table table)
    table)
  "The abbrev table of mode-specific abbrevs for Fundamental Mode.")

(defvar abbrevs-changed nil
  "Non-nil if any word abbrevs were defined or altered.
This causes `save-some-buffers' to offer to save the abbrevs.")

(defcustom abbrev-all-caps nil
  "Non-nil means expand multi-word abbrevs in all caps if the abbrev was so."
  :type 'boolean)

(defvar abbrev-start-location nil
  "Buffer position for `expand-abbrev' to use as the start of the abbrev.
When nil, use the word before point as the abbrev.
Calling `expand-abbrev' sets this to nil.")

(defvar abbrev-start-location-buffer nil
  "Buffer that `abbrev-start-location' has been set for.
Trying to expand an abbrev in any other buffer clears `abbrev-start-location'.")

(defvar last-abbrev nil
  "The abbrev-symbol of the last abbrev expanded.  See `abbrev-symbol'.")

(defvar last-abbrev-text nil
  "The exact text of the last abbrev that was expanded.
It is nil if the abbrev has already been unexpanded.")

(defvar last-abbrev-location 0
  "The location of the start of the last abbrev that was expanded.")

;; (defvar-local local-abbrev-table fundamental-mode-abbrev-table
;;   "Local (mode-specific) abbrev table of current buffer.")

(defun clear-abbrev-table (table)
  "Undefine all abbrevs in abbrev table TABLE, leaving TABLE empty."
  (setq abbrevs-changed t)
  (let* ((sym (obarray-get table "")))
    (obarray-clear table)
    ;; Preserve the table's properties.
    (unless sym
      (error "Assertion failed"))
    (let ((newsym (obarray-put table "")))
      (set newsym nil)       ; Make sure it won't be confused for an abbrev.
      (setplist newsym (symbol-plist sym)))
    (abbrev-table-put table :abbrev-table-modiff
                      (1+ (abbrev-table-get table :abbrev-table-modiff))))
  ;; For backward compatibility, always return nil.
  nil)

(defun define-abbrev (table abbrev expansion &optional hook &rest props)
  "Define ABBREV in TABLE, to expand into EXPANSION and optionally call HOOK.
ABBREV must be a string, and should be lower-case.
EXPANSION should usually be a string.
To undefine an abbrev, define it with EXPANSION = nil.
If HOOK is non-nil, it should be a function of no arguments;
it is called after EXPANSION is inserted.
If EXPANSION is not a string (and not nil), the abbrev is a
 special one, which does not expand in the usual way but only
 runs HOOK.

If HOOK is a non-nil symbol with a non-nil `no-self-insert' property,
it can control whether the character that triggered abbrev expansion
is inserted.  If such a HOOK returns non-nil, the character is not
inserted.  If such a HOOK returns nil, then so does `abbrev-insert'
\(and `expand-abbrev'), as if no abbrev expansion had taken place.

PROPS is a property list.  The following properties are special:
- `:count': the value for the abbrev's usage-count, which is incremented each
  time the abbrev is used (the default is zero).
- `:system': if non-nil, says that this is a \"system\" abbreviation
  which should not be saved in the user's abbreviation file.
  Unless `:system' is `force', a system abbreviation will not
  overwrite a non-system abbreviation of the same name.
- `:case-fixed': non-nil means that abbreviations are looked up without
  case-folding, and the expansion is not capitalized/upcased.
- `:enable-function': a function of no arguments which returns non-nil
  if the abbrev should be used for a particular call of `expand-abbrev'.

An obsolete but still supported calling form is:

\(define-abbrev TABLE NAME EXPANSION &optional HOOK COUNT SYSTEM)."
  (declare (indent defun))
  (when (and (consp props) (or (null (car props)) (numberp (car props))))
    ;; Old-style calling convention.
    (setq props `(:count ,(car props)
                  ,@(if (cadr props) (list :system (cadr props))))))
  (unless (plist-get props :count)
    (setq props (plist-put props :count 0)))
  (setq props (plist-put props :abbrev-table-modiff
                         (abbrev-table-get table :abbrev-table-modiff)))
  (let ((system-flag (plist-get props :system))
        (sym (obarray-put table abbrev)))
    ;; Don't override a prior user-defined abbrev with a system abbrev,
    ;; unless system-flag is `force'.
    (unless (and (not (memq system-flag '(nil force)))
                 (boundp sym) (symbol-value sym)
                 (not (abbrev-get sym :system)))
      (unless (or system-flag
                  (and (boundp sym)
                       ;; load-file-name
                       (equal (symbol-value sym) expansion)
                       (equal (symbol-function sym) hook)))
        (setq abbrevs-changed t))
      (set sym expansion)
      (fset sym hook)
      (setplist sym
                ;; Don't store the `force' value of `system-flag' into
                ;; the :system property.
                (if (eq 'force system-flag) (plist-put props :system t) props))
      (abbrev-table-put table :abbrev-table-modiff
                        (1+ (abbrev-table-get table :abbrev-table-modiff))))
    abbrev))

(defun abbrev--check-chars (abbrev global)
  "Check if the characters in ABBREV have word syntax in either the
current (if global is nil) or standard syntax table."
  (with-syntax-table
      (cond ((null global) (syntax-table))
            ;; ((syntax-table-p global) global)
            (t (standard-syntax-table)))
    (when (string-match "\\W" abbrev)
      (let ((badchars ())
            (pos 0))
        (while (string-match "\\W" abbrev pos)
          (unless (memq (aref abbrev (match-beginning 0)) badchars)
            (push (aref abbrev (match-beginning 0)) badchars))
          (setq pos (1+ pos)))
        (error "Some abbrev characters (%s) are not word constituents %s"
               (apply #'string (nreverse badchars))
               (if global "in the standard syntax" "in this mode"))))))

(defun define-global-abbrev (abbrev expansion)
  "Define ABBREV as a global abbreviation that expands into EXPANSION.
The characters in ABBREV must all be word constituents in the standard
syntax table."
  (interactive "sDefine global abbrev: \nsExpansion for %s: ")
  (abbrev--check-chars abbrev 'global)
  (define-abbrev global-abbrev-table (downcase abbrev) expansion))

(defun define-mode-abbrev (abbrev expansion)
  "Define ABBREV as a mode-specific abbreviation that expands into EXPANSION.
The characters in ABBREV must all be word-constituents in the current mode."
  (interactive "sDefine mode abbrev: \nsExpansion for %s: ")
  (unless local-abbrev-table
    (error "Major mode has no abbrev table"))
  (abbrev--check-chars abbrev nil)
  (define-abbrev local-abbrev-table (downcase abbrev) expansion))

(defun abbrev--active-tables (&optional tables)
  "Return the list of abbrev tables that are currently active.
TABLES, if non-nil, overrides the usual rules.  It can hold
either a single abbrev table or a list of abbrev tables."
  ;; We could just remove the `tables' arg and let callers use
  ;; (or table (abbrev--active-tables)) but then they'd have to be careful
  ;; to treat the distinction between a single table and a list of tables.
  (cond
   ((consp tables) tables)
   ((obarrayp tables) (list tables))
   (t
    (let ((tables (if (listp local-abbrev-table)
                      (append local-abbrev-table
                              (list global-abbrev-table))
                    (list local-abbrev-table global-abbrev-table))))
      ;; Add the minor-mode abbrev tables.
      (dolist (x abbrev-minor-mode-table-alist)
        (when (and (symbolp (car x)) (boundp (car x)) (symbol-value (car x)))
          (setq tables
                (if (listp (cdr x))
                    (append (cdr x) tables) (cons (cdr x) tables)))))
      tables))))


(defun abbrev--symbol (abbrev table)
  "Return the symbol representing abbrev named ABBREV in TABLE.
This symbol's name is ABBREV, but it is not the canonical symbol of that name;
it is interned in the abbrev-table TABLE rather than the normal obarray.
The value is nil if such an abbrev is not defined."
  (let* ((case-fold (not (abbrev-table-get table :case-fixed)))
         ;; In case the table doesn't set :case-fixed but some of the
         ;; abbrevs do, we have to be careful.
         (sym
          ;; First try without case-folding.
          (or (obarray-get table abbrev)
              (when case-fold
                ;; We didn't find any abbrev, try case-folding.
                (let ((sym (obarray-get table (downcase abbrev))))
                  ;; Only use it if it doesn't require :case-fixed.
                  (and sym (not (abbrev-get sym :case-fixed))
                       sym))))))
    (if (symbol-value sym)
        sym)))


(defun abbrev-symbol (abbrev &optional table)
  "Return the symbol representing the abbrev named ABBREV in TABLE.
This symbol's name is ABBREV, but it is not the canonical symbol of that name;
it is interned in an abbrev-table rather than the normal obarray.
The value is nil if such an abbrev is not defined.
Optional second arg TABLE is the abbrev table to look it up in.
The default is to try buffer's mode-specific abbrev table, then global table."
  (let ((tables (abbrev--active-tables table))
        sym)
    (while (and tables (not sym))
      (let* ((table (pop tables)))
        (setq tables (append (abbrev-table-get table :parents) tables))
        (setq sym (abbrev--symbol abbrev table))))
    sym))


(defun abbrev-expansion (abbrev &optional table)
  "Return the string that ABBREV expands into in the current buffer.
Optionally specify an abbrev TABLE as second arg;
then ABBREV is looked up in that table only."
  (symbol-value (abbrev-symbol abbrev table)))


(defun abbrev--before-point ()
  "Try and find an abbrev before point.  Return it if found, nil otherwise."
  (unless (eq abbrev-start-location-buffer (current-buffer))
    (setq abbrev-start-location nil))

  (let ((tables (abbrev--active-tables))
        (pos (point))
        start end name res)

    (if abbrev-start-location
        (progn
          (setq start abbrev-start-location)
          (setq abbrev-start-location nil)
          ;; Remove the hyphen inserted by `abbrev-prefix-mark'.
          (when (and (< start (point-max))
                     (eq (char-after start) ?-))
            (delete-region start (1+ start))
            (setq pos (1- pos)))
          (skip-syntax-backward " ")
          (setq end (point))
          (when (> end start)
            (setq name (buffer-substring start end))
            (goto-char pos)               ; Restore point.
            (list (abbrev-symbol name tables) name start end)))

      (while (and tables (not (car res)))
        (let* ((table (pop tables))
               (enable-fun (abbrev-table-get table :enable-function)))
          (setq tables (append (abbrev-table-get table :parents) tables))
          (setq res
                (and (or (not enable-fun) (funcall enable-fun))
                     (let ((re (abbrev-table-get table :regexp)))
                       (if (null re)
                           ;; We used to default `re' to "\\<\\(\\w+\\)\\W*"
                           ;; but when words-include-escapes is set, that
                           ;; is not right and fixing it is boring.
                           (let ((lim (point)))
                             (backward-word 1)
                             (setq start (point))
                             (forward-word 1)
                             (setq end (min (point) lim)))
                         (when (looking-back re (line-beginning-position))
                           (setq start (match-beginning 1))
                           (setq end   (match-end 1)))))
                     (setq name  (buffer-substring start end))
                     (let ((abbrev (abbrev--symbol name table)))
                       (when abbrev
                         (setq enable-fun (abbrev-get abbrev :enable-function))
                         (and (or (not enable-fun) (funcall enable-fun))
                              ;; This will also look it up in parent tables.
                              ;; This is not on purpose, but it seems harmless.
                              (list abbrev name start end))))))
          ;; Restore point.
          (goto-char pos)))
      res)))

(defun abbrev-insert (abbrev &optional name wordstart wordend)
  "Insert abbrev ABBREV at point.
If non-nil, NAME is the name by which this abbrev was found.
If non-nil, WORDSTART is the buffer position where to insert the abbrev.
If WORDEND is non-nil, it is a buffer position; the abbrev replaces the
previous text between WORDSTART and WORDEND.
Return ABBREV if the expansion should be considered as having taken place.
The return value can be influenced by a `no-self-insert' property;
see `define-abbrev' for details."
  (unless name (setq name (symbol-name abbrev)))
  (unless wordstart (setq wordstart (point)))
  (unless wordend (setq wordend wordstart))
  ;; Increment use count.
  (abbrev-put abbrev :count (1+ (abbrev-get abbrev :count)))
  (let ((value abbrev))
    ;; If this abbrev has an expansion, delete the abbrev
    ;; and insert the expansion.
    (when (stringp (symbol-value abbrev))
      (goto-char wordstart)
      ;; Insert at beginning so that markers at the end (e.g. point)
      ;; are preserved.
      (insert (symbol-value abbrev))
      (delete-char (- wordend wordstart))
      (let ((case-fold-search nil))
        ;; If the abbrev's name is different from the buffer text (the
        ;; only difference should be capitalization), then we may want
        ;; to adjust the capitalization of the expansion.
        (when (and (not (equal name (symbol-name abbrev)))
                   (string-match "[[:upper:]]" name))
          (if (not (string-match "[[:lower:]]" name))
              ;; Abbrev was all caps.  If expansion is multiple words,
              ;; normally capitalize each word.
              (if (and (not abbrev-all-caps)
                       (save-excursion
                         (> (progn (backward-word 1) (point))
                            (progn (goto-char wordstart)
                                   (forward-word 1) (point)))))
                  (upcase-initials-region wordstart (point))
                (upcase-region wordstart (point)))
            ;; Abbrev included some caps.  Cap first initial of expansion.
            (let ((end (point)))
              ;; Find the initial.
              (goto-char wordstart)
              (skip-syntax-forward "^w" (1- end))
              ;; Change just that.
              (upcase-initials-region (point) (1+ (point)))
              (goto-char end))))))
    ;; Now point is at the end of the expansion and the beginning is
    ;; in last-abbrev-location.
    (when (symbol-function abbrev)
      (let* ((hook (symbol-function abbrev))
             (expanded
              ;; If the abbrev has a hook function, run it.
              (funcall hook)))
        ;; In addition, if the hook function is a symbol with
        ;; a non-nil `no-self-insert' property, let the value it
        ;; returned specify whether we consider that an expansion took
        ;; place.  If it returns nil, no expansion has been done.
        (if (and (symbolp hook)
                 (null expanded)
                 (get hook 'no-self-insert))
            (setq value nil))))
    value))

;; GNU's `make-obsolete-variable' subr — records obsolescence so the
;; byte-compiler (and `describe-variable') can warn.
(defun make-obsolete-variable (obsolete-name current-name
                                             &optional when access-type)
  "Make the byte-compiler warn that OBSOLETE-NAME is obsolete."
  (put obsolete-name 'byte-obsolete-variable
       (purecopy (list current-name when access-type))))

(defvar abbrev-expand-functions nil
  "Wrapper hook around `abbrev--default-expand'.")
(make-obsolete-variable 'abbrev-expand-functions 'abbrev-expand-function "24.4")

(defvar abbrev-expand-function #'abbrev--default-expand
  "Function that `expand-abbrev' uses to perform abbrev expansion.
Takes no arguments, and should return the abbrev symbol if expansion
took place.")

(defcustom abbrev-suggest nil
  "Non-nil means suggest using abbrevs to save typing.
When abbrev mode is active and this option is non-nil, Emacs will
suggest in the echo area to use an existing abbrev if doing so
will save enough typing.  See `abbrev-suggest-hint-threshold' for
the definition of \"enough typing\"."
    :type 'boolean
    :version "28.1")

(defcustom abbrev-suggest-hint-threshold 3
  "Threshold for when to suggest to use an abbrev to save typing.
The threshold is the amount of typing, in terms of the number of
characters, that would be saved by using the abbrev.  The
thinking is that if the expansion is only a few characters
longer than the abbrev, the benefit of informing the user is not
significant.  If you always want to be informed about existing
abbrevs for the text you type, set this value to zero or less.
This setting only applies if `abbrev-suggest' is non-nil."
    :type 'natnum
    :version "28.1")

(defun abbrev--suggest-get-active-tables-including-parents ()
  "Return a list of all active abbrev tables, including parent tables."
  (let* ((tables (abbrev--active-tables))
         (all tables))
    (dolist (table tables)
      (setq all (append (abbrev-table-get table :parents) all)))
    all))

(defun abbrev--suggest-get-active-abbrev-expansions ()
  "Return a list of all the active abbrev expansions.
Includes expansions from parent abbrev tables."
  (let (expansions)
    (dolist (table (abbrev--suggest-get-active-tables-including-parents))
      (mapatoms (lambda (e)
                  (let ((value (symbol-value (abbrev--symbol e table))))
                    (when value
                      (push (cons value (symbol-name e)) expansions))))
                table))
    expansions))

(defun abbrev--suggest-count-words (expansion)
  "Return the number of words in EXPANSION.
Expansion is a string of one or more words."
  (length (split-string expansion " " t)))

(defun abbrev--suggest-get-previous-words (n)
  "Return the N words before point, spaces included."
  (let ((end (point)))
    (save-excursion
      (backward-word n)
      (replace-regexp-in-string
       "\\s " " "
       (buffer-substring-no-properties (point) end)))))

(defun abbrev--suggest-above-threshold (expansion)
  "Return non-nil if the abbrev in EXPANSION provides significant savings.
A significant saving, here, means the difference in length between
the abbrev and its expansion is not below the threshold specified
by the value of `abbrev-suggest-hint-threshold'.
EXPANSION is a cons cell where the car is the expansion and the cdr is
the abbrev."
  (>= (- (length (car expansion))
         (length (cdr expansion)))
      abbrev-suggest-hint-threshold))

(defvar abbrev--suggest-saved-recommendations nil
  "Keeps the list of expansions that have abbrevs defined.
The user can show this list by calling
`abbrev-suggest-show-report'.")

(defun abbrev--suggest-inform-user (expansion)
  "Display a message to the user about the existing abbrev.
EXPANSION is a cons cell where the `car' is the expansion and the
`cdr' is the abbrev."
  (run-with-idle-timer
   1 nil
   (lambda ()
     (message "You can write `%s' using the abbrev `%s'."
              (car expansion) (cdr expansion))))
  (push expansion abbrev--suggest-saved-recommendations))

(defun abbrev--suggest-shortest-abbrev (new current)
  "Return the shortest of the two abbrevs given by NEW and CURRENT.
NEW and CURRENT are cons cells where the `car' is the expansion
and the `cdr' is the abbrev."
  (if (not current)
      new
    (if (< (length (cdr new))
           (length (cdr current)))
        new
      current)))

(defun abbrev--suggest-maybe-suggest ()
  "Suggest an abbrev to the user based on the word(s) before point.
Uses `abbrev-suggest-hint-threshold' to find out if the user should be
informed about the existing abbrev."
  (let (words abbrev-found word-count)
    (dolist (expansion (abbrev--suggest-get-active-abbrev-expansions))
      (setq word-count (abbrev--suggest-count-words (car expansion))
            words (abbrev--suggest-get-previous-words word-count))
      (let ((case-fold-search t))
        (when (and (> word-count 0)
                   (string-match (car expansion) words)
                   (abbrev--suggest-above-threshold expansion))
          (setq abbrev-found (abbrev--suggest-shortest-abbrev
                              expansion abbrev-found)))))
    (when abbrev-found
      (abbrev--suggest-inform-user abbrev-found))))

(defun abbrev--suggest-get-totals ()
  "Return a list of all expansions and how many times they were used.
Each expansion in the returned list is a cons cell where the `car' is the
expansion text and the `cdr' is the number of times the expansion has been
typed."
  (let (total cell)
    (dolist (expansion abbrev--suggest-saved-recommendations)
      (if (not (assoc (car expansion) total))
          (push (cons (car expansion) 1) total)
        (setq cell (assoc (car expansion) total))
        (setcdr cell (1+ (cdr cell)))))
    total))

(defun abbrev-suggest-show-report ()
  "Show a buffer with the list of abbrevs you could have used.
This shows the abbrevs you've \"missed\" because you typed the
full text instead of the abbrevs that expand into that text."
  (interactive)
  (let ((totals (abbrev--suggest-get-totals))
        (buf (get-buffer-create "*abbrev-suggest*")))
    (set-buffer buf)
    (erase-buffer)
    (insert (substitute-command-keys "** Abbrev expansion usage **

Below is a list of expansions for which abbrevs are defined, and
the number of times the expansion was typed manually.  To display
and edit all abbrevs, type \\[edit-abbrevs].\n\n"))
    (dolist (expansion totals)
      (insert (format " %s: %d\n" (car expansion) (cdr expansion))))
    (display-buffer buf)))

(defun expand-abbrev ()
  "Expand the abbrev before point, if there is an abbrev there.
Effective when explicitly called even when `abbrev-mode' is nil.
Calls the value of `abbrev-expand-function' with no argument to do
the work, and returns whatever it does.  (That return value should
be the abbrev symbol if expansion occurred, else nil.)"
  (interactive)
  (or (funcall abbrev-expand-function)
      (if abbrev-suggest
          (abbrev--suggest-maybe-suggest))))

(defun abbrev--default-expand ()
  "Default function to use for `abbrev-expand-function'.
This also respects the obsolete wrapper hook `abbrev-expand-functions'.
\(See `with-wrapper-hook' for details about wrapper hooks.)
Calls `abbrev-insert' to insert any expansion, and returns what it does."
  (abbrev--expand-wrapped abbrev-expand-functions))

(defun abbrev--expand-wrapped (wrappers)
  "Run the `abbrev-expand-functions' WRAPPERS around `abbrev--expand-body'.
Each wrapper is called with a continuation function as its first arg."
  (if (consp wrappers)
      (funcall (car wrappers)
               (lambda () (abbrev--expand-wrapped (cdr wrappers))))
    (abbrev--expand-body)))

(defun abbrev--expand-body ()
  "Expand the abbrev found by `abbrev--before-point', if any."
  (let ((res (abbrev--before-point)))
    (let ((sym (nth 0 res)) (name (nth 1 res))
          (wordstart (nth 2 res)) (wordend (nth 3 res)))
      (when sym
        (let ((startpos (copy-marker (point) t))
              (endmark (copy-marker wordend t)))
          (unless (or ;; executing-kbd-macro
                   noninteractive
                   (window-minibuffer-p))
            ;; Add an undo boundary, in case we are doing this for
            ;; a self-inserting command which has avoided making one so far.
            (undo-boundary))
          ;; Now sym is the abbrev symbol.
          (setq last-abbrev-text name)
          (setq last-abbrev sym)
          (setq last-abbrev-location wordstart)
          ;; If this abbrev has an expansion, delete the abbrev
          ;; and insert the expansion.
          (prog1
              (abbrev-insert sym name wordstart wordend)
            ;; Yuck!!  If expand-abbrev is called with point slightly
            ;; further than the end of the abbrev, move point back to
            ;; where it started.
            (if (and (> startpos endmark)
                     (= (point) endmark)) ;Obey skeletons that move point.
                (goto-char startpos))))))))

(defun unexpand-abbrev ()
  "Undo the expansion of the last abbrev that expanded.
This differs from ordinary undo in that other editing done since then
is not undone."
  (interactive)
  (save-excursion
    (when (<= (point-min) last-abbrev-location (point-max))
      (goto-char last-abbrev-location)
      (when (stringp last-abbrev-text)
        ;; This isn't correct if last-abbrev's hook was used
        ;; to do the expansion.
        (let ((val (symbol-value last-abbrev)))
          (unless (stringp val)
            (error "Value of abbrev-symbol must be a string"))
          ;; Don't inherit properties here; just copy from old contents.
          (replace-region-contents (point) (+ (point) (length val))
                                   last-abbrev-text 0)
          (goto-char (+ (point) (length last-abbrev-text)))
          (setq last-abbrev-text nil))))))

(defun abbrev--write (sym)
  "Write the abbrev in a `read'able form.
Presumes that `standard-output' points to `current-buffer'."
  (insert "    (")
  (prin1 (symbol-name sym))
  (insert " ")
  (prin1 (symbol-value sym))
  (insert " ")
  (prin1 (symbol-function sym))
  (insert " :count ")
  (prin1 (abbrev-get sym :count))
  (when (abbrev-get sym :case-fixed)
    (insert " :case-fixed ")
    (prin1 (abbrev-get sym :case-fixed)))
  (when (abbrev-get sym :enable-function)
    (insert " :enable-function ")
    (prin1 (abbrev-get sym :enable-function)))
  (insert ")\n"))

(defun abbrev--describe (sym)
  "Describe abbrev SYM.
Print on `standard-output' the abbrev, count of use, expansion."
  (when (symbol-value sym)
    (prin1 (symbol-name sym))
    (if (null (abbrev-get sym :system))
        (indent-to 15 1)
      (insert " (sys)")
      (indent-to 20 1))
    (prin1 (abbrev-get sym :count))
    (indent-to 20 1)
    (prin1 (symbol-value sym))
    (when (symbol-function sym)
      (indent-to 45 1)
      (prin1 (symbol-function sym)))
    (terpri)))

(defun insert-abbrev-table-description (name &optional readable)
  "Insert before point a full description of abbrev table named NAME.
NAME is a symbol whose value is an abbrev table.
If optional 2nd arg READABLE is non-nil, insert a human-readable
description.

If READABLE is nil, insert an expression.  The expression is
a call to `define-abbrev-table' that, when evaluated, will define
the abbrev table NAME exactly as it is currently defined.
Abbrevs marked as \"system abbrevs\" are ignored."
  (let ((symbols (abbrev--table-symbols name readable)))
    (setq symbols (sort symbols #'string-lessp))
    (let ((standard-output (current-buffer)))
      (if readable
          (progn
            (insert "(")
            (prin1 name)
            (insert ")\n\n")
            (mapc #'abbrev--describe symbols)
            (insert "\n\n"))
        (insert "(define-abbrev-table '")
        (prin1 name)
        (if (null symbols)
            (insert " '())\n\n")
          (insert "\n  '(\n")
          (mapc #'abbrev--write symbols)
          (insert "   ))\n\n")))
      nil)))

(defun abbrev--table-symbols (name &optional system)
  "Return the user abbrev symbols in the abbrev table named NAME.
NAME is a symbol whose value is an abbrev table.  System abbrevs
are omitted unless SYSTEM is non-nil."
  (let ((table (symbol-value name))
        (symbols ()))
    (mapatoms (lambda (sym)
                (if (and (symbol-value sym) (or system (not (abbrev-get sym :system))))
                    (push sym symbols)))
              table)
    symbols))

(defun define-abbrev-table (tablename definitions
                                      &optional docstring &rest props)
  "Define TABLENAME (a symbol) as an abbrev table name.
Define abbrevs in it according to DEFINITIONS, which is a list of elements
of the form (ABBREVNAME EXPANSION ...) that are passed to `define-abbrev'.
PROPS is a property list to apply to the table.
Properties with special meaning:
- `:parents' contains a list of abbrev tables from which this table inherits
  abbreviations.
- `:case-fixed' non-nil means that abbreviations are looked up without
  case-folding, and the expansion is not capitalized/upcased.
- `:regexp' is a regular expression that specifies how to extract the
  name of the abbrev before point.  The submatch 1 is treated
  as the potential name of an abbrev.  If `:regexp' is nil, the default
  behavior uses `backward-word' and `forward-word' to extract the name
  of the abbrev, which can therefore by default only be a single word.
- `:enable-function' can be set to a function of no arguments which returns
  non-nil if and only if the abbrevs in this table should be used for this
  instance of `expand-abbrev'."
  (declare (doc-string 3) (indent defun))
  ;; We used to manually add the docstring, but we also want to record this
  ;; location as the definition of the variable (in load-history), so we may
  ;; as well just use `defvar'.
  (when (and docstring props (symbolp docstring))
    ;; There is really no docstring, instead the docstring arg
    ;; is a property name.
    (push docstring props) (setq docstring nil))
  (defvar-1 tablename nil docstring)
  (let ((table (if (boundp tablename) (symbol-value tablename))))
    (unless table
      (setq table (make-abbrev-table))
      (set tablename table)
      (unless (memq tablename abbrev-table-name-list)
        (push tablename abbrev-table-name-list)))
    ;; We used to just pass them to `make-abbrev-table', but that fails
    ;; if the table was pre-existing as is the case if it was created by
    ;; loading the user's abbrev file.
    (while (consp props)
      (unless (cdr props) (error "Missing value for property %S" (car props)))
      (abbrev-table-put table (pop props) (pop props)))
    (dolist (elt definitions)
      (apply #'define-abbrev table elt))))

;; GNU's lisp-mode.el / elisp-mode.el define these tables explicitly;
;; `define-derived-mode's auto-declaration leaves them bound either
;; way, but the :parents metadata below matches GNU's values.
(define-abbrev-table 'lisp-mode-abbrev-table ()
  "Abbrev table for Lisp mode.")

(define-abbrev-table 'emacs-lisp-mode-abbrev-table ()
  "Abbrev table for Emacs Lisp mode.
It has `lisp-mode-abbrev-table' as its parent."
  :parents (list lisp-mode-abbrev-table))

(defun abbrev-table-menu (table &optional prompt sortfun)
  "Return a menu that shows all abbrevs in TABLE.
Selecting an entry runs `abbrev-insert' for that entry's abbrev.
PROMPT is the prompt to use for the keymap.
SORTFUN is passed to `sort' to change the default ordering."
  (unless sortfun (setq sortfun 'string-lessp))
  (let ((entries ()))
    (obarray-map (lambda (abbrev)
                   (when (symbol-value abbrev)
                     (let ((name (symbol-name abbrev)))
                       (push `(,(intern name) menu-item ,name
                               (lambda () (interactive)
                                 (abbrev-insert ',abbrev)))
                             entries))))
                 table)
    (nconc (make-sparse-keymap prompt)
           (sort entries (lambda (x y)
                           (funcall sortfun (nth 2 x) (nth 2 y)))))))

(defface abbrev-table-name
  '((t :inherit font-lock-function-name-face))
  "Face used for displaying the abbrev table name in `edit-abbrevs-mode'."
  :version "29.1")

(defvar edit-abbrevs-mode-font-lock-keywords
  `(("^(\\(?:\\sw\\|\\s_\\|\\\\.\\)+)$" 0 'abbrev-table-name)))

;; Keep it after define-abbrev-table, since define-derived-mode uses
;; define-abbrev-table.
(define-derived-mode edit-abbrevs-mode fundamental-mode "Edit-Abbrevs"
  "Major mode for editing the list of abbrev definitions.
This mode is for editing abbrevs in a buffer prepared by `edit-abbrevs',
which see."
  :interactive nil
  (setq-local font-lock-defaults
              '(edit-abbrevs-mode-font-lock-keywords nil nil ((?_ . "w"))))
  (setq font-lock-multiline nil))

(defvar save-some-buffers-functions nil
  "Abnormal hook run by `save-some-buffers'.")

(defun abbrev--possibly-save (query &optional arg)
  "Hook function for use by `save-some-buffers-functions'.

Maybe save abbrevs, and record whether we either saved them or asked to."
  ;; Query mode.
  (if (eq query 'query)
      (and save-abbrevs abbrevs-changed)
    (and save-abbrevs
         abbrevs-changed
         (prog1
             (if (or arg
                     (eq save-abbrevs 'silently)
                     (y-or-n-p (format "Save abbrevs in %s? " abbrev-file-name)))
                 (progn
                   (write-abbrev-file nil)
                   nil)
               ;; Inhibit message in `save-some-buffers'.
               t)
           ;; Don't ask again whether saved or user said no.
           (setq abbrevs-changed nil)))))

(add-hook 'save-some-buffers-functions #'abbrev--possibly-save)

(defun tab-to-tab-stop ()
  "Insert spaces or tabs to next defined tab-stop column.
The variable `tab-stop-list' is a list of columns at which there are tab stops.
Use \\[edit-tab-stops] to edit them interactively.
Whether this inserts tabs or spaces depends on `indent-tabs-mode'."
  (interactive)
  (and abbrev-mode (= (char-syntax (preceding-char)) ?w)
       (expand-abbrev))
  (let ((nexttab (indent-next-tab-stop (current-column))))
    (delete-horizontal-space t)
    (indent-to nexttab)))

(defun move-to-tab-stop ()
  "Move point to next defined tab-stop column.
The variable `tab-stop-list' is a list of columns at which there are tab stops.
Use \\[edit-tab-stops] to edit them interactively."
  (interactive)
  (let ((nexttab (indent-next-tab-stop (current-column))))
    (let ((before (point)))
      (move-to-column nexttab t)
      (save-excursion
        (goto-char before)
        ;; If we just added a tab, or moved over one,
        ;; delete any superfluous spaces before the old point.
        (if (and (eq (preceding-char) ?\s)
                 (eq (following-char) ?\t))
            (let ((tabend (* (/ (current-column) tab-width) tab-width)))
              (while (and (> (current-column) tabend)
                          (eq (preceding-char) ?\s))
                (forward-char -1))
              (delete-region (point) before)))))))

(defun count-screen-lines (&optional beg end)
  "Count screen lines between BEG and END (batch: count-lines)."
  (count-lines (or beg (point-min)) (or end (point-max))))

;; window.el cluster (verbatim GNU where possible).

(defmacro with-selected-window (window &rest body)
  "Execute BODY within WINDOW, then restore the previously selected window."
  (declare (indent 1) (debug t))
  `(let ((with-selected-window--old (selected-window))
         (with-selected-window--win ,window))
     (unwind-protect
         (progn
           (select-window with-selected-window--win)
           ,@body)
       (select-window with-selected-window--old))))

(defun beginning-of-buffer-other-window (arg)
  "Move point to the beginning of the buffer in the other window.
Leave mark at previous position.
With arg N, put point N/10 of the way from the true beginning."
  (interactive "P")
  (with-selected-window (other-window-for-scrolling)
    ;; Set point and mark in that window's buffer.
    (with-no-warnings
      (beginning-of-buffer arg))
    ;; Set point accordingly.
    (recenter '(t))))

(defun end-of-buffer-other-window (arg)
  "Move point to the end of the buffer in the other window.
Leave mark at previous position.
With arg N, put point N/10 of the way from the true end."
  (interactive "P")
  ;; See beginning-of-buffer-other-window for comments.
  (with-selected-window (other-window-for-scrolling)
    (with-no-warnings
      (end-of-buffer arg))
    (recenter '(t))))

;; disp-table.el cluster (verbatim GNU).
;;
;; GNU sizes a char-table's extra slots from the subtype's
;; `char-table-extra-slots' property, read by `make-char-table' at
;; creation.  The C-level inits give case-table 3, category-table 2,
;; char-code-property-table 5, syntax-table and
;; keyboard-translate-table 0; disp-table.el puts 18 on display-table.

(put 'case-table 'char-table-extra-slots 3)
(put 'category-table 'char-table-extra-slots 2)
(put 'char-code-property-table 'char-table-extra-slots 5)
(put 'syntax-table 'char-table-extra-slots 0)
(put 'keyboard-translate-table 'char-table-extra-slots 0)
(put 'display-table 'char-table-extra-slots 18)

(defun make-display-table ()
  "Return a new, empty display table."
  (make-char-table 'display-table nil))

(or standard-display-table
    (setq standard-display-table (make-display-table)))

;;; Display-table slot names.  The property value says which slot.

(put 'truncation 'display-table-slot 0)
(put 'wrap 'display-table-slot 1)
(put 'escape 'display-table-slot 2)
(put 'control 'display-table-slot 3)
(put 'selective-display 'display-table-slot 4)
(put 'vertical-border 'display-table-slot 5)

(put 'box-vertical 'display-table-slot 6)
(put 'box-horizontal 'display-table-slot 7)
(put 'box-down-right 'display-table-slot 8)
(put 'box-down-left 'display-table-slot 9)
(put 'box-up-right 'display-table-slot 10)
(put 'box-up-left 'display-table-slot 11)

(put 'box-double-vertical 'display-table-slot 12)
(put 'box-double-horizontal 'display-table-slot 13)
(put 'box-double-down-right 'display-table-slot 14)
(put 'box-double-down-left 'display-table-slot 15)
(put 'box-double-up-right 'display-table-slot 16)
(put 'box-double-up-left 'display-table-slot 17)

(defun display-table-slot (display-table slot)
  "Return the value of the extra slot in DISPLAY-TABLE named SLOT.
SLOT may be a number from 0 to 17 inclusive, or a slot name (symbol)."
  (let ((slot-number
	 (if (numberp slot) slot
	   (or (get slot 'display-table-slot)
	       (error "Invalid display-table slot name: %s" slot)))))
    (char-table-extra-slot display-table slot-number)))

(defun set-display-table-slot (display-table slot value)
  "Set the value of the extra slot in DISPLAY-TABLE named SLOT to VALUE."
  (let ((slot-number
	 (if (numberp slot) slot
	   (or (get slot 'display-table-slot)
	       (error "Invalid display-table slot name: %s" slot)))))
    (set-char-table-extra-slot display-table slot-number value)))

;; window.el splitting cluster (verbatim GNU, plus a flat
;; walk-window-tree approximation).

(defun window-splittable-p (window &optional horizontal)
  "Return non-nil if `split-window-sensibly' may split WINDOW."
  (when (and (window-live-p window)
             (not (window-parameter window 'window-side)))
    (with-current-buffer (window-buffer window)
      (if horizontal
	  (and (memq window-size-fixed '(nil height))
	       (numberp split-width-threshold)
	       (>= (window-width window)
		   (max split-width-threshold
			(* 2 (max window-min-width 2)))))
	(and (memq window-size-fixed '(nil width))
	     (numberp split-height-threshold)
	     (>= (window-height window)
		 (max split-height-threshold
		      (* 2 (max window-min-height
				(if mode-line-format 2 1))))))))))

(defun window--try-vertical-split (window)
  "Helper function for `split-window-sensibly'."
  (when (window-splittable-p window)
    (split-window window nil 'below)))

(defun window--try-horizontal-split (window)
  "Helper function for `split-window-sensibly'."
  (when (window-splittable-p window t)
    (split-window window nil 'right)))

(defun window--frame-landscape-p (&optional frame)
  "Non-nil if FRAME is wider than it is tall."
  (if (display-graphic-p frame)
      (> (frame-pixel-width frame) (frame-pixel-height frame))
    ;; On a terminal, displayed characters are usually roughly twice as
    ;; tall as they are wide.
    (> (frame-width frame) (* 2 (frame-height frame)))))

(defun walk-window-tree (fun &optional frame _any-window nomini _all-frames)
  "Call FUN on each live window of FRAME (flat model: `window-list')."
  (dolist (window (window-list frame (if nomini nil t)))
    (funcall fun window)))

(defun split-window-sensibly (&optional window)
  "Split WINDOW in a way suitable for `display-buffer'."
  (let ((window (or window (selected-window))))
    (or (if (or
             (eql split-window-preferred-direction 'horizontal)
             (and (eql split-window-preferred-direction 'longest)
                  (window--frame-landscape-p (window-frame window))))
            (or (window--try-horizontal-split window)
                (window--try-vertical-split window))
          (or (window--try-vertical-split window)
              (window--try-horizontal-split window)))
	(and
         ;; If WINDOW is the only usable window on its frame (it is
         ;; the only one or, not being the only one, all the other
         ;; ones are dedicated) and is not the minibuffer window, try
         ;; to split it vertically disregarding the value of
         ;; `split-height-threshold'.
         (let ((frame (window-frame window)))
           (or
            (eq window (frame-root-window frame))
            (catch 'done
              (walk-window-tree (lambda (w)
                                  (unless (or (eq w window)
                                              (window-dedicated-p w))
                                    (throw 'done nil)))
                                frame nil 'nomini)
              t)))
	 (not (window-minibuffer-p window))
         (let ((split-height-threshold 0))
           (window--try-vertical-split window))))))

(defun reindent-then-newline-and-indent ()
  "Reindent current line, insert newline, then indent that line."
  (interactive "*")
  (indent-according-to-mode)
  (newline)
  (indent-according-to-mode))

(defun indent-new-comment-line (&optional soft)
  "Break line at point and indent, continuing a comment if any."
  (interactive "*")
  (newline-and-indent))

(defun indent-pp-sexp (&optional arg)
  "Indent each line of the list starting just after point."
  (interactive "*P")
  (let ((end (save-excursion (forward-sexp (or arg 1)) (point))))
    (when end (indent-region (point) end))))

;; ---------- balanced-expression navigation (GNU lisp.el) ----------

(defun buffer-end (arg)
  "Return `point-max' if ARG is positive, `point-min' otherwise."
  (if (> arg 0) (point-max) (point-min)))

(defsubst ppss-comment-or-string-start (state)
  "Return the start position of the innermost string/comment in STATE."
  (nth 8 state))

(defun forward-sexp-default-function (&optional arg)
  "Default function for `forward-sexp-function'."
  (goto-char (or (scan-sexps (point) arg) (buffer-end arg)))
  (if (< arg 0) (backward-prefix-chars)))

(defvar forward-sexp-function nil
  "If non-nil, `forward-sexp' delegates to this function.
Should take the same arguments and behave similarly to `forward-sexp'.")

(defun forward-sexp (&optional arg interactive)
  "Move forward across one balanced expression (sexp).
With ARG, do it that many times.  Negative arg -N means move
backward across N balanced expressions.  This command assumes
point is not in a string or comment.  Calls
`forward-sexp-function' to do the work, if that is non-nil.
If unable to move over a sexp, signal `scan-error' with three
arguments: a message, the start of the obstacle (usually a
parenthesis or list marker of some kind), and end of the
obstacle.  If INTERACTIVE is non-nil, as it is interactively,
report errors as appropriate for this kind of usage."
  (interactive "^p\nd")
  (if interactive
      (condition-case nil
          (forward-sexp arg nil)
        (scan-error (user-error (if (> arg 0)
                                    "No next sexp"
                                  "No previous sexp"))))
    (or arg (setq arg 1))
    (if forward-sexp-function
        (funcall forward-sexp-function arg)
      (forward-sexp-default-function arg))))

(defun backward-sexp (&optional arg interactive)
  "Move backward across one balanced expression (sexp).
With ARG, do it that many times.  Negative arg -N means
move forward across N balanced expressions.
This command assumes point is not in a string or comment.
Uses `forward-sexp' to do the work.
If INTERACTIVE is non-nil, as it is interactively,
report errors as appropriate for this kind of usage."
  (interactive "^p\nd")
  (or arg (setq arg 1))
  (forward-sexp (- arg) interactive))

(defun forward-list (&optional arg interactive)
  "Move forward across one balanced group of parentheses.
This command will also work on other parentheses-like expressions
defined by the current language mode.
With ARG, do it that many times.
Negative arg -N means move backward across N groups of parentheses.
This command assumes point is not in a string or comment.
If INTERACTIVE is non-nil, as it is interactively,
report errors as appropriate for this kind of usage."
  (interactive "^p\nd")
  (if interactive
      (condition-case nil
          (forward-list arg nil)
        (scan-error (user-error (if (> arg 0)
                                    "No next group"
                                  "No previous group"))))
    (or arg (setq arg 1))
    (goto-char (or (scan-lists (point) arg 0) (buffer-end arg)))))

(defun backward-list (&optional arg interactive)
  "Move backward across one balanced group of parentheses.
This command will also work on other parentheses-like expressions
defined by the current language mode.
With ARG, do it that many times.
Negative arg -N means move forward across N groups of parentheses.
This command assumes point is not in a string or comment.
If INTERACTIVE is non-nil, as it is interactively,
report errors as appropriate for this kind of usage."
  (interactive "^p\nd")
  (or arg (setq arg 1))
  (forward-list (- arg) interactive))

(defun down-list (&optional arg interactive)
  "Move forward down one level of parentheses.
This command will also work on other parentheses-like expressions
defined by the current language mode.
With ARG, do this that many times.
A negative argument means move backward but still go down a level.
This command assumes point is not in a string or comment.
If INTERACTIVE is non-nil, as it is interactively,
report errors as appropriate for this kind of usage."
  (interactive "^p\nd")
  (when (ppss-comment-or-string-start (syntax-ppss))
    (user-error "This command doesn't work in strings or comments"))
  (if interactive
      (condition-case nil
          (down-list arg nil)
        (scan-error (user-error "At bottom level")))
    (or arg (setq arg 1))
    (let ((inc (if (> arg 0) 1 -1)))
      (while (/= arg 0)
        (goto-char (or (scan-lists (point) inc -1) (buffer-end arg)))
        (setq arg (- arg inc))))))

(defun backward-up-list (&optional arg escape-strings no-syntax-crossing)
  "Move backward out of one level of parentheses.
This command will also work on other parentheses-like expressions
defined by the current language mode.  With ARG, do this that
many times.  A negative argument means move forward but still to
a less deep spot.

If ESCAPE-STRINGS is non-nil (as it is interactively), move out
of enclosing strings as well.

If NO-SYNTAX-CROSSING is non-nil (as it is interactively), prefer
to break out of any enclosing string instead of moving to the
start of a list broken across multiple strings.

On error, location of point is unspecified."
  (interactive "^p\nd\nd")
  (up-list (- (or arg 1)) escape-strings no-syntax-crossing))

(defun up-list (&optional arg escape-strings no-syntax-crossing)
  "Move forward out of one level of parentheses.
This command will also work on other parentheses-like expressions
defined by the current language mode.  With ARG, do this that
many times.  A negative argument means move backward but still to
a less deep spot.

If ESCAPE-STRINGS is non-nil (as it is interactively), move out
of enclosing strings as well.

If NO-SYNTAX-CROSSING is non-nil (as it is interactively), prefer
to break out of any enclosing string instead of moving to the
end of a list broken across multiple strings.

On error, location of point is unspecified."
  (interactive "^p\nd\nd")
  (or arg (setq arg 1))
  (let ((inc (if (> arg 0) 1 -1))
        (pos nil))
    (while (/= arg 0)
      (condition-case err
          (save-restriction
            ;; If we've been asked not to cross string boundaries
            ;; and we're inside a string, narrow to that string so
            ;; that scan-lists doesn't find a match in a different
            ;; string.
            (when no-syntax-crossing
              (let* ((syntax (syntax-ppss))
                     (string-comment-start (nth 8 syntax)))
                (when string-comment-start
                  (save-excursion
                    (goto-char string-comment-start)
                    (narrow-to-region
                     (point)
                     (if (nth 3 syntax) ; in string
                         (condition-case nil
                             (progn (forward-sexp) (point))
                           (scan-error (point-max)))
                       (forward-comment 1)
                       (point)))))))
            (if (null forward-sexp-function)
                (goto-char (or (scan-lists (point) inc 1)
                               (buffer-end arg)))
              (condition-case err
                  (while (progn (setq pos (point))
                                (forward-sexp inc)
                                (/= (point) pos)))
                (scan-error (goto-char (nth (if (> arg 0) 3 2) err))))
              (if (= (point) pos)
                  (signal 'scan-error
                          (list "Unbalanced parentheses" (point) (point))))))
        (scan-error
         (let ((syntax nil))
           (or
            ;; If we bumped up against the end of a list, see whether
            ;; we're inside a string: if so, just go to the beginning
            ;; or end of that string.
            (and escape-strings
                 (or syntax (setq syntax (syntax-ppss)))
                 (nth 3 syntax)
                 (goto-char (nth 8 syntax))
                 (progn (when (> inc 0)
                          (forward-sexp))
                        t))
            ;; If we narrowed to a comment above and failed to escape
            ;; it, the error might be our fault, not an indication
            ;; that we're out of syntax.  Try again from beginning or
            ;; end of the comment.
            (and no-syntax-crossing
                 (or syntax (setq syntax (syntax-ppss)))
                 (nth 4 syntax)
                 (goto-char (nth 8 syntax))
                 (or (< inc 0)
                     (forward-comment 1))
                 (setq arg (+ arg inc)))
            (if no-syntax-crossing
                ;; Assume called interactively; don't signal an error.
                (user-error "At top level")
              (signal (car err) (cdr err)))))))
      (setq arg (- arg inc)))))

;; ---------- files.el: auto-mode & local variables (GNU port) ----------
;; GNU marks these buffer-locals permanent (buffer.c); preserved by
;; kill-all-local-variables.
(put 'buffer-file-name 'permanent-local t)
(put 'buffer-file-truename 'permanent-local t)
(put 'default-directory 'permanent-local t)
(put 'buffer-auto-save-file-name 'permanent-local t)
(put 'buffer-saved-size 'permanent-local t)
(put 'buffer-backed-up 'permanent-local t)

;; minor convenience feature for handling of an obsolete Rmail file format.
(defun define-obsolete-function-alias (obsolete-name current-name
                                                     &optional when docstring)
  "Make OBSOLETE-NAME a (function) alias for CURRENT-NAME."
  (defalias obsolete-name current-name docstring)
  (put obsolete-name 'byte-obsolete-function t)
  obsolete-name)

(defun provided-mode-derived-p (mode &rest modes)
  "Non-nil if MODE is a major mode derived from one of MODES."
  (let ((m mode)
        (found nil))
    (while (and m (not found))
      (if (memq m modes)
          (setq found t)
        (setq m (get m 'derived-mode-parent))))
    found))

(defvar file-name-version-regexp
  "\\(?:~\\|\\.~[-[:alnum:]:#@^._]+\\(?:~[[:digit:]]+\\)?~\\)"
  ;; The last ~[[:digit:]]+ matches relative versions in git,
  ;; e.g. `foo.js.~HEAD~1~'.
  "Regular expression matching the backup/version part of a file name.
Used by `file-name-sans-versions'.")

(defun file-name-sans-versions (name &optional keep-backup-version)
  "Return file NAME sans backup versions or strings.
This is a separate procedure so your site-init or startup file can
redefine it.
If the optional argument KEEP-BACKUP-VERSION is non-nil,
we do not remove backup version numbers, only true file version numbers.
See `file-name-version-regexp' for what constitutes backup versions
and version strings."
  (let ((handler (find-file-name-handler name 'file-name-sans-versions)))
    (if handler
	(funcall handler 'file-name-sans-versions name keep-backup-version)
      (substring name 0
		 (unless keep-backup-version
                   (string-match (concat file-name-version-regexp "\\'")
                                 name))))))

;; GNU C var (indent.c); consulted by hack-local-variables--find-variables.
(defvar selective-display nil)

;; filelock.c.
(defvar create-lockfiles t
  "Non-nil means use lockfiles to avoid editing clashes.")

;; minibuf.c DEFVAR_LISP.
(defvar file-name-history nil
  "History list of file names read in minibuffer.")

;; files.el: directory copying (verbatim GNU).
(defcustom copy-directory-create-symlink nil
  "Whether copying a directory creates a symbolic link when the source
directory is a symlink."
  :type 'boolean)

(defconst directory-files-no-dot-files-regexp
  "[^.]\\|\\.\\.\\."
  "Regexp matching any file name except \".\" and \"..\".
More precisely, it matches parts of any nonempty string except those two.
It is useful as the regexp argument to `directory-files' and
`directory-files-and-attributes'.")

(defun copy-directory (directory newname &optional keep-time parents
                                 copy-contents)
  "Copy DIRECTORY to NEWNAME.  Both args must be strings.
This function always sets the file modes of the output files to match
the corresponding input file.

The third arg KEEP-TIME non-nil means give the output files the same
last-modified time as the old ones.  (This works on only some systems.)

A prefix arg makes KEEP-TIME non-nil.

Noninteractively, the PARENTS argument says whether to create
parent directories if they don't exist.  Interactively, this
happens by default.

If DIRECTORY is a symlink and `copy-directory-create-symlink' is
non-nil, create a symlink with the same target as DIRECTORY.

If NEWNAME is a directory name, copy DIRECTORY as a subdirectory
there.  However, if called from Lisp with a non-nil optional
argument COPY-CONTENTS, copy the contents of DIRECTORY directly
into NEWNAME instead."
  (interactive
   (let ((dir (read-directory-name
	       "Copy directory: " default-directory default-directory t nil)))
     (list dir
	   (read-directory-name
	    (format "Copy directory %s to: " dir)
	    default-directory default-directory nil nil)
	   current-prefix-arg t nil)))
  (when (file-in-directory-p newname directory)
    (error "Cannot copy `%s' into its subdirectory `%s'"
           directory newname))
  ;; If default-directory is a remote directory, make sure we find its
  ;; copy-directory handler.
  (let ((handler (or (find-file-name-handler directory 'copy-directory)
		     (find-file-name-handler newname 'copy-directory)))
	follow)
    (if handler
	(funcall handler 'copy-directory directory
                 newname keep-time parents copy-contents)

      ;; Compute target name.
      (setq directory (directory-file-name (expand-file-name directory))
	    newname (expand-file-name newname))

      ;; GNU signals before creating anything when NEWNAME is inside
      ;; DIRECTORY itself.
      (when (string-prefix-p (file-name-as-directory (file-truename directory))
			     (file-name-as-directory (file-truename newname)))
	(error "Cannot copy ‘%s’ into its subdirectory ‘%s’"
	       directory newname))

      ;; If DIRECTORY is a symlink, create a symlink with the same target.
      (if (and (file-symlink-p directory)
               copy-directory-create-symlink)
          (let ((target (car (file-attributes directory))))
	    (if (directory-name-p newname)
		(make-symbolic-link target
				    (concat newname
					    (file-name-nondirectory directory))
				    t)
	      (make-symbolic-link target newname t)))
        ;; Else proceed to copy as a regular directory
	;; first by creating the destination directory if needed,
	;; preparing to follow any symlink to a directory we did not create.
	(setq follow
	    (if (not (directory-name-p newname))
	       ;; If NEWNAME is not a directory name, create it;
	       ;; that is where we will copy the files of DIRECTORY.
	       (make-directory newname parents)
	      ;; NEWNAME is a directory name.  If COPY-CONTENTS is non-nil,
	      ;; create NEWNAME if it is not already a directory;
	      ;; otherwise, create NEWNAME/[DIRECTORY-BASENAME].
	      (unless copy-contents
	         (setq newname (concat newname
				       (file-name-nondirectory directory))))
	      (condition-case err
		  (make-directory (directory-file-name newname) parents)
		(error
		 (or (file-directory-p newname)
		     (signal (car err) (cdr err)))))))

        ;; Copy recursively.
        (dolist (file
	         ;; We do not want to copy "." and "..".
	         (directory-files directory 'full
				  directory-files-no-dot-files-regexp))
	  (let ((target (concat (file-name-as-directory newname)
			        (file-name-nondirectory file)))
	        (filetype (car (file-attributes file))))
	    (cond
	     ((eq filetype t)           ; Directory but not a symlink.
	      (copy-directory file target keep-time parents t))
	     ((stringp filetype)        ; Symbolic link
	      (make-symbolic-link filetype target t))
	     ((copy-file file target t keep-time)))))

        ;; Set directory attributes.
        (let ((modes (file-modes directory))
	      (times (and keep-time (file-attribute-modification-time
				     (file-attributes directory))))
	      (follow-flag (unless follow 'nofollow)))
	  (if modes (set-file-modes newname modes follow-flag))
	  (when times
            (set-file-times newname times follow-flag)))))))

;; coding.c: return a coding system like CS with the given eol type.
;; Our coding systems are BASE-EOL symbols (utf-8-unix), as in GNU names.
(defun coding-system-change-eol-conversion (coding-system eol-type)
  "Return a coding system like CODING-SYSTEM but with EOL-TYPE.
EOL-TYPE is `unix', `dos', `mac', or 0, 1, 2 respectively."
  (let ((eol (cond ((memq eol-type '(unix 0)) "unix")
                   ((memq eol-type '(dos 1)) "dos")
                   ((memq eol-type '(mac 2)) "mac")
                   (t "unix")))
        (base (coding-system-base coding-system)))
    (intern (concat (symbol-name base) "-" eol))))

;; files.el: insert-directory cluster (verbatim GNU).
;; files.el 1287.
(defun executable-find (command &optional remote)
  "Search for COMMAND in `exec-path' and return the absolute file name.
Return nil if COMMAND is not found anywhere in `exec-path'.
If REMOTE is non-nil, search on a remote host if `default-directory' is
remote, otherwise search locally."
  (if (and remote (file-remote-p default-directory))
      (let ((res (locate-file
	          command
	          (mapcar
	           (lambda (x) (concat (file-remote-p default-directory) x))
	           (exec-path))
	          exec-suffixes 'file-executable-p)))
        (when (stringp res) (file-local-name res)))
    ;; Use 1 rather than file-executable-p to better match the
    ;; behavior of call-process.
    (let ((default-directory (file-name-quote default-directory 'top)))
      (locate-file command exec-path exec-suffixes 1))))


(defun shell-quote-wildcard-pattern (pattern)
  "Quote characters special to the shell in PATTERN, leave wildcards alone.

PATTERN is assumed to represent a file-name wildcard suitable for the
underlying filesystem.  For Unix and GNU/Linux, each character from the
set [ \\t\\n;<>&|()\\=`\\='\"#$] is quoted with a backslash; for DOS/Windows, all
the parts of the pattern that don't include wildcard characters are
quoted with double quotes.

This function leaves alone existing quote characters (\\ on Unix and \"
on Windows), so PATTERN can use them to quote wildcard characters that
need to be passed verbatim to shell commands."
  (save-match-data
    (cond
     ((memq system-type '(ms-dos windows-nt cygwin))
      ;; DOS/Windows don't allow `"' in file names.  So if the
      ;; argument has quotes, we can safely assume it is already
      ;; quoted by the caller.
      (if (or (string-search "\"" pattern)
	      ;; We quote [&()#$`'] in case their shell is a port of a
	      ;; Unixy shell.  We quote [,=+] because stock DOS and
	      ;; Windows shells require that in some cases, such as
	      ;; passing arguments to batch files that use positional
	      ;; arguments like %1.
	      (not (string-match "[ \t;&()#$`',=+]" pattern)))
	  pattern
	(let ((result "\"")
	      (beg 0)
	      end)
	  (while (string-match "[*?]+" pattern beg)
	    (setq end (match-beginning 0)
		  result (concat result (substring pattern beg end)
				 "\""
				 (substring pattern end (match-end 0))
				 "\"")
		  beg (match-end 0)))
	  (concat result (substring pattern beg) "\""))))
     (t
      (let ((beg 0))
	(while (string-match "[ \t\n;<>&|()`'\"#$]" pattern beg)
	  (setq pattern
		(concat (substring pattern 0 (match-beginning 0))
			"\\"
			(substring pattern (match-beginning 0)))
		beg (1+ (match-end 0)))))
      pattern))))


(defcustom insert-directory-program
  (if (and (memq system-type '(berkeley-unix darwin))
           (executable-find "gls"))
      (purecopy "gls")
    (purecopy "ls"))
  "Absolute or relative name of the `ls'-like program.
This is used by `insert-directory' and `dired-insert-directory'
\(thus, also by `dired').  For Dired, this should ideally point to
GNU ls, or another version of ls that supports the \"--dired\"
flag.  See `dired-use-ls-dired'.

On GNU/Linux and other capable systems, the default is \"ls\".

On *BSD and macOS systems, the default \"ls\" does not support
the \"--dired\" flag.  Therefore, the default is to use the
\"gls\" executable on such machines, if it exists.  This means
that there should normally be no need to customize this when
installing GNU coreutils using something like ports or Homebrew."
  :group 'dired
  :type 'string
  :initialize #'custom-initialize-delay
  :version "30.1")

(defun files--use-insert-directory-program-p ()
  "Return non-nil if we should use `insert-directory-program'.
Return nil if we should prefer `ls-lisp' instead."
  ;; FIXME: Should we also check `file-accessible-directory-p' so we
  ;; automatically redirect to ls-lisp when operating on magic file names?
  (and (if (boundp 'ls-lisp-use-insert-directory-program)
           ls-lisp-use-insert-directory-program
         t)
       insert-directory-program))


(defun get-free-disk-space (dir)
  "String describing the amount of free space on DIR's file system.
If DIR's free space cannot be obtained, this function returns nil."
  (save-match-data
    (let ((avail (nth 2 (file-system-info dir))))
      (if avail
          (funcall byte-count-to-string-function avail)))))

(defvar directory-listing-before-filename-regexp
  (let* ((l "\\([A-Za-z]\\|[^\0-\177]\\)")
	 (l-or-quote "\\([A-Za-z']\\|[^\0-\177]\\)")
	 ;; In some locales, month abbreviations are as short as 2 letters,
	 ;; and they can be followed by ".".
	 ;; In Breton, a month name  can include a quote character.
	 (month (concat l-or-quote l-or-quote "+\\.?"))
	 (s " ")
	 (yyyy "[0-9][0-9][0-9][0-9]")
	 (dd "[ 0-3][0-9]")
	 (HH:MM "[ 0-2][0-9][:.][0-5][0-9]")
	 (seconds "[0-6][0-9]\\([.,][0-9]+\\)?")
	 (zone "[-+][0-2][0-9][0-5][0-9]")
	 (iso-mm-dd "[01][0-9]-[0-3][0-9]")
	 (iso-time (concat HH:MM "\\(:" seconds "\\( ?" zone "\\)?\\)?"))
	 (iso (concat "\\(\\(" yyyy "-\\)?" iso-mm-dd "[ T]" iso-time
		      "\\|" yyyy "-" iso-mm-dd "\\)"))
	 (western (concat "\\(" month s "+" dd "\\|" dd "\\.?" s month "\\)"
			  s "+"
			  "\\(" HH:MM "\\|" yyyy "\\)"))
	 (western-comma (concat month s "+" dd "," s "+" yyyy))
         ;; This represents the date in strftime(3) format "%e-%b-%Y"
         ;; (aka "%v"), as it is the default for many ls incarnations.
         (DD-MMM-YYYY (concat dd "-" month "-" yyyy s HH:MM))
	 ;; Japanese MS-Windows ls-lisp has one-digit months, and
	 ;; omits the Kanji characters after month and day-of-month.
	 ;; On Mac OS X 10.3, the date format in East Asian locales is
	 ;; day-of-month digits followed by month digits.
	 (mm "[ 0-1]?[0-9]")
	 (east-asian
	  (concat "\\(" mm l "?" s dd l "?" s "+"
		  "\\|" dd s mm s "+" "\\)"
		  "\\(" HH:MM "\\|" yyyy l "?" "\\)")))
	 ;; The "[0-9]" below requires the previous column to end in a digit.
	 ;; This avoids recognizing `1 may 1997' as a date in the line:
	 ;; -r--r--r--   1 may      1997        1168 Oct 19 16:49 README

	 ;; The "[BkKMGTPEZYRQ]?" below supports "ls -alh" output.

	 ;; For non-iso date formats, we add the ".*" in order to find
	 ;; the last possible match.  This avoids recognizing
	 ;; `jservice 10 1024' as a date in the line:
	 ;; drwxr-xr-x  3 jservice  10  1024 Jul  2  1997 esg-host

         ;; vc dired listings provide the state or blanks between file
         ;; permissions and date.  The state is always surrounded by
         ;; parentheses:
         ;; -rw-r--r-- (modified) 2005-10-22 21:25 files.el
         ;; This is not supported yet.
    (purecopy (concat "\\([0-9][BkKMGTPEZYRQ]? " iso
		      "\\|.*[0-9][BkKMGTPEZYRQ]? "
	              "\\(" western "\\|" western-comma
                      "\\|" DD-MMM-YYYY "\\|" east-asian "\\)"
		      "\\) +")))
  "Regular expression to match up to the file name in a directory listing.
The default value is designed to recognize dates and times
regardless of the language.")

(defvar insert-directory-ls-version 'unknown)

(defun insert-directory-wildcard-in-dir-p (dir)
  "Return non-nil if DIR contains shell wildcards in its parent directory part.
The return value is a cons (DIRECTORY . WILDCARD), where DIRECTORY is the
part of DIR up to and excluding the first component that includes
wildcard characters, and WILDCARD is the rest of DIR's components.  The
DIRECTORY part of the value includes the trailing slash, to indicate that
it is a directory.

Valid wildcards are `*', `?', `[abc]' and `[a-z]'."
  (let ((wildcards "[?*"))
    (when (and (or (not (featurep 'ls-lisp))
                   ls-lisp-support-shell-wildcards)
               (string-match (concat "[" wildcards "]") (file-name-directory dir))
               (not (file-exists-p dir))) ; Prefer an existing file to wildcards.
      (let ((regexp (format "\\`\\([^%s]*/\\)\\([^%s]*[%s].*\\)"
                            wildcards wildcards wildcards)))
        (string-match regexp dir)
        (cons (match-string 1 dir) (match-string 2 dir))))))

(defun insert-directory-clean (beg switches)
  (when (if (stringp switches)
	    (string-match "--dired\\>" switches)
	  (member "--dired" switches))
    ;; The following overshoots by one line for an empty
    ;; directory listed with "--dired", but without "-a"
    ;; switch, where the ls output contains a
    ;; "//DIRED-OPTIONS//" line, but no "//DIRED//" line.
    ;; We take care of that case later.
    (forward-line -2)
    ;; We reset case-fold-search here and elsewhere, because
    ;; case-insensitive search for strings with uppercase 'I' will fail
    ;; in language environments (such as Turkish) where 'I' downcases to
    ;; 'ı', not to 'i'.
    (when (let ((case-fold-search nil)) (looking-at "//SUBDIRED//"))
      (delete-region (point) (progn (forward-line 1) (point)))
      (forward-line -1))
    (if (let ((case-fold-search nil)) (looking-at "//DIRED//"))
	(let ((end (line-end-position))
	      (linebeg (point))
	      error-lines)
	  ;; Find all the lines that are error messages,
	  ;; and record the bounds of each one.
	  (goto-char beg)
	  (while (< (point) linebeg)
	    (or (eql (following-char) ?\s)
		(push (list (point) (line-end-position)) error-lines))
	    (forward-line 1))
	  (setq error-lines (nreverse error-lines))
	  ;; Now read the numeric positions of file names.
	  (goto-char linebeg)
	  (forward-word-strictly 1)
	  (forward-char 3)
	  (while (< (point) end)
	    (let ((start (insert-directory-adj-pos
			  (+ beg (read (current-buffer)))
			  error-lines))
		  (end (insert-directory-adj-pos
			(+ beg (read (current-buffer)))
			error-lines)))
	      (if (memq (char-after end) '(?\n ?\s ?/ ?* ?@ ?% ?= ?|))
		  ;; End is followed by \n or by output of -F.
		  (put-text-property start end 'dired-filename t)
		;; It seems that we can't trust ls's output as to
		;; byte positions of filenames.
		(put-text-property beg (point) 'dired-filename nil)
		(end-of-line))))
	  (goto-char end)
	  (beginning-of-line)
	  (delete-region (point) (progn (forward-line 1) (point))))
      ;; Take care of the case where the ls output contains a
      ;; "//DIRED-OPTIONS//"-line, but no "//DIRED//"-line
      ;; and we went one line too far back (see above).
      (forward-line 1))
    (if (let ((case-fold-search nil)) (looking-at "//DIRED-OPTIONS//"))
	(delete-region (point) (progn (forward-line 1) (point))))))

;; insert-directory
;; - must insert _exactly_one_line_ describing FILE if WILDCARD and
;;   FULL-DIRECTORY-P is nil.
;;   The single line of output must display FILE's name as it was
;;   given, namely, an absolute path name.
;; - must insert exactly one line for each file if WILDCARD or
;;   FULL-DIRECTORY-P is t, plus one optional "total" line
;;   before the file lines, plus optional text after the file lines.
;;   Lines are delimited by "\n", so filenames containing "\n" are not
;;   allowed.
;;   File lines should display the basename.
;; - must be consistent with
;;   - functions dired-move-to-filename, (these two define what a file line is)
;;   		 dired-move-to-end-of-filename,
;;		 dired-between-files, (shortcut for (not (dired-move-to-filename)))
;;   		 dired-insert-headerline
;;   		 dired-after-subdir-garbage (defines what a "total" line is)
;;   - variable dired-subdir-regexp
;; - may be passed "--dired" as the first argument in SWITCHES.
;;   File name handlers might have to remove this switch if their
;;   "ls" command does not support it.
(defun insert-directory (file switches &optional wildcard full-directory-p)
  "Insert directory listing for FILE, formatted according to SWITCHES.
Leaves point after the inserted text.
SWITCHES may be a string of options, or a list of strings
representing individual options.
Optional third arg WILDCARD means treat FILE as shell wildcard.
Optional fourth arg FULL-DIRECTORY-P means file is a directory and
switches do not contain `d', so that a full listing is expected.

Depending on the value of `ls-lisp-use-insert-directory-program'
this works either using a Lisp emulation of the \"ls\" program
or by running a directory listing program
whose name is in the variable `insert-directory-program'
\(and if WILDCARD, it also runs the shell specified by `shell-file-name').

When SWITCHES contains the long `--dired' option, this function
treats it specially, for the sake of dired.  However, the
normally equivalent short `-D' option is just passed on to
`insert-directory-program', as any other option."
  ;; We need the directory in order to find the right handler.
  (let ((handler (find-file-name-handler (expand-file-name file)
					 'insert-directory)))
    (cond
     (handler
      (funcall handler 'insert-directory file switches
	       wildcard full-directory-p))
     ((not (files--use-insert-directory-program-p))
      (require 'ls-lisp)
      (declare-function ls-lisp--insert-directory "ls-lisp")
      (ls-lisp--insert-directory file switches wildcard full-directory-p))
     (t
      (let (result (beg (point)))

	;; Read the actual directory using `insert-directory-program'.
	;; RESULT gets the status code.
	(let* (;; We at first read by no-conversion, then after
	       ;; putting text property `dired-filename, decode one
	       ;; bunch by one to preserve that property.
	       (coding-system-for-read 'no-conversion)
	       ;; This is to control encoding the arguments in call-process.
	       (coding-system-for-write
		(and enable-multibyte-characters
		     (or file-name-coding-system
			 default-file-name-coding-system))))
	  (setq result
		(if wildcard
		    ;; If the wildcard is just in the file part, then run ls in
                    ;; the directory part of the file pattern using the last
                    ;; component as argument.  Otherwise, run ls in the longest
                    ;; subdirectory of the directory part free of wildcards; use
                    ;; the remaining of the file pattern as argument.
		    (let* ((dir-wildcard (insert-directory-wildcard-in-dir-p file))
                           (default-directory
                            (cond (dir-wildcard (car dir-wildcard))
                                  (t
			           (if (file-name-absolute-p file)
				       (file-name-directory file)
				     (file-name-directory (expand-file-name file))))))
			   (pattern (if dir-wildcard (cdr dir-wildcard) (file-name-nondirectory file))))
		      ;; NB since switches is passed to the shell, be
		      ;; careful of malicious values, eg "-l;reboot".
		      ;; See eg dired-safe-switches-p.
		      (call-process
		       shell-file-name nil t nil
		       shell-command-switch
		       (concat (if (memq system-type '(ms-dos windows-nt))
				   ""
				 "\\") ; Disregard Unix shell aliases!
			       insert-directory-program
			       " -d "
			       ;; Quote switches that require quoting
			       ;; such as "--block-size='1".  But don't
			       ;; quote switches that use patterns
			       ;; such as "--ignore=PATTERN" (bug#71935).
			       (mapconcat #'shell-quote-wildcard-pattern
					  (if (stringp switches)
					      (split-string-and-unquote switches)
					    switches)
					  " ")
			       " -- "
			       ;; Quote some characters that have
			       ;; special meanings in shells; but
			       ;; don't quote the wildcards--we want
			       ;; them to be special.  We also
			       ;; currently don't quote the quoting
			       ;; characters in case people want to
			       ;; use them explicitly to quote
			       ;; wildcard characters.
			       (shell-quote-wildcard-pattern pattern))))
		  ;; SunOS 4.1.3, SVr4 and others need the "." to list the
		  ;; directory if FILE is a symbolic link.
		  (unless full-directory-p
		    (setq switches
			  (cond
                           ((stringp switches) (concat switches " -d"))
                           ((member "-d" switches) switches)
                           (t (append switches '("-d"))))))
		  (if (string-match "\\`~" file)
		      (setq file (expand-file-name file)))
		  (apply #'call-process
			 insert-directory-program nil t nil
			 (append
			  (if (listp switches) switches
			    (unless (equal switches "")
			      ;; Split the switches at any spaces so we can
			      ;; pass separate options as separate args.
			      (split-string-and-unquote switches)))
			  ;; Avoid lossage if FILE starts with `-'.
			  '("--")
			  (list file))))))

	;; If we got "//DIRED//" in the output, it means we got a real
	;; directory listing, even if `ls' returned nonzero.
	;; So ignore any errors.
	(when (if (stringp switches)
		  (string-match "--dired\\>" switches)
		(member "--dired" switches))
	  (save-excursion
            (let ((case-fold-search nil))
	      (forward-line -2)
	      (when (looking-at "//SUBDIRED//")
	        (forward-line -1))
	      (if (looking-at "//DIRED//")
		  (setq result 0)))))

	(when (and (not (eq 0 result))
		   (eq insert-directory-ls-version 'unknown))
	  ;; The first time ls returns an error,
	  ;; find the version numbers of ls,
	  ;; and set insert-directory-ls-version
	  ;; to > if it is more than 5.2.1, < if it is less, nil if it
	  ;; is equal or if the info cannot be obtained.
	  ;; (That can mean it isn't GNU ls.)
	  (let ((version-out
		 (with-temp-buffer
		   (call-process "ls" nil t nil "--version")
		   (buffer-string))))
	    (setq insert-directory-ls-version
		  (if (string-match "ls (.*utils) \\([0-9.]*\\)$" version-out)
		      (let* ((version (match-string 1 version-out))
		             (split (split-string version "[.]"))
		             (numbers (mapcar #'string-to-number split))
		             (min '(5 2 1))
		             comparison)
		        (while (and (not comparison) (or numbers min))
			  (cond ((null min)
			         (setq comparison #'>))
			        ((null numbers)
			         (setq comparison #'<))
			        ((> (car numbers) (car min))
			         (setq comparison #'>))
			        ((< (car numbers) (car min))
			         (setq comparison #'<))
			        (t
				 (setq numbers (cdr numbers)
				       min (cdr min)))))
			(or comparison #'=))
		    nil))))

	;; For GNU ls versions 5.2.2 and up, ignore minor errors.
	(when (and (eq 1 result) (eq insert-directory-ls-version #'>))
	  (setq result 0))

	;; If `insert-directory-program' failed, delete the error output and
	;; return nil — matching the reference build's C-level
	;; `insert-directory', which does not signal on ls failure.
	(unless (eq 0 result)
	  (delete-region beg (point)))
        (insert-directory-clean beg switches)
	;; Now decode what read if necessary.
	(let ((coding (or coding-system-for-read
			  file-name-coding-system
			  default-file-name-coding-system
			  'undecided))
	      coding-no-eol
	      val pos)
	  (when (and enable-multibyte-characters
		     (not (memq (coding-system-base coding)
				'(raw-text no-conversion))))
	    ;; If no coding system is specified or detection is
	    ;; requested, detect the coding.
	    (if (eq (coding-system-base coding) 'undecided)
		(setq coding (detect-coding-region beg (point) t)))
	    (if (not (eq (coding-system-base coding) 'undecided))
		(save-restriction
		  (setq coding-no-eol
			(coding-system-change-eol-conversion coding 'unix))
		  (narrow-to-region beg (point))
		  (goto-char (point-min))
		  (while (not (eobp))
		    (setq pos (point)
			  val (get-text-property (point) 'dired-filename))
		    (goto-char (next-single-property-change
				(point) 'dired-filename nil (point-max)))
		    ;; Force no eol conversion on a file name, so
		    ;; that CR is preserved.
		    (decode-coding-region pos (point)
					  (if val coding-no-eol coding))
		    (if val
			(put-text-property pos (point)
					   'dired-filename t))))))))))))

(defun insert-directory-adj-pos (pos error-lines)
  "Convert `ls --dired' file name position value POS to a buffer position.
File name position values returned in ls --dired output
count only stdout; they don't count the error messages sent to stderr.
So this function converts to them to real buffer positions.
ERROR-LINES is a list of buffer positions of error message lines,
of the form (START END)."
  (while (and error-lines (< (caar error-lines) pos))
    (setq pos (+ pos (- (nth 1 (car error-lines)) (nth 0 (car error-lines)))))
    (pop error-lines))
  pos)

(defun insert-directory-safely (file switches
				     &optional wildcard full-directory-p)
  "Insert directory listing for FILE, formatted according to SWITCHES.

Like `insert-directory', but if FILE does not exist, it inserts a
message to that effect instead of signaling an error."
  (if (file-exists-p file)
      (insert-directory file switches wildcard full-directory-p)
    ;; Simulate the message printed by `ls'.
    (insert (format "%s: No such file or directory\n" file))))

(defcustom byte-count-to-string-function #'file-size-human-readable-iec
  "Function that turns a number of bytes into a human-readable string.
It is for use when displaying file sizes and disk space where other
constraints do not force a specific format."
  :type '(radio
          (function-item file-size-human-readable-iec)
          (function-item file-size-human-readable)
          (function :tag "Custom function" :value number-to-string))
  :group 'files
  :version "27.1")

;; subr.el: mode-hook machinery (verbatim GNU).
(defvar-local delay-mode-hooks nil
  "If non-nil, `run-mode-hooks' should delay running the hooks.")
(defvar-local delayed-mode-hooks nil
  "List of delayed mode hooks waiting to be run.")
(defvar-local delayed-after-hook-functions nil
  "List of functions to run at the end of `run-mode-hooks'.")

(defun run-mode-hooks (&rest hooks)
  "Run mode hooks `delayed-mode-hooks' and HOOKS, or delay HOOKS.
Call `hack-local-variables' to set up file local and directory local
variables.

If the variable `delay-mode-hooks' is non-nil, does not do anything,
just adds the HOOKS to the list `delayed-mode-hooks'.
Otherwise, runs hooks in the sequence: `change-major-mode-after-body-hook',
`delayed-mode-hooks' (in reverse order), HOOKS, then runs
`hack-local-variables' (if the buffer is visiting a file),
runs the hook `after-change-major-mode-hook', and finally
evaluates the functions in `delayed-after-hook-functions' (see
`define-derived-mode').

Major mode functions should use this instead of `run-hooks' when
running their FOO-mode-hook."
  (if delay-mode-hooks
      ;; Delaying case.
      (dolist (hook hooks)
	(push hook delayed-mode-hooks))
    ;; Normal case, just run the hook as before plus any delayed hooks.
    (setq hooks (nconc (nreverse delayed-mode-hooks) hooks))
    (and (bound-and-true-p syntax-propertize-function)
         (not (local-variable-p 'parse-sexp-lookup-properties))
         ;; `syntax-propertize' sets `parse-sexp-lookup-properties' for us, but
         ;; in order for the sexp primitives to automatically call
         ;; `syntax-propertize' we need `parse-sexp-lookup-properties' to be
         ;; set first.
         (setq-local parse-sexp-lookup-properties t))
    (setq delayed-mode-hooks nil)
    (apply #'run-hooks (cons 'change-major-mode-after-body-hook hooks))
    (if (buffer-file-name)
        (with-demoted-errors "File local-variables error: %s"
          (hack-local-variables 'no-mode)))
    (run-hooks 'after-change-major-mode-hook)
    (dolist (fun (prog1 (nreverse delayed-after-hook-functions)
                    (setq delayed-after-hook-functions nil)))
      (funcall fun))))

(defmacro delay-mode-hooks (&rest body)
  "Execute BODY, but delay any `run-mode-hooks'.
These hooks will be executed by the first following call to
`run-mode-hooks' that occurs outside any `delay-mode-hooks' form.
Affects only hooks run in the current buffer."
  (declare (debug t) (indent 0))
  `(progn
     (make-local-variable 'delay-mode-hooks)
     (let ((delay-mode-hooks t))
       ,@body)))

(defvar local-enable-local-variables t
  "Like `enable-local-variables', except for major mode in a -*- line.
The meaningful values are nil and non-nil.  The default is non-nil.
It should be set in a buffer-local fashion.

Setting this to nil has the same effect as setting `enable-local-variables'
to nil, except that it does not ignore any mode: setting in a -*- line.
Unless this difference matters to you, you should set `enable-local-variables'
instead of this variable.")

(defcustom enable-local-eval 'maybe
  "Control processing of the \"variable\" `eval' in a file's local variables.
The value can be t, nil or something else.
A value of t means obey `eval' variables.
A value of nil means ignore them; anything else means query."
  :risky t
  :type '(choice (const :tag "Obey" t)
		 (const :tag "Ignore" nil)
		 (other :tag "Query" other))
  :group 'find-file)

(defun normal-mode (&optional find-file)
  "Choose the major mode for this buffer automatically.
Also sets up any specified local variables of the file or its directory.
Uses the visited file name, the -*- line, and the local variables spec.

This function is called automatically from `find-file'.  In that case,
we may set up the file-specified mode and local variables,
depending on the value of `enable-local-variables'.
In addition, if `local-enable-local-variables' is nil, we do
not set local variables (though we do notice a mode specified with -*-.)

`enable-local-variables' is ignored if you run `normal-mode' interactively,
or from Lisp without specifying the optional argument FIND-FILE;
in that case, this function acts as if `enable-local-variables' were t.

If invoked in a buffer that doesn't visit a file, this function
processes only the major mode specification in the -*- line and
the local variables spec."
  (interactive)
  (kill-all-local-variables)
  (unless delay-mode-hooks
    (run-hooks 'change-major-mode-after-body-hook
               'after-change-major-mode-hook))
  (let ((enable-local-variables (or (not find-file) enable-local-variables)))
    ;; FIXME this is less efficient than it could be, since both
    ;; s-a-m and h-l-v may parse the same regions, looking for "mode:".
    (with-demoted-errors "File mode specification error: %S"
      (set-auto-mode))
    ;; `delay-mode-hooks' being non-nil will have prevented the major
    ;; mode's call to `run-mode-hooks' from calling
    ;; `hack-local-variables'.  In that case, call it now.
    (when delay-mode-hooks
      (with-demoted-errors "File local-variables error: %S"
        (hack-local-variables 'no-mode))))
  ;; Turn font lock off and on, to make sure it takes account of
  ;; whatever file local variables are relevant to it.
  (when (and font-lock-mode
             ;; Font-lock-mode (now in font-core.el) can be ON when
             ;; font-lock.el still hasn't been loaded.
             (boundp 'font-lock-keywords)
             (eq (car font-lock-keywords) t))
    (setq font-lock-keywords (cadr font-lock-keywords))
    (font-lock-mode 1)))

(defcustom auto-mode-case-fold t
  "Non-nil means to try second pass through `auto-mode-alist'.
This means that if the first case-sensitive search through the alist fails
to find a matching major mode, a second case-insensitive search is made.
On systems with case-insensitive file names, this variable is ignored,
since only a single case-insensitive search through the alist is made."
  :group 'files
  :version "22.1"
  :type 'boolean)

(defvar auto-mode-alist
  '(("\\.elc\\'" . elisp-byte-code-mode) ("\\.gpg\\(~\\|\\.~[0-9]+~\\)?\\'" nil epa-file) ("\\.zst\\'" nil jka-compr) ("\\.dz\\'" nil jka-compr) ("\\.xz\\'" nil jka-compr) ("\\.lzma\\'" nil jka-compr) ("\\.lz\\'" nil jka-compr) ("\\.g?z\\'" nil jka-compr) ("\\.bz2\\'" nil jka-compr) ("\\.Z\\'" nil jka-compr) ("\\.ya?ml\\'" . yaml-ts-mode-maybe) ("\\.vr[hi]?\\'" . vera-mode) ("\\.tsx\\'" . tsx-ts-mode-maybe) ("\\.ts\\'" . typescript-ts-mode-maybe) ("\\.rs\\'" . rust-ts-mode-maybe) ("\\(?:\\.\\(?:rbw?\\|ru\\|rake\\|thor\\|axlsx\\|jbuilder\\|rabl\\|gemspec\\|podspec\\)\\|/\\(?:Gem\\|Rake\\|Cap\\|Thor\\|Puppet\\|Berks\\|Brew\\|Fast\\|Vagrant\\|Guard\\|Pod\\)file\\)\\'" . ruby-mode) ("\\.re?st\\'" . rst-mode) ("/\\(?:Pipfile\\|\\.?flake8\\)\\'" . conf-mode) ("\\(?:\\.\\(?:p\\(?:th\\|y[iw]?\\)\\)\\|/\\(?:SCons\\(?:\\(?:crip\\|truc\\)t\\)\\)\\)\\'" . python-mode) ("/\\.php_cs\\(?:\\.dist\\)?\\'" . php-ts-mode-maybe) ("\\.\\(?:php\\|inc\\|stub\\)\\'" . php-ts-mode-maybe) ("\\.\\(?:php[s345]?\\|phtml\\)\\'" . php-ts-mode-maybe) ("\\.m\\'" . octave-maybe-mode) ("\\.lua\\'" . lua-mode) ("\\.less\\'" . less-css-mode) ("\\.[hl]?eex\\'" . heex-ts-mode-maybe) ("/go\\.work\\'" . go-work-ts-mode-maybe) ("/go\\.mod\\'" . go-mod-ts-mode-maybe) ("\\.go\\'" . go-ts-mode-maybe) ("mix\\.lock" . elixir-ts-mode-maybe) ("\\.exs\\'" . elixir-ts-mode-maybe) ("\\.ex\\'" . elixir-ts-mode-maybe) ("\\.elixir\\'" . elixir-ts-mode-maybe) ("\\.editorconfig\\'" . editorconfig-conf-mode) ("\\(?:\\(?:\\(?:Contain\\|Dock\\)erfile\\)\\(?:\\..*\\)?\\|\\.[Dd]ockerfile\\)\\'" . dockerfile-ts-mode-maybe) ("\\.scss\\'" . scss-mode) ("\\.cs\\'" . csharp-mode) ("\\(?:CMakeLists\\.txt\\|\\.cmake\\)\\'" . cmake-ts-mode-maybe) ("\\.awk\\'" . awk-mode) ("\\.\\(u?lpc\\|pike\\|pmod\\(\\.in\\)?\\)\\'" . pike-mode) ("\\.idl\\'" . idl-mode) ("\\.java\\'" . java-mode) ("\\.m\\'" . objc-mode) ("\\.ii\\'" . c++-mode) ("\\.i\\'" . c-mode) ("\\.lex\\'" . c-mode) ("\\.y\\(acc\\)?\\'" . c-mode) ("\\.h\\'" . c-or-c++-mode) ("\\.c\\'" . c-mode) ("\\.\\(CC?\\|HH?\\)\\'" . c++-mode) ("\\.[ch]\\(pp\\|xx\\|\\+\\+\\)\\'" . c++-mode) ("\\.\\(cc\\|hh\\)\\'" . c++-mode) ("\\.\\(bat\\|cmd\\)\\'" . bat-mode) ("\\.[sx]?html?\\(\\.[a-zA-Z_]+\\)?\\'" . mhtml-mode) ("\\.svgz?\\'" . image-mode) ("\\.svgz?\\'" . xml-mode) ("\\.x[bp]m\\'" . image-mode) ("\\.x[bp]m\\'" . c-mode) ("\\.p[bpgn]m\\'" . image-mode) ("\\.tiff?\\'" . image-mode) ("\\.gif\\'" . image-mode) ("\\.png\\'" . image-mode) ("\\.jpe?g\\'" . image-mode) ("\\.webp\\'" . image-mode) ("\\.te?xt\\'" . text-mode) ("\\.[tT]e[xX]\\'" . tex-mode) ("\\.ins\\'" . tex-mode) ("\\.ltx\\'" . latex-mode) ("\\.dtx\\'" . doctex-mode) ("\\.org\\'" . org-mode) ("\\.dir-locals\\(?:-2\\)?\\.el\\'" . lisp-data-mode) ("\\.eld\\'" . lisp-data-mode) ("eww-bookmarks\\'" . lisp-data-mode) ("tramp\\'" . lisp-data-mode) ("/archive-contents\\'" . lisp-data-mode) ("places\\'" . lisp-data-mode) ("\\.emacs-places\\'" . lisp-data-mode) ("\\.el\\'" . emacs-lisp-mode) ("Project\\.ede\\'" . emacs-lisp-mode) ("\\(?:\\.\\(?:scm\\|sls\\|sld\\|stk\\|ss\\|sch\\)\\|/\\.guile\\)\\'" . scheme-mode) ("\\.l\\'" . lisp-mode) ("\\.li?sp\\'" . lisp-mode) ("\\.[fF]\\'" . fortran-mode) ("\\.for\\'" . fortran-mode) ("\\.p\\'" . pascal-mode) ("\\.pas\\'" . pascal-mode) ("\\.\\(dpr\\|DPR\\)\\'" . opascal-mode) ("\\.\\([pP]\\([Llm]\\|erl\\|od\\)\\|al\\)\\'" . perl-mode) ("Imakefile\\'" . makefile-imake-mode) ("Makeppfile\\(?:\\.mk\\)?\\'" . makefile-makepp-mode) ("\\.makepp\\'" . makefile-makepp-mode) ("\\.mk\\'" . makefile-bsdmake-mode) ("\\.make\\'" . makefile-bsdmake-mode) ("GNUmakefile\\'" . makefile-gmake-mode) ("[Mm]akefile\\'" . makefile-bsdmake-mode) ("\\.am\\'" . makefile-automake-mode) ("\\.texinfo\\'" . texinfo-mode) ("\\.te?xi\\'" . texinfo-mode) ("\\.[sS]\\'" . asm-mode) ("\\.asm\\'" . asm-mode) ("\\.css\\'" . css-mode) ("\\.mixal\\'" . mixal-mode) ("\\.gcov\\'" . compilation-mode) ("/[._]?[A-Za-z0-9-]*\\(?:gdbinit\\(?:\\.\\(?:ini?\\|loader\\)\\)?\\|gdb\\.ini\\)\\'" . gdb-script-mode) ("-gdb\\.gdb" . gdb-script-mode) ("[cC]hange\\.?[lL]og?\\'" . change-log-mode) ("[cC]hange[lL]og[-.][0-9]+\\'" . change-log-mode) ("\\$CHANGE_LOG\\$\\.TXT" . change-log-mode) ("\\.scm\\.[0-9]*\\'" . scheme-mode) ("\\.[ckz]?sh\\'\\|\\.shar\\'\\|/\\.z?profile\\'" . sh-mode) ("\\.bash\\'" . sh-mode) ("/bash-fc\\.[0-9A-Za-z]\\{6\\}\\'" . sh-mode) ("\\`/etc/profile\\'" . sh-mode) ("/PKGBUILD\\'" . sh-mode) ("\\(/\\|\\`\\)\\.\\(bash_\\(profile\\|history\\|log\\(in\\|out\\)\\)\\|z?log\\(in\\|out\\)\\)\\'" . sh-mode) ("\\(/\\|\\`\\)\\.\\(shrc\\|zshrc\\|m?kshrc\\|bashrc\\|t?cshrc\\|esrc\\)\\'" . sh-mode) ("\\(/\\|\\`\\)\\.\\([kz]shenv\\|xinitrc\\|startxrc\\|xsession\\)\\'" . sh-mode) ("\\.m?spec\\'" . sh-mode) ("\\.m[mes]\\'" . nroff-mode) ("\\.man\\'" . nroff-mode) ("\\.sty\\'" . latex-mode) ("\\.cl[so]\\'" . latex-mode) ("\\.bbl\\'" . latex-mode) ("\\.bib\\'" . bibtex-mode) ("\\.bst\\'" . bibtex-style-mode) ("\\.sql\\'" . sql-mode) ("\\(acinclude\\|aclocal\\|acsite\\)\\.m4\\'" . autoconf-mode) ("\\.m[4c]\\'" . m4-mode) ("\\.mf\\'" . metafont-mode) ("\\.mp\\'" . metapost-mode) ("\\.vhdl?\\'" . vhdl-mode) ("\\.article\\'" . text-mode) ("\\.letter\\'" . text-mode) ("\\.i?tcl\\'" . tcl-mode) ("\\.exp\\'" . tcl-mode) ("\\.itk\\'" . tcl-mode) ("\\.icn\\'" . icon-mode) ("\\.sim\\'" . simula-mode) ("\\.mss\\'" . scribe-mode) ("\\.f9[05]\\'" . f90-mode) ("\\.f0[38]\\'" . f90-mode) ("\\.srt\\'" . srecode-template-mode) ("\\.prolog\\'" . prolog-mode) ("\\.tar\\'" . tar-mode) ("\\.\\(arc\\|zip\\|lzh\\|lha\\|zoo\\|[jew]ar\\|xpi\\|rar\\|cbr\\|7z\\|squashfs\\|ARC\\|ZIP\\|LZH\\|LHA\\|ZOO\\|[JEW]AR\\|XPI\\|RAR\\|CBR\\|7Z\\|SQUASHFS\\)\\'" . archive-mode) ("\\.oxt\\'" . archive-mode) ("\\.\\(deb\\|[oi]pk\\)\\'" . archive-mode) ("\\`/tmp/Re" . text-mode) ("/Message[0-9]*\\'" . text-mode) ("\\`/tmp/fol/" . text-mode) ("\\.oak\\'" . scheme-mode) ("\\.sgml?\\'" . sgml-mode) ("\\.x[ms]l\\'" . xml-mode) ("\\.slnx\\'" . xml-mode) ("\\.dbk\\'" . xml-mode) ("\\.dtd\\'" . sgml-mode) ("\\.ds\\(ss\\)?l\\'" . dsssl-mode) ("\\.js[mx]?\\'" . javascript-mode) ("\\.har\\'" . javascript-mode) ("\\.json\\'" . js-json-mode) ("\\.[ds]?va?h?\\'" . verilog-mode) ("\\.by\\'" . bovine-grammar-mode) ("\\.wy\\'" . wisent-grammar-mode) ("\\.erts\\'" . erts-mode) ("[:/\\]\\..*\\(emacs\\|gnus\\|viper\\)\\'" . emacs-lisp-mode) ("\\`\\..*emacs\\'" . emacs-lisp-mode) ("[:/]_emacs\\'" . emacs-lisp-mode) ("/crontab\\.X*[0-9]+\\'" . shell-script-mode) ("\\.ml\\'" . lisp-mode) ("\\.ld[si]?\\'" . ld-script-mode) ("ld\\.?script\\'" . ld-script-mode) ("\\.xs\\'" . c-mode) ("\\.x[abdsru]?[cnw]?\\'" . ld-script-mode) ("\\.zone\\'" . dns-mode) ("\\.soa\\'" . dns-mode) ("\\.asd\\'" . lisp-mode) ("\\.\\(asn\\|mib\\|smi\\)\\'" . snmp-mode) ("\\.\\(as\\|mi\\|sm\\)2\\'" . snmpv2-mode) ("\\.\\(diffs?\\|patch\\|rej\\)\\'" . diff-mode) ("\\.\\(dif\\|pat\\)\\'" . diff-mode) ("\\.[eE]?[pP][sS]\\'" . ps-mode) ("\\.\\(?:PDF\\|EPUB\\|CBZ\\|FB2\\|O?XPS\\|DVI\\|OD[FGPST]\\|DOCX\\|XLSX?\\|PPTX?\\|pdf\\|epub\\|cbz\\|fb2\\|o?xps\\|djvu\\|dvi\\|od[fgpst]\\|docx\\|xlsx?\\|pptx?\\)\\'" . doc-view-mode-maybe) ("configure\\.\\(ac\\|in\\)\\'" . autoconf-mode) ("\\.s\\(v\\|iv\\|ieve\\)\\'" . sieve-mode) ("BROWSE\\'" . ebrowse-tree-mode) ("\\.ebrowse\\'" . ebrowse-tree-mode) ("#\\*mail\\*" . mail-mode) ("\\.g\\'" . antlr-mode) ("\\.g4\\'" . antlr-v4-mode) ("\\.mod\\'" . m2-mode) ("\\.ses\\'" . ses-mode) ("\\.docbook\\'" . sgml-mode) ("\\.com\\'" . dcl-mode) ("/config\\.\\(?:bat\\|log\\)\\'" . fundamental-mode) ("/\\.?\\(authinfo\\|netrc\\)\\'" . authinfo-mode) ("\\.\\(?:[iI][nN][iI]\\|[lL][sS][tT]\\|[rR][eE][gG]\\|[sS][yY][sS]\\)\\'" . conf-mode) ("\\.la\\'" . conf-unix-mode) ("\\.ppd\\'" . conf-ppd-mode) ("java.+\\.conf\\'" . conf-javaprop-mode) ("\\.properties\\(?:\\.[a-zA-Z0-9._-]+\\)?\\'" . conf-javaprop-mode) ("\\.toml\\'" . conf-toml-mode) ("\\.desktop\\'" . conf-desktop-mode) ("npmrc\\'" . conf-npmrc-mode) ("/\\.redshift\\.conf\\'" . conf-windows-mode) ("\\`/etc/\\(?:DIR_COLORS\\|ethers\\|.?fstab\\|.*hosts\\|lesskey\\|login\\.?de\\(?:fs\\|vperm\\)\\|magic\\|mtab\\|pam\\.d/.*\\|permissions\\(?:\\.d/.+\\)?\\|protocols\\|rpc\\|services\\)\\'" . conf-space-mode) ("\\`/etc/\\(?:acpid?/.+\\|aliases\\(?:\\.d/.+\\)?\\|default/.+\\|group-?\\|hosts\\..+\\|inittab\\|ksysguarddrc\\|opera6rc\\|passwd-?\\|shadow-?\\|sysconfig/.+\\)\\'" . conf-mode) ("[cC]hange[lL]og[-.][-0-9a-z]+\\'" . change-log-mode) ("/\\.?\\(?:gitconfig\\|gnokiirc\\|hgrc\\|kde.*rc\\|mime\\.types\\|wgetrc\\)\\'" . conf-mode) ("/\\.mailmap\\'" . conf-unix-mode) ("/\\.\\(?:asound\\|enigma\\|fetchmail\\|gltron\\|gtk\\|hxplayer\\|mairix\\|mbsync\\|msmtp\\|net\\|neverball\\|nvidia-settings-\\|offlineimap\\|qt/.+\\|realplayer\\|reportbug\\|rtorrent\\.\\|screen\\|scummvm\\|sversion\\|sylpheed/.+\\|xmp\\)rc\\'" . conf-mode) ("/\\.\\(?:gdbtkinit\\|grip\\|mpdconf\\|notmuch-config\\|orbital/.+txt\\|rhosts\\|tuxracer/options\\)\\'" . conf-mode) ("/\\.?X\\(?:default\\|resource\\|re\\)s\\>" . conf-xdefaults-mode) ("/X11.+app-defaults/\\|\\.ad\\'" . conf-xdefaults-mode) ("/X11.+locale/.+/Compose\\'" . conf-colon-mode) ("/X11.+locale/compose\\.dir\\'" . conf-javaprop-mode) ("\\.~?[0-9]+\\.[0-9][-.0-9]*~?\\'" nil t) ("\\.\\(?:orig\\|in\\|[bB][aA][kK]\\)\\'" nil t) ("[/.]c\\(?:on\\)?f\\(?:i?g\\)?\\(?:\\.[a-zA-Z0-9._-]+\\)?\\'" . conf-mode-maybe) ("\\.[1-9]\\'" . nroff-mode) ("\\.avif\\'" . image-mode) ("\\.art\\'" . image-mode) ("\\.avs\\'" . image-mode) ("\\.bmp\\'" . image-mode) ("\\.cmyk\\'" . image-mode) ("\\.cmyka\\'" . image-mode) ("\\.crw\\'" . image-mode) ("\\.dcm\\'" . image-mode) ("\\.dcr\\'" . image-mode) ("\\.dcx\\'" . image-mode) ("\\.dng\\'" . image-mode) ("\\.dpx\\'" . image-mode) ("\\.fax\\'" . image-mode) ("\\.heic\\'" . image-mode) ("\\.hrz\\'" . image-mode) ("\\.icb\\'" . image-mode) ("\\.icc\\'" . image-mode) ("\\.icm\\'" . image-mode) ("\\.ico\\'" . image-mode) ("\\.icon\\'" . image-mode) ("\\.jbg\\'" . image-mode) ("\\.jbig\\'" . image-mode) ("\\.jng\\'" . image-mode) ("\\.jnx\\'" . image-mode) ("\\.miff\\'" . image-mode) ("\\.mng\\'" . image-mode) ("\\.mvg\\'" . image-mode) ("\\.otb\\'" . image-mode) ("\\.p7\\'" . image-mode) ("\\.pcx\\'" . image-mode) ("\\.pdb\\'" . image-mode) ("\\.pfa\\'" . image-mode) ("\\.pfb\\'" . image-mode) ("\\.picon\\'" . image-mode) ("\\.pict\\'" . image-mode) ("\\.rgb\\'" . image-mode) ("\\.rgba\\'" . image-mode) ("\\.six\\'" . image-mode) ("\\.tga\\'" . image-mode) ("\\.wbmp\\'" . image-mode) ("\\.wmf\\'" . image-mode) ("\\.wpg\\'" . image-mode) ("\\.xcf\\'" . image-mode) ("\\.xmp\\'" . image-mode) ("\\.xwd\\'" . image-mode) ("\\.yuv\\'" . image-mode) ("\\.tgz\\'" . tar-mode) ("\\.tbz2?\\'" . tar-mode) ("\\.txz\\'" . tar-mode) ("\\.tzst\\'" . tar-mode))
  "Alist of filename patterns vs corresponding major mode functions.
GNU Emacs runtime default, including entries injected by autoloads
(cc-mode, python, jka-compr, epa-file, and friends).")

(put 'auto-mode-alist 'risky-local-variable t)

(defun conf-mode-maybe ()
  "Select Conf mode or XML mode according to start of file."
  (if (save-excursion
	(save-restriction
	  (widen)
	  (goto-char (point-min))
	  (looking-at "<\\?xml \\|<!-- \\|<!DOCTYPE ")))
      (xml-mode)
    (conf-mode)))

(defvar interpreter-mode-alist
  '(("j?ruby\\(?:[0-9.]+\\)" . ruby-mode) ("jruby" . ruby-mode) ("rbx" . ruby-mode) ("ruby" . ruby-mode) ("python[0-9.]*" . python-mode) ("php\\(?:-?[34578]\\(?:\\.[0-9]+\\)*\\)?" . php-ts-mode-maybe) ("lua" . lua-mode) ("rhino" . js-mode) ("gjs" . js-mode) ("nodejs" . js-mode) ("node" . js-mode) ("gawk" . awk-mode) ("nawk" . awk-mode) ("mawk" . awk-mode) ("awk" . awk-mode) ("pike" . pike-mode) ("\\(mini\\)?perl5?" . perl-mode) ("wishx?" . tcl-mode) ("tcl\\(sh\\)?" . tcl-mode) ("expect" . tcl-mode) ("octave" . octave-mode) ("scm" . scheme-mode) ("[acjkwz]sh" . sh-mode) ("r?bash2?" . sh-mode) ("dash" . sh-mode) ("mksh" . sh-mode) ("\\(dt\\|pd\\|w\\)ksh" . sh-mode) ("es" . sh-mode) ("i?tcsh" . sh-mode) ("oash" . sh-mode) ("rc" . sh-mode) ("rpm" . sh-mode) ("sh5?" . sh-mode) ("tail" . text-mode) ("more" . text-mode) ("less" . text-mode) ("pg" . text-mode) ("make" . makefile-gmake-mode) ("guile" . scheme-mode) ("clisp" . lisp-mode) ("emacs" . emacs-lisp-mode))
  "Alist of interpreters vs corresponding major modes.
GNU Emacs runtime default.")

(defvar inhibit-local-variables-regexps
  '("\\.tar\\'" "\\.t[bg]z\\'"
    "\\.arc\\'" "\\.zip\\'" "\\.lzh\\'" "\\.lha\\'"
    "\\.zoo\\'" "\\.[jew]ar\\'" "\\.xpi\\'" "\\.rar\\'"
    "\\.7z\\'"
    "\\.sx[dmicw]\\'" "\\.odt\\'"
    "\\.diff\\'" "\\.patch\\'"
    "\\.tiff?\\'" "\\.gif\\'" "\\.png\\'" "\\.jpe?g\\'")
  "List of regexps matching file names in which to ignore local variables.
This includes `-*-' lines as well as trailing \"Local Variables\" sections.
Files matching this list are typically binary file formats.
They may happen to contain sequences that look like local variable
specifications, but are not really, or they may be containers for
member files with their own local variable sections, which are
not appropriate for the containing file.
The function `inhibit-local-variables-p' uses this.")

(defvar inhibit-local-variables-suffixes nil
  "List of regexps matching suffixes to remove from file names.
The function `inhibit-local-variables-p' uses this: when checking
a file name, it first discards from the end of the name anything that
matches one of these regexps.")

;; Can't think of any situation in which you'd want this to be nil...
(defvar inhibit-local-variables-ignore-case t
  "Non-nil means `inhibit-local-variables-p' ignores case.")

(defun inhibit-local-variables-p ()
  "Return non-nil if file local variables should be ignored.
This checks the file (or buffer) name against `inhibit-local-variables-regexps'
and `inhibit-local-variables-suffixes'.  If
`inhibit-local-variables-ignore-case' is non-nil, this ignores case."
  (let ((temp inhibit-local-variables-regexps)
	(name (if buffer-file-name
		  (file-name-sans-versions buffer-file-name)
		(buffer-name)))
	(case-fold-search inhibit-local-variables-ignore-case))
    (while (let ((sufs inhibit-local-variables-suffixes))
	     (while (and sufs (not (string-match (car sufs) name)))
	       (setq sufs (cdr sufs)))
	     sufs)
      (setq name (substring name 0 (match-beginning 0))))
    (while (and temp
		(not (string-match (car temp) name)))
      (setq temp (cdr temp)))
    temp))

(defvar auto-mode-interpreter-regexp
  (concat
   "#![ \t]*"
   ;; Optional group 1: env(1) invocation.
   "\\("
   "[^ \t\n]*/bin/env[ \t]*"
   ;; Within group 1: possible -S/--split-string and environment
   ;; adjustments.
   "\\(?:"
   ;; -S/--split-string
   "\\(?:-[0a-z]*S[ \t]*\\|--split-string=\\)"
   ;; More env arguments.
   "\\(?:-[^ \t\n]+[ \t]+\\)*"
   ;; Interpreter environment modifications.
   "\\(?:[^ \t\n]+=[^ \t\n]*[ \t]+\\)*"
   "\\)?"
   "\\)?"
   ;; Group 2: interpreter.
   "\\([^ \t\n]+\\)")
  "Regexp matching interpreters, for file mode determination.
This regular expression is matched against the first line of a file
to determine the file's mode in `set-auto-mode'.  If it matches, the file
is assumed to be interpreted by the interpreter matched by the second group
of the regular expression.  The mode is then determined as the mode
associated with that interpreter in `interpreter-mode-alist'.")

(defvar magic-mode-alist nil
  "Alist of buffer beginnings vs. corresponding major mode functions.
Each element looks like (REGEXP . FUNCTION) or (MATCH-FUNCTION . FUNCTION).
After visiting a file, if REGEXP matches the text at the beginning of the
buffer (case-sensitively), or calling MATCH-FUNCTION returns non-nil,
`normal-mode' will call FUNCTION rather than allowing `auto-mode-alist' to
decide the buffer's major mode.

If FUNCTION is nil, then it is not called.  (That is a way of saying
\"allow `auto-mode-alist' to decide for these files.\")")
(put 'magic-mode-alist 'risky-local-variable t)

(defvar magic-fallback-mode-alist
  `((image-type-auto-detected-p . image-mode)
    ("\\(PK00\\)?[P]K\003\004" . archive-mode) ; zip
    ;; The < comes before the groups (but the first) to reduce backtracking.
    ;; TODO: UTF-16 <?xml may be preceded by a BOM 0xff 0xfe or 0xfe 0xff.
    ;; We use [ \t\r\n] instead of `\\s ' to make regex overflow less likely.
    (,(let* ((incomment-re "\\(?:[^-]\\|-[^-]\\)")
	     (comment-re (concat "\\(?:!--" incomment-re "*-->[ \t\r\n]*<\\)")))
	(concat "\\(?:<\\?xml[ \t\r\n]+[^>]*>\\)?[ \t\r\n]*<"
		comment-re "*"
		"\\(?:!DOCTYPE[ \t\r\n]+[^>]*>[ \t\r\n]*<[ \t\r\n]*" comment-re "*\\)?"
		"[Hh][Tt][Mm][Ll]"))
     . mhtml-mode)
    ("<![Dd][Oo][Cc][Tt][Yy][Pp][Ee][ \t\r\n]+[Hh][Tt][Mm][Ll]" . mhtml-mode)
    ;; These two must come after html, because they are more general:
    ("<\\?xml " . xml-mode)
    (,(let* ((incomment-re "\\(?:[^-]\\|-[^-]\\)")
	     (comment-re (concat "\\(?:!--" incomment-re "*-->[ \t\r\n]*<\\)")))
	(concat "[ \t\r\n]*<" comment-re "*!DOCTYPE "))
     . sgml-mode)
    ("\320\317\021\340\241\261\032\341" . doc-view-mode-maybe) ; Word documents 1997-2004
    ("%!PS" . ps-mode)
    ("# xmcd " . conf-unix-mode))
  "Like `magic-mode-alist' but has lower priority than `auto-mode-alist'.
Each element looks like (REGEXP . FUNCTION) or (MATCH-FUNCTION . FUNCTION).
After visiting a file, if REGEXP matches the text at the beginning of the
buffer (case-sensitively), or calling MATCH-FUNCTION returns non-nil,
`normal-mode' will call FUNCTION, provided that `magic-mode-alist' and
`auto-mode-alist' have not specified a mode for this file.

If FUNCTION is nil, then it is not called.")
(put 'magic-fallback-mode-alist 'risky-local-variable t)

(defvar magic-mode-regexp-match-limit 4000
  "Upper limit on `magic-mode-alist' regexp matches.
Also applies to `magic-fallback-mode-alist'.")

(defun set-auto-mode--find-matching-alist-entry (alist name case-insensitive)
  "Find first matching entry in ALIST for file NAME.

If CASE-INSENSITIVE, the file system of file NAME is case-insensitive."
  (let (mode)
    (while name
      (let ((newmode
             (if case-insensitive
                 ;; Filesystem is case-insensitive.
                 (let ((case-fold-search t))
                   (assoc-default name alist 'string-match))
               ;; Filesystem is case-sensitive.
               (or
                ;; First match case-sensitively.
                (let ((case-fold-search nil))
                  (assoc-default name alist 'string-match))
                ;; Fallback to case-insensitive match.
                (and auto-mode-case-fold
                     (let ((case-fold-search t))
                       (assoc-default name alist 'string-match)))))))
        (when newmode
          (when mode
            ;; We had already found a mode but in a (REGEXP MODE t)
            ;; entry, so we still have to run MODE.  Let's do it now.
            ;; FIXME: It's kind of ugly to run the function here.
            ;; An alternative could be to return a list of functions and
            ;; callers.
            (set-auto-mode-0 mode t))
          (setq mode newmode))
        (if (and newmode
                 (not (functionp newmode))
                 (consp newmode)
                 (cadr newmode))
            ;; It's a (REGEXP MODE t): Keep looking but remember the MODE.
            (setq mode (car newmode)
                  name (substring name 0 (match-beginning 0)))
          (setq name nil))))
    mode))

(defun set-auto-mode--apply-alist (alist keep-mode-if-same dir-local)
  "Helper function for `set-auto-mode'.
This function takes an alist of the same form as
`auto-mode-alist'.  It then tries to find the appropriate match
in the alist for the current buffer; setting the mode if
possible.
Return non-nil if the mode was set, nil otherwise.
DIR-LOCAL non-nil means this call is via directory-locals, and
extra checks should be done."
  (if buffer-file-name
      (let (mode
            (name buffer-file-name)
            (remote-id (file-remote-p buffer-file-name))
            (case-insensitive-p (file-name-case-insensitive-p
                                 buffer-file-name)))
        ;; Remove backup-suffixes from file name.
        (setq name (file-name-sans-versions name))
        ;; Remove remote file name identification.
        (when (and (stringp remote-id)
                   (string-match (regexp-quote remote-id) name))
          (setq name (substring name (match-end 0))))
        (setq mode (set-auto-mode--find-matching-alist-entry
                    alist name case-insensitive-p))
        (when (and dir-local mode
                   (not (set-auto-mode--dir-local-valid-p mode)))
          (message "Ignoring invalid mode `%S'" mode)
          (setq mode nil))
        (when mode
          (set-auto-mode-0 mode keep-mode-if-same)
          t))))

(defun set-auto-mode--dir-local-valid-p (mode)
  "Say whether MODE can be used in a .dir-local.el `auto-mode-alist'."
  (and (symbolp mode)
       (string-suffix-p "-mode" (symbol-name mode))
       (commandp mode)
       (not (provided-mode-derived-p mode 'special-mode))))

(defun set-auto-mode (&optional keep-mode-if-same)
  "Select major mode appropriate for current buffer.

To find the right major mode, this function checks for a -*- mode tag
checks for a `mode:' entry in the Local Variables section of the file,
checks if there an `auto-mode-alist' entry in `.dir-locals.el',
checks if it uses an interpreter listed in `interpreter-mode-alist',
matches the buffer beginning against `magic-mode-alist',
compares the file name against the entries in `auto-mode-alist',
then matches the buffer beginning against `magic-fallback-mode-alist'.
It also obeys `major-mode-remap-alist' and `major-mode-remap-defaults'.

If `enable-local-variables' is nil, or if the file name matches
`inhibit-local-variables-regexps', this function does not check
for any mode: tag anywhere in the file.  If `local-enable-local-variables'
is nil, then the only mode: tag that can be relevant is a -*- one.

If the optional argument KEEP-MODE-IF-SAME is non-nil, then we
set the major mode only if that would change it.  In other words
we don't actually set it to the same mode the buffer already has."
  ;; Look for -*-MODENAME-*- or -*- ... mode: MODENAME; ... -*-
  (let ((try-locals (not (inhibit-local-variables-p)))
	end modes)
    ;; Once we drop the deprecated feature where mode: is also allowed to
    ;; specify minor-modes (ie, there can be more than one "mode:"), we can
    ;; remove this section and just let (hack-local-variables t) handle it.
    ;; Find a -*- mode tag.
    (save-excursion
      (goto-char (point-min))
      (skip-chars-forward " \t\n")
      ;; Note by design local-enable-local-variables does not matter here.
      (and enable-local-variables
	   try-locals
	   (setq end (set-auto-mode-1))
	   (if (save-excursion (search-forward ":" end t))
	       ;; Find all specifications for the `mode:' variable
	       ;; and execute them left to right.
	       (while (let ((case-fold-search t))
			(or (and (looking-at "mode:")
				 (goto-char (match-end 0)))
			    (re-search-forward "[ \t;]mode:" end t)))
		 (skip-chars-forward " \t")
		 (let ((beg (point)))
		   (if (search-forward ";" end t)
		       (forward-char -1)
		     (goto-char end))
		   (skip-chars-backward " \t")
		   (push (intern (concat (downcase (buffer-substring beg (point))) "-mode"))
			 modes)))
	     ;; Simple -*-MODE-*- case.
	     (push (intern (concat (downcase (buffer-substring (point) end))
				   "-mode"))
		   modes))))
    (or
     ;; If we found modes to use, invoke them now, outside the save-excursion.
     ;; Presume `modes' holds a major mode followed by minor modes.
     (let ((done ()))
       (dolist (mode (nreverse modes))
	 (if (eq done :keep)
	     ;; `keep-mode-if-same' is set and the (major) mode
	     ;; was already set.  Refrain from calling the following
	     ;; minor modes since they have already been set.
	     ;; It was especially important in the past when calling
	     ;; minor modes without an arg would toggle them, but it's
             ;; still preferable to avoid re-enabling them,
	     nil
	   (let ((res (set-auto-mode-0 mode keep-mode-if-same)))
	     (setq done (or res done)))))
       done)
     ;; Check for auto-mode-alist entry in dir-locals.
     (with-demoted-errors "Directory-local variables error: %s"
       ;; Note this is a no-op if enable-local-variables is nil.
       ;; We don't use `hack-dir-local-get-variables-functions' here, because
       ;; modes are specific to Emacs.
       (let* ((mode-alist (cdr (hack-dir-local--get-variables
                                (lambda (key) (eq key 'auto-mode-alist))))))
         (set-auto-mode--apply-alist mode-alist keep-mode-if-same t)))
     (let ((mode (hack-local-variables t (not try-locals))))
       (unless (memq mode modes)	; already tried and failed
         (set-auto-mode-0 mode keep-mode-if-same)))
     ;; If we didn't, look for an interpreter specified in the first line.
     ;; As a special case, allow for things like "#!/bin/env perl", which
     ;; finds the interpreter anywhere in $PATH.
     (when-let*
	 ((interp (save-excursion
		    (goto-char (point-min))
		    (if (looking-at auto-mode-interpreter-regexp)
			(match-string 2))))
	  ;; Map interpreter name to a mode, signaling we're done at the
	  ;; same time.
	  (mode (assoc-default
		 (file-name-nondirectory interp)
		 (mapcar (lambda (e)
                           (cons
                            (format "\\`%s\\'" (car e))
                            (cdr e)))
			 interpreter-mode-alist)
		 #'string-match-p)))
       ;; If we found an interpreter mode to use, invoke it now.
       (set-auto-mode-0 mode keep-mode-if-same))
     ;; Next try matching the buffer beginning against magic-mode-alist.
     (let ((mode (save-excursion
		   (goto-char (point-min))
		   (save-restriction
		     (narrow-to-region (point-min)
				       (min (point-max)
					    (+ (point-min) magic-mode-regexp-match-limit)))
                     (assoc-default
                      nil magic-mode-alist
                      (lambda (re _dummy)
                        (cond
                         ((functionp re)
                          (funcall re))
                         ((stringp re)
                          (let ((case-fold-search nil))
                            (looking-at re)))
                         (t
                          (error
                           "Problem in magic-mode-alist with element %s"
                           re)))))))))
       (set-auto-mode-0 mode keep-mode-if-same))
     ;; Next compare the filename against the entries in auto-mode-alist.
     (set-auto-mode--apply-alist auto-mode-alist
                                 keep-mode-if-same nil)
     ;; Next try matching the buffer beginning against magic-fallback-mode-alist.
     (let ((mode (save-excursion
		   (goto-char (point-min))
		   (save-restriction
		     (narrow-to-region (point-min)
				       (min (point-max)
					    (+ (point-min) magic-mode-regexp-match-limit)))
		     (assoc-default nil magic-fallback-mode-alist
                                    (lambda (re _dummy)
                                      (cond
                                       ((functionp re)
                                        (funcall re))
                                       ((stringp re)
                                        (let ((case-fold-search nil))
                                          (looking-at re)))
                                       (t
                                        (error
                                         "Problem with magic-fallback-mode-alist element: %s"
                                         re)))))))))
       (set-auto-mode-0 mode keep-mode-if-same))
     (set-buffer-major-mode (current-buffer)))))

(defvar-local set-auto-mode--last nil
  "Remember the mode we have set via `set-auto-mode-0'.")

(defcustom major-mode-remap-alist nil
  "Alist mapping file-specified modes to alternative modes.
Each entry is of the form (MODE . FUNCTION) which means that in place
of activating the major mode MODE (specified via something like
`auto-mode-alist', file-local variables, ...) we actually call FUNCTION
instead.
FUNCTION is typically a major mode which \"does the same thing\" as
MODE, but can also be nil to hide other entries (either in this var or
in `major-mode-remap-defaults') and means that we should call MODE."
  :type '(alist
          :tag "Remappings"
          :key-type (symbol :tag "From major mode")
          :value-type (function :tag "To mode (or function)")))

(defvar major-mode-remap-defaults nil
  "Alist mapping file-specified modes to alternative modes.
This works like `major-mode-remap-alist' except it has lower priority
and it is meant to be modified by packages rather than users.")

(defun major-mode-remap (mode)
  "Return the function to use to enable MODE."
  (or (cdr (or (assq mode major-mode-remap-alist)
               (assq mode major-mode-remap-defaults)))
      mode))

;; When `keep-mode-if-same' is set, we are working on behalf of
;; set-visited-file-name.  In that case, if the major mode specified is the
;; same one we already have, don't actually reset it.  We don't want to lose
;; minor modes such as Font Lock.
(defun set-auto-mode-0 (mode &optional keep-mode-if-same)
  "Apply MODE and return it.
If optional arg KEEP-MODE-IF-SAME is non-nil, MODE is chased of
any aliases and compared to current major mode.  If they are the
same, do nothing and return `:keep'.
Return nil if MODE could not be applied."
  (when mode
    (if (and keep-mode-if-same
	     (or (eq (indirect-function mode)
		     (indirect-function major-mode))
		 (and set-auto-mode--last
		      (eq mode (car set-auto-mode--last))
		      (eq major-mode (cdr set-auto-mode--last)))))
	:keep
      (let ((modefun (major-mode-remap mode)))
        (if (not (functionp modefun))
            (progn
              (message "Ignoring unknown mode `%s'%s" mode
                       (if (eq mode modefun) ""
                         (format " (remapped to `%S')" modefun)))
              nil)
          (funcall modefun)
          (unless (or (eq mode major-mode) ;`set-auto-mode--last' is overkill.
                      ;; `modefun' is something like a minor mode.
                      (local-variable-p 'set-auto-mode--last))
            (setq set-auto-mode--last (cons mode major-mode)))
          mode)))))

(defvar file-auto-mode-skip "^\\(#!\\|'\\\\\"\\)"
  "Regexp of lines to skip when looking for file-local settings.
If the first line matches this regular expression, then the -*-...-*- file-
local settings will be consulted on the second line instead of the first.")

(defun set-auto-mode-1 ()
  "Find the -*- spec in the buffer.
Call with point at the place to start searching from.
If one is found, set point to the beginning and return the position
of the end.  Otherwise, return nil; may change point.
The variable `inhibit-local-variables-regexps' can cause a -*- spec to
be ignored; but `enable-local-variables' and `local-enable-local-variables'
have no effect."
  (let (beg end)
    (and
     ;; Don't look for -*- if this file name matches any
     ;; of the regexps in inhibit-local-variables-regexps.
     (not (inhibit-local-variables-p))
     (search-forward "-*-" (line-end-position
                            ;; If the file begins with "#!"  (exec
                            ;; interpreter magic), look for mode frobs
                            ;; in the first two lines.  You cannot
                            ;; necessarily put them in the first line
                            ;; of such a file without screwing up the
                            ;; interpreter invocation.  The same holds
                            ;; for '\" in man pages (preprocessor
                            ;; magic for the `man' program).
                            (and (looking-at file-auto-mode-skip) 2))
                     t)
     (progn
       (skip-chars-forward " \t")
       (setq beg (point))
       (search-forward "-*-" (line-end-position) t))
     (progn
       (forward-char -3)
       (skip-chars-backward " \t")
       (setq end (point))
       (goto-char beg)
       end))))

;;; Handling file local variables

(defvar ignored-local-variables
  '(ignored-local-variables safe-local-variable-values
    file-local-variables-alist dir-local-variables-alist)
  "Variables to be ignored in a file's local variable spec.")
(put 'ignored-local-variables 'risky-local-variable t)

(defvar hack-local-variables-hook nil
  "Normal hook run after processing a file's local variables specs.
Major modes can use this to examine user-specified local variables
in order to initialize other data structure based on them.")

(defcustom safe-local-variable-values nil
  "List of variable-value pairs that are considered safe.
Each element is a cons cell (VAR . VAL), where VAR is a variable
symbol and VAL is a value that is considered safe.

Also see `ignored-local-variable-values'."
  :risky t
  :group 'find-file
  :type 'alist)

(defcustom ignored-local-variable-values nil
  "List of variable-value pairs that should always be ignored.
Each element is a cons cell (VAR . VAL), where VAR is a variable
symbol and VAL is its value; if VAR is set to VAL by a file-local
variables section, that setting should be ignored.

Also see `safe-local-variable-values'."
  :risky t
  :group 'find-file
  :type 'alist
  :version "28.1")

(defcustom safe-local-eval-forms
  ;; This should be here at least as long as Emacs supports write-file-hooks.
  '((add-hook 'write-file-hooks 'time-stamp)
    (add-hook 'write-file-functions 'time-stamp)
    (add-hook 'before-save-hook 'time-stamp nil t)
    (add-hook 'before-save-hook 'delete-trailing-whitespace nil t))
  "Expressions that are considered safe in an `eval:' local variable.
Add expressions to this list if you want Emacs to evaluate them, when
they appear in an `eval' local variable specification, without first
asking you for confirmation."
  :risky t
  :group 'find-file
  :version "24.1"			; added write-file-hooks
  :type '(repeat sexp))

;; Risky local variables:
(mapc (lambda (var) (put var 'risky-local-variable t))
      '(after-load-alist
	buffer-auto-save-file-name
	buffer-file-name
	buffer-file-truename
	buffer-undo-list
	debugger
	default-text-properties
	eval
	exec-directory
	exec-path
	file-name-handler-alist
	frame-title-format
	global-mode-string
	header-line-format
	icon-title-format
	inhibit-quit
	load-path
	max-lisp-eval-depth
	minor-mode-map-alist
	minor-mode-overriding-map-alist
	mode-line-format
	mode-name
	overriding-local-map
	overriding-terminal-local-map
	process-environment
	standard-input
	standard-output
	unread-command-events))

;; Safe local variables:
;;
;; For variables defined by major modes, the safety declarations can go into
;; the major mode's file, since that will be loaded before file variables are
;; processed.
;;
;; For variables defined by minor modes, put the safety declarations in the
;; file defining the minor mode after the defcustom/defvar using an autoload
;; cookie, e.g.:
;;
;;   ;;;###autoload(put 'variable 'safe-local-variable 'stringp)
;;
;; Otherwise, when Emacs visits a file specifying that local variable, the
;; minor mode file may not be loaded yet.
;;
;; For variables defined in the C source code the declaration should go here:

(dolist (pair
	 '((buffer-read-only        . booleanp)	;; C source code
	   (default-directory       . stringp)	;; C source code
	   (fill-column             . integerp)	;; C source code
	   (indent-tabs-mode        . booleanp)	;; C source code
	   (left-margin             . integerp)	;; C source code
	   (inhibit-compacting-font-caches . booleanp) ;; C source code
	   (no-update-autoloads     . booleanp)
	   (lexical-binding	 . booleanp)	  ;; C source code
	   (tab-width               . integerp)	  ;; C source code
	   (truncate-lines          . booleanp)	  ;; C source code
	   (word-wrap               . booleanp)	  ;; C source code
	   (bidi-display-reordering . booleanp))) ;; C source code
  (put (car pair) 'safe-local-variable (cdr pair)))

(put 'bidi-paragraph-direction 'safe-local-variable
     (lambda (v) (memq v '(nil right-to-left left-to-right))))

;; Predicate functions referenced by the safe-local-variable puts below.
(defun list-of-strings-p (object)
  "Return t if OBJECT is nil or a list of strings."
  (while (and (consp object) (stringp (car object)))
    (setq object (cdr object)))
  (null object))
(defun c-string-list-p (val)
  "Return non-nil if VAL is a list of strings."
  (and (listp val)
       (catch 'string
	 (dolist (elt val)
	   (if (not (stringp elt))
	       (throw 'string nil)))
	 t)))
(defun version-control-safe-local-p (x)
  "Return whether X is safe as local value for `version-control'."
  (or (booleanp x) (equal x 'never)))
(defun time-stamp-zone-type-p (zone)
  "Return non-nil if ZONE looks like a valid timezone rule."
  (or (memq zone '(nil t wall))
      (stringp zone)
      (and (consp zone)
           (integerp (car zone)))))
(defun vc--safe-branch-regexps-p (val)
  "Return non-nil if VAL is a safe local value for `vc-*-branch-regexps'."
  (or (eq val t)
      (and (listp val)
           (seq-every-p (lambda (elt)
                          (or (symbolp elt) (stringp elt)))
                        val))))

;; GNU's runtime safe-local-variable symbol properties (from autoload
;; cookies, :safe defcustom keywords, and files.el/cus-start.el puts).
(put 'Info-documentlanguage 'safe-local-variable 'symbolp)
(put 'abbrev-mode 'safe-local-variable 'booleanp)
(put 'allout-distinctive-bullets-string 'safe-local-variable 'stringp)
(put 'allout-file-xref-bullet 'safe-local-variable 'string-or-null-p)
(put 'allout-header-prefix 'safe-local-variable 'stringp)
(put 'allout-numbered-bullet 'safe-local-variable 'string-or-null-p)
(put 'allout-old-style-prefixes 'safe-local-variable 'booleanp)
(put 'allout-plain-bullets-string 'safe-local-variable 'stringp)
(put 'allout-presentation-padding 'safe-local-variable 'integerp)
(put 'allout-primary-bullet 'safe-local-variable 'stringp)
(put 'allout-show-bodies 'safe-local-variable 'booleanp)
(put 'allout-stylish-prefixes 'safe-local-variable 'booleanp)
(put 'allout-use-hanging-indents 'safe-local-variable 'booleanp)
(put 'allout-widgets-mode-inhibit 'safe-local-variable 'booleanp)
(put 'auto-fill-function 'safe-local-variable 'null)
(put 'auto-insert 'safe-local-variable 'null)
(put 'autoload-compute-prefixes 'safe-local-variable 'booleanp)
(put 'bidi-display-reordering 'safe-local-variable 'booleanp)
(put 'buffer-read-only 'safe-local-variable 'booleanp)
(put 'bug-reference-bug-regexp 'safe-local-variable 'stringp)
(put 'byte-compile-dynamic 'safe-local-variable 'booleanp)
(put 'byte-compile-dynamic-docstrings 'safe-local-variable 'booleanp)
(put 'byte-compile-error-on-warn 'safe-local-variable 'booleanp)
(put 'c++-font-lock-extra-types 'safe-local-variable 'c-string-list-p)
(put 'c-backslash-column 'safe-local-variable 'integerp)
(put 'c-basic-offset 'safe-local-variable 'integerp)
(put 'c-file-style 'safe-local-variable 'string-or-null-p)
(put 'c-font-lock-extra-types 'safe-local-variable 'c-string-list-p)
(put 'change-log-default-name 'safe-local-variable 'string-or-null-p)
(put 'checkdoc-allow-quoting-nil-and-t 'safe-local-variable 'booleanp)
(put 'checkdoc-arguments-in-order-flag 'safe-local-variable 'booleanp)
(put 'checkdoc-arguments-missing-flag 'safe-local-variable 'booleanp)
(put 'checkdoc-common-verbs-regexp 'safe-local-variable 'stringp)
(put 'checkdoc-force-docstrings-flag 'safe-local-variable 'booleanp)
(put 'checkdoc-force-history-flag 'safe-local-variable 'booleanp)
(put 'checkdoc-ispell-list-words 'safe-local-variable 'list-of-strings-p)
(put 'checkdoc-package-keywords-flag 'safe-local-variable 'booleanp)
(put 'checkdoc-permit-comma-termination-flag 'safe-local-variable 'booleanp)
(put 'checkdoc-proper-noun-regexp 'safe-local-variable 'stringp)
(put 'checkdoc-spellcheck-documentation-flag 'safe-local-variable 'booleanp)
(put 'checkdoc-symbol-words 'safe-local-variable 'list-of-strings-p)
(put 'checkdoc-verb-check-experimental-flag 'safe-local-variable 'booleanp)
(put 'colon-double-space 'safe-local-variable 'booleanp)
(put 'comment-column 'safe-local-variable 'integerp)
(put 'comment-end 'safe-local-variable 'stringp)
(put 'comment-end-skip 'safe-local-variable 'stringp)
(put 'comment-multi-line 'safe-local-variable 'booleanp)
(put 'comment-start 'safe-local-variable 'string-or-null-p)
(put 'comment-start-skip 'safe-local-variable 'stringp)
(put 'compilation-directory 'safe-local-variable 'stringp)
(put 'copyright-at-end-flag 'safe-local-variable 'booleanp)
(put 'copyright-names-regexp 'safe-local-variable 'stringp)
(put 'copyright-year-ranges 'safe-local-variable 'booleanp)
(put 'cperl-brace-offset 'safe-local-variable 'integerp)
(put 'cperl-continued-brace-offset 'safe-local-variable 'integerp)
(put 'cperl-continued-statement-offset 'safe-local-variable 'integerp)
(put 'cperl-extra-newline-before-brace 'safe-local-variable 'booleanp)
(put 'cperl-file-style 'safe-local-variable 'stringp)
(put 'cperl-indent-level 'safe-local-variable 'integerp)
(put 'cperl-label-offset 'safe-local-variable 'integerp)
(put 'cperl-merge-trailing-else 'safe-local-variable 'booleanp)
(put 'create-lockfiles 'safe-local-variable 'booleanp)
(put 'default-directory 'safe-local-variable 'stringp)
(put 'default-justification 'safe-local-variable 'symbolp)
(put 'diff-add-log-use-relative-names 'safe-local-variable 'booleanp)
(put 'display-fill-column-indicator 'safe-local-variable 'booleanp)
(put 'electric-quote-comment 'safe-local-variable 'booleanp)
(put 'electric-quote-context-sensitive 'safe-local-variable 'booleanp)
(put 'electric-quote-paragraph 'safe-local-variable 'booleanp)
(put 'electric-quote-replace-consecutive 'safe-local-variable 'booleanp)
(put 'electric-quote-replace-double 'safe-local-variable 'booleanp)
(put 'electric-quote-string 'safe-local-variable 'booleanp)
(put 'enable-character-translation 'safe-local-variable 'booleanp)
(put 'fill-column 'safe-local-variable 'integerp)
(put 'fill-prefix 'safe-local-variable 'string-or-null-p)
(put 'generated-autoload-file 'safe-local-variable 'stringp)
(put 'generated-autoload-load-name 'safe-local-variable 'stringp)
(put 'goto-line-history-local 'safe-local-variable 'booleanp)
(put 'idl-font-lock-extra-types 'safe-local-variable 'c-string-list-p)
(put 'indent-tabs-mode 'safe-local-variable 'booleanp)
(put 'inhibit-compacting-font-caches 'safe-local-variable 'booleanp)
(put 'ispell-local-dictionary 'safe-local-variable 'string-or-null-p)
(put 'ispell-local-pdict 'safe-local-variable 'stringp)
(put 'java-font-lock-extra-types 'safe-local-variable 'c-string-list-p)
(put 'kept-new-versions 'safe-local-variable 'natnump)
(put 'kept-old-versions 'safe-local-variable 'natnump)
(put 'left-margin 'safe-local-variable 'integerp)
(put 'less-css-compile-at-save 'safe-local-variable 'booleanp)
(put 'less-css-input-file-name 'safe-local-variable 'stringp)
(put 'less-css-lessc-options 'safe-local-variable 't)
(put 'less-css-output-directory 'safe-local-variable 'stringp)
(put 'lexical-binding 'safe-local-variable 'booleanp)
(put 'lisp-body-indent 'safe-local-variable 'integerp)
(put 'next-error-verbose 'safe-local-variable 'booleanp)
(put 'no-byte-compile 'safe-local-variable 'booleanp)
(put 'no-native-compile 'safe-local-variable 'booleanp)
(put 'no-update-autoloads 'safe-local-variable 'booleanp)
(put 'objc-font-lock-extra-types 'safe-local-variable 'c-string-list-p)
(put 'outline-heading-end-regexp 'safe-local-variable 'stringp)
(put 'outline-regexp 'safe-local-variable 'stringp)
(put 'page-delimiter 'safe-local-variable 'stringp)
(put 'paragraph-ignore-fill-prefix 'safe-local-variable 'booleanp)
(put 'paragraph-separate 'safe-local-variable 'stringp)
(put 'paragraph-start 'safe-local-variable 'stringp)
(put 'perl-brace-imaginary-offset 'safe-local-variable 'integerp)
(put 'perl-brace-offset 'safe-local-variable 'integerp)
(put 'perl-continued-brace-offset 'safe-local-variable 'integerp)
(put 'perl-continued-statement-offset 'safe-local-variable 'integerp)
(put 'perl-indent-level 'safe-local-variable 'integerp)
(put 'perl-label-offset 'safe-local-variable 'integerp)
(put 'pike-font-lock-extra-types 'safe-local-variable 'c-string-list-p)
(put 'project-kill-buffers-display-buffer-list 'safe-local-variable 'booleanp)
(put 'project-vc-include-untracked 'safe-local-variable 'booleanp)
(put 'project-vc-merge-submodules 'safe-local-variable 'booleanp)
(put 'project-vc-name 'safe-local-variable 'stringp)
(put 'read-symbol-shorthands 'safe-local-variable 'consp)
(put 'reftex-guess-label-type 'safe-local-variable 'booleanp)
(put 'reftex-level-indent 'safe-local-variable 'integerp)
(put 'require-final-newline 'safe-local-variable 'symbolp)
(put 'sentence-end 'safe-local-variable 'string-or-null-p)
(put 'sentence-end-base 'safe-local-variable 'stringp)
(put 'sentence-end-double-space 'safe-local-variable 'booleanp)
(put 'sentence-end-without-period 'safe-local-variable 'booleanp)
(put 'sentence-end-without-space 'safe-local-variable 'stringp)
(put 'sh-shell 'safe-local-variable 'symbolp)
(put 'show-paren-predicate 'safe-local-variable 'booleanp)
(put 'show-trailing-whitespace 'safe-local-variable 'booleanp)
(put 'sort-fold-case 'safe-local-variable 'booleanp)
(put 'sort-numeric-base 'safe-local-variable 'integerp)
(put 'tab-stop-list 'safe-local-variable 'listp)
(put 'tab-width 'safe-local-variable 'integerp)
(put 'tags-case-fold-search 'safe-local-variable 'symbolp)
(put 'tags-file-name 'safe-local-variable 'stringp)
(put 'time-stamp-end 'safe-local-variable 'stringp)
(put 'time-stamp-format 'safe-local-variable 'stringp)
(put 'time-stamp-inserts-lines 'safe-local-variable 'booleanp)
(put 'time-stamp-line-limit 'safe-local-variable 'integerp)
(put 'time-stamp-pattern 'safe-local-variable 'stringp)
(put 'time-stamp-start 'safe-local-variable 'stringp)
(put 'time-stamp-time-zone 'safe-local-variable 'time-stamp-zone-type-p)
(put 'truncate-lines 'safe-local-variable 'booleanp)
(put 'vc-default-patch-addressee 'safe-local-variable 'stringp)
(put 'vc-follow-symlinks 'safe-local-variable 'null)
(put 'vc-prepare-patches-separately 'safe-local-variable 'booleanp)
(put 'vc-topic-branch-regexps 'safe-local-variable 'vc--safe-branch-regexps-p)
(put 'vc-trunk-branch-regexps 'safe-local-variable 'vc--safe-branch-regexps-p)
(put 'version-control 'safe-local-variable 'version-control-safe-local-p)
(put 'word-wrap 'safe-local-variable 'booleanp)
;; GNU's safe-local-eval-function properties.
(put 'c-set-style 'safe-local-eval-function t)
(put 'goto-address 'safe-local-eval-function t)


(defvar-local file-local-variables-alist nil
  "Alist of file-local variable settings in the current buffer.
Each element in this list has the form (VAR . VALUE), where VAR
is a file-local variable (a symbol) and VALUE is the value
specified.  The actual value in the buffer may differ from VALUE,
if it is changed by the major or minor modes, or by the user.")
(put 'file-local-variables-alist 'permanent-local t)

(defvar-local dir-local-variables-alist nil
  "Alist of directory-local variable settings in the current buffer.
Each element in this list has the form (VAR . VALUE), where VAR
is a directory-local variable (a symbol) and VALUE is the value
specified in .dir-locals.el.  The actual value in the buffer
may differ from VALUE, if it is changed by the major or minor modes,
or by the user.")

(defvar before-hack-local-variables-hook nil
  "Normal hook run before setting file-local variables.
It is called after checking for unsafe/risky variables and
setting `file-local-variables-alist', and before applying the
variables stored in `file-local-variables-alist'.  A hook
function is allowed to change the contents of this alist.

This hook is called only if there is at least one file-local
variable to set.")

(defvar permanently-enabled-local-variables
  '(lexical-binding read-symbol-shorthands)
  "A list of file-local variables that are always enabled.
This overrides any `enable-local-variables' setting.")

(defcustom safe-local-variable-directories '()
  "A list of directories where local variables are always enabled.
Directory-local variables loaded from these directories, such as the
variables in .dir-locals.el, will be enabled even if they are risky.
The names of the directories in the list must be absolute, and must
end in a slash.  Remote directories can be included if the
variable `enable-remote-dir-locals' is non-nil."
  :version "30.1"
  :type '(repeat string)
  :risky t
  :group 'find-file)

(defun hack-local-variables-confirm (all-vars unsafe-vars risky-vars dir-name)
  "Get confirmation before setting up local variable values.
ALL-VARS is the list of all variables to be set up.
UNSAFE-VARS is the list of those that aren't marked as safe or risky.
RISKY-VARS is the list of those that are marked as risky.
If these settings come from directory-local variables, then
DIR-NAME is the name of the associated directory.  Otherwise it is nil."
  (unless noninteractive
    (let ((name (cond (dir-name)
		      (buffer-file-name
		       (file-name-nondirectory buffer-file-name))
		      ((concat "buffer " (buffer-name)))))
	  (offer-save (and (eq enable-local-variables t)
			   unsafe-vars))
	  (buf (get-buffer-create "*Local Variables*")))
      ;; Set up the contents of the *Local Variables* buffer.
      (with-current-buffer buf
	(erase-buffer)
	(cond
	 (unsafe-vars
	  (insert "The local variables list in " name
		  "\nor .dir-locals.el contains values that may not be safe (*)"
		  (if risky-vars
		      ", and variables that are risky (**)."
		    ".")))
	 (risky-vars
	  (insert "The local variables list in " name
		  "\ncontains variables that are risky (**)."))
	 (t
	  (insert "A local variables list is specified in " name ".")))
	(insert "\n\nDo you want to apply it?  You can type
y  -- to apply the local variables list.
n  -- to ignore the local variables list.")
	(if offer-save
	    (insert "
!  -- to apply the local variables list, and permanently mark these
      values (*) as safe (in the future, they will be set automatically.)
i  -- to ignore the local variables list, and permanently mark these
      values (*) as ignored"
                    (if dir-name "
+  -- to apply the local variables list, and trust all directory-local
      variables in this directory\n\n"
                      "\n\n"))
	  (insert "\n\n"))
	(dolist (elt all-vars)
	  (cond ((member elt unsafe-vars)
		 (insert "  * "))
		((member elt risky-vars)
		 (insert " ** "))
		(t
		 (insert "    ")))
	  (princ (car elt) buf)
	  (insert " : ")
	  ;; Make strings with embedded whitespace easier to read.
	  (let ((print-escape-newlines t))
	    (prin1 (cdr elt) buf))
	  (insert "\n"))
        (setq-local cursor-type nil)
	(set-buffer-modified-p nil)
	(goto-char (point-min)))

      ;; Display the buffer and read a choice.
      (save-window-excursion
	(pop-to-buffer buf '(display-buffer--maybe-at-bottom))
	(let* ((exit-chars '(?y ?n ?\s))
	       (prompt (format "Please type %s%s: "
			       (if offer-save
                                   (if dir-name
                                       "y, n, !, i, +"
                                     "y, n, !, i")
                                 "y or n")
			       (if (< (line-number-at-pos (point-max))
				      (window-body-height))
				   ""
				 ", or C-v/M-v to scroll")))
	       char)
	  (when offer-save
            (push ?i exit-chars)
            (push ?! exit-chars)
            (when dir-name
              (push ?+ exit-chars)))
	  (setq char (read-char-choice prompt exit-chars))
          (when (and offer-save dir-name (= char ?+))
            (customize-push-and-save 'safe-local-variable-directories
                                     (list dir-name)))
	  (when (and offer-save
                     (or (= char ?!) (= char ?i))
                     unsafe-vars)
	    (customize-push-and-save
             (if (= char ?!)
                 'safe-local-variable-values
               'ignored-local-variable-values)
             unsafe-vars))
	  (prog1 (memq char '(?! ?\s ?y ?+))
	    (quit-window t)))))))

(defconst hack-local-variable-regexp
  "[ \t]*\\([^][;\"'?()\\ \t\n]+\\)[ \t]*:[ \t]*")

(defun hack-local-variables-prop-line (&optional handle-mode)
  "Return local variables specified in the -*- line.
Usually returns an alist of elements (VAR . VAL), where VAR is a
variable and VAL is the specified value.  Ignores any
specification for `coding:', and sometimes for `mode' (which
should have already been handled by `set-auto-coding' and
`set-auto-mode', respectively).  Return nil if the -*- line is
malformed.

If HANDLE-MODE is nil, we return the alist of all the local
variables in the line except `coding' as described above.  If it
is neither nil nor t, we do the same, except that any settings of
`mode' and `coding' are ignored.  If HANDLE-MODE is t, we ignore
all settings in the line except for `mode', which \(if present) we
return as the symbol specifying the mode."
  (catch 'malformed-line
    (save-excursion
      (goto-char (point-min))
      (let ((end (set-auto-mode-1))
	    result)
	(cond ((not end)
	       nil)
	      ((looking-at "[ \t]*\\([^ \t\n\r:;]+\\)\\([ \t]*-\\*-\\)")
	       ;; Simple form: "-*- MODENAME -*-".
	       (if (eq handle-mode t)
		   (intern (concat (match-string 1) "-mode"))))
	      (t
	       ;; Hairy form: '-*-' [ <variable> ':' <value> ';' ]* '-*-'
	       ;; (last ";" is optional).
	       ;; If HANDLE-MODE is t, just check for `mode'.
	       ;; Otherwise, parse the -*- line into the RESULT alist.
	       (while (not (or (and (eq handle-mode t) result)
                               (>= (point) end)))
		 (unless (looking-at hack-local-variable-regexp)
		   (message "Malformed mode-line: %S in buffer %S"
                            (buffer-substring-no-properties (point) end) (buffer-name))
		   (throw 'malformed-line nil))
		 (goto-char (match-end 0))
		 ;; There used to be a downcase here,
		 ;; but the manual didn't say so,
		 ;; and people want to set var names that aren't all lc.
		 (let* ((key (intern (match-string 1)))
			(val (save-restriction
			       (narrow-to-region (point) end)
                               ;; As a defensive measure, we do not allow
                               ;; circular data in the file-local data.
			       (let ((read-circle nil))
				 (read (current-buffer)))))
			;; It is traditional to ignore
			;; case when checking for `mode' in set-auto-mode,
			;; so we must do that here as well.
			;; That is inconsistent, but we're stuck with it.
			;; The same can be said for `coding' in set-auto-coding.
			(keyname (downcase (symbol-name key))))
                   (cond
                    ((eq handle-mode t)
                     (and (equal keyname "mode")
                          (setq result
                                (intern (concat (downcase (symbol-name val))
                                                "-mode")))))
                    ((equal keyname "coding"))
                    (t
                     (when (or (not handle-mode)
                               (not (equal keyname "mode")))
                       (condition-case nil
                           (push (cons (cond ((eq key 'eval) 'eval)
                                             ;; Downcase "Mode:".
                                             ((equal keyname "mode") 'mode)
                                             (t (indirect-variable key)))
                                       val)
                                 result)
                         (error nil)))))
		   (skip-chars-forward " \t;")))
	       result))))))

(defun hack-local-variables-filter (variables dir-name)
  "Filter local variable settings, querying the user if necessary.
VARIABLES is the alist of variable-value settings.  This alist is
 filtered based on the values of `ignored-local-variables',
 `enable-local-eval', `enable-local-variables', and (if necessary)
 user interaction.  The results are added to
 `file-local-variables-alist', without applying them.
If these settings come from directory-local variables, then
DIR-NAME is the name of the associated directory.  Otherwise it is nil."
  ;; Find those variables that we may want to save to
  ;; `safe-local-variable-values'.
  (let (all-vars risky-vars unsafe-vars)
    (dolist (elt variables)
      (let ((var (car elt))
	    (val (cdr elt)))
	(cond ((memq var ignored-local-variables)
	       ;; Ignore any variable in `ignored-local-variables'.
	       nil)
              ;; Ignore variables with the specified values.
              ((member elt ignored-local-variable-values)
               nil)
	      ;; Obey `enable-local-eval'.
	      ((eq var 'eval)
	       (when enable-local-eval
		 (let ((safe (or (hack-one-local-variable-eval-safep val)
				 ;; In case previously marked safe (bug#5636).
				 (safe-local-variable-p var val))))
		   ;; If not safe and e-l-v = :safe, ignore totally.
		   (when (or safe (not (eq enable-local-variables :safe)))
		     (push elt all-vars)
		     (or (eq enable-local-eval t)
			 safe
			 (push elt unsafe-vars))))))
	      ;; Ignore duplicates (except `mode') in the present list.
	      ((and (assq var all-vars) (not (eq var 'mode))) nil)
	      ;; Accept known-safe variables.
	      ((or (memq var '(mode unibyte coding))
		   (safe-local-variable-p var val))
	       (push elt all-vars))
	      ;; The variable is either risky or unsafe:
	      ((not (eq enable-local-variables :safe))
	       (push elt all-vars)
	       (if (risky-local-variable-p var val)
		   (push elt risky-vars)
		 (push elt unsafe-vars))))))
    (and all-vars
	 ;; Query, unless all vars are safe or user wants no querying.
	 (or (and (eq enable-local-variables t)
		  (null unsafe-vars)
		  (null risky-vars))
	     (memq enable-local-variables '(:all :safe))
             (delq nil (mapcar (lambda (dir)
                                 (and dir-name dir
                                      (file-equal-p dir dir-name)))
                               safe-local-variable-directories))
	     (hack-local-variables-confirm all-vars unsafe-vars
					   risky-vars dir-name))
	 (dolist (elt all-vars)
	   (unless (memq (car elt) '(eval mode))
	     (unless dir-name
	       (setq dir-local-variables-alist
		     (assq-delete-all (car elt) dir-local-variables-alist)))
	     (setq file-local-variables-alist
		   (assq-delete-all (car elt) file-local-variables-alist)))
	   (push elt file-local-variables-alist)))))

;; TODO?  Warn once per file rather than once per session?
(defvar hack-local-variables--warned-lexical nil)

(defun hack-local-variables (&optional handle-mode inhibit-locals)
  "Parse and put into effect this buffer's local variables spec.
Also puts into effect directory-local variables.
For buffers not visiting files, apply the directory-local variables that
would be applicable to files in `default-directory'.

Uses `hack-local-variables-apply' and `hack-dir-local-variables'
to apply the variables.

If `enable-local-variables' or `local-enable-local-variables' is
nil, or INHIBIT-LOCALS is non-nil, this function disregards all
normal local variables.  If `inhibit-local-variables-regexps'
applies to the file in question, the file is not scanned for
local variables, but directory-local variables may still be
applied.

Variables present in `permanently-enabled-local-variables' will
still be evaluated, even if local variables are otherwise
inhibited.

If HANDLE-MODE is t, the function only checks whether a \"mode:\"
is specified, and returns the corresponding mode symbol, or nil.
In this case, try to ignore minor-modes, and return only a major-mode.
If HANDLE-MODE is nil, the function gathers all the specified local
variables.  If HANDLE-MODE is neither nil nor t, the function gathers
all the specified local variables, but ignores any settings of \"mode:\"."
  ;; We don't let inhibit-local-variables-p influence the value of
  ;; enable-local-variables, because then it would affect dir-local
  ;; variables.  We don't want to search eg tar files for file local
  ;; variable sections, but there is no reason dir-locals cannot apply
  ;; to them.  The real meaning of inhibit-local-variables-p is "do
  ;; not scan this file for local variables".
  (let ((enable-local-variables
	 (and (not inhibit-locals)
              local-enable-local-variables enable-local-variables)))
    (if (eq handle-mode t)
        ;; We're looking just for the major mode setting.
        (and enable-local-variables
             (not (inhibit-local-variables-p))
	     ;; If HANDLE-MODE is t, and the prop line specifies a
	     ;; mode, then we're done, and have no need to scan further.
             (or (hack-local-variables-prop-line t)
                 ;; Look for the mode elsewhere in the buffer.
                 (hack-local-variables--find-variables t)))
      ;; Normal handling of local variables.
      (setq file-local-variables-alist nil)
      (when (and (file-remote-p default-directory)
                 (fboundp 'hack-connection-local-variables)
                 (fboundp 'connection-local-criteria-for-default-directory))
        (with-demoted-errors "Connection-local variables error: %s"
	  ;; Note this is a no-op if enable-local-variables is nil.
	  (hack-connection-local-variables
           (connection-local-criteria-for-default-directory))))
      (with-demoted-errors "Directory-local variables error: %s"
	;; Note this is a no-op if enable-local-variables is nil.
	(hack-dir-local-variables))
      (let ((result (append (hack-local-variables--find-variables handle-mode)
                            (hack-local-variables-prop-line handle-mode))))
        (if (and enable-local-variables
                 (not (inhibit-local-variables-p)))
            (progn
	      ;; Set the variables.
	      (hack-local-variables-filter result nil)
	      (hack-local-variables-apply))
          ;; Handle `lexical-binding' and other special local
          ;; variables.
          (dolist (variable permanently-enabled-local-variables)
            (when-let* ((elem (assq variable result)))
              (push elem file-local-variables-alist)))
          (hack-local-variables-apply))))))

(defun internal--get-default-lexical-binding (from)
  (let ((mib (lambda (node) (buttonize node (lambda (_) (info node))
                                  nil "mouse-2: Jump to Info node"))))
    (or (and (bufferp from) (zerop (buffer-size from)))
        (and (stringp from)
             (eql 0 (file-attribute-size (file-attributes from))))
        (let ((source
               (if (not (and (bufferp from)
                             (string-match-p "\\` \\*load\\*\\(-[0-9]+\\)?\\'"
                                             (buffer-name from))
                             load-file-name))
                   from
                 (abbreviate-file-name load-file-name))))
          (condition-case nil
              (display-warning
               `(files missing-lexbind-cookie
                       ,(if (bufferp source) 'eval-buffer source))
               (format-message "Missing `lexical-binding' cookie in %S.
You can add one with `M-x %s RET'.
See `%s' and `%s'
for more information."
                               source
                               (buttonize "elisp-enable-lexical-binding"
                                          (lambda (_)
                                            (pop-to-buffer
                                             (if (bufferp source) source
                                               (find-file-noselect source)))
                                            (call-interactively
                                             #'elisp-enable-lexical-binding))
                                          nil "mouse-2: Add cookie")
                               (funcall mib "(elisp)Selecting Lisp Dialect")
                               (funcall mib "(elisp)Converting to Lexical Binding"))
               :warning)
            ;; In various corner-case situations, `display-warning' may
            ;; fail (e.g. not yet defined, or can't be (auto)loaded),
            ;; so use a simple fallback that won't get in the way.
            (error
             ;; But not if this particular warning is disabled.
             (unless (equal warning-inhibit-types
                            '((files missing-lexbind-cookie)))
               (message "Missing `lexical-binding' cookie in %S" source))))))
    (default-toplevel-value 'lexical-binding)))

(setq internal--get-default-lexical-binding-function
      #'internal--get-default-lexical-binding)

(defun hack-local-variables--find-variables (&optional handle-mode)
  "Return all local variables in the current buffer.
If HANDLE-MODE is nil, we gather all the specified local
variables.  If HANDLE-MODE is neither nil nor t, we do the same,
except that any settings of `mode' are ignored.

If HANDLE-MODE is t, all we do is check whether a \"mode:\"
is specified, and return the corresponding mode symbol, or nil.
In this case, we try to ignore minor-modes, and return only a
major-mode."
  (let ((result nil))
    ;; Look for "Local variables:" line in last page.
    (save-excursion
      (goto-char (point-max))
      (search-backward "\n\^L" (max (- (point-max) 3000) (point-min))
		       'move)
      (when (let ((case-fold-search t))
	      (search-forward "Local Variables:" nil t))
        (skip-chars-forward " \t")
        ;; suffix is what comes after "local variables:" in its line.
        ;; prefix is what comes before "local variables:" in its line.
        (let ((suffix
	       (concat
	        (regexp-quote (buffer-substring (point)
					        (line-end-position)))
	        "$"))
	      (prefix
	       (concat "^" (regexp-quote
			    (buffer-substring (line-beginning-position)
					      (match-beginning 0))))))

	  (forward-line 1)
	  (let ((startpos (point))
	        endpos
                (selective-p (eq selective-display t))
	        (thisbuf (current-buffer)))
	    (save-excursion
	      (unless (let ((case-fold-search t))
		        (re-search-forward
		         (concat prefix "[ \t]*End:[ \t]*" suffix)
		         nil t))
	        ;; This used to be an error, but really all it means is
	        ;; that this may simply not be a local-variables section,
	        ;; so just ignore it.
	        (message "Local variables list is not properly terminated"))
	      (beginning-of-line)
	      (setq endpos (point)))

	    (with-temp-buffer
	      (insert-buffer-substring thisbuf startpos endpos)
	      (goto-char (point-min))
              (if selective-p
	          (subst-char-in-region (point) (point-max) ?\r ?\n))
	      (while (not (eobp))
	        ;; Discard the prefix.
	        (if (looking-at prefix)
		    (delete-region (point) (match-end 0))
		  (user-error "Local variables entry is missing the prefix"))
	        (end-of-line)
	        ;; Discard the suffix.
	        (if (looking-back suffix (line-beginning-position))
		    (delete-region (match-beginning 0) (point))
		  (user-error "Local variables entry is missing the suffix"))
	        (forward-line 1))
	      (goto-char (point-min))

	      (while (not (eobp))
	        ;; Find the variable name;
	        (unless (looking-at hack-local-variable-regexp)
                  (user-error "Malformed local variable line: %S"
                              (buffer-substring-no-properties
                               (point) (line-end-position))))
                (goto-char (match-end 1))
	        (let* ((str (match-string 1))
		       (var (intern str))
		       val val2)
		  (and (equal (downcase (symbol-name var)) "mode")
		       (setq var 'mode))
		  ;; Read the variable value.
		  (skip-chars-forward "^:")
		  (forward-char 1)
                  ;; As a defensive measure, we do not allow
                  ;; circular data in the file-local data.
		  (let ((read-circle nil))
		    (setq val (read (current-buffer))))
		  (if (eq handle-mode t)
		      (and (eq var 'mode)
			   ;; Specifying minor-modes via mode: is
			   ;; deprecated, but try to reject them anyway.
			   (not (string-match
			         "-minor\\'"
			         (setq val2 (downcase (symbol-name val)))))
			   (let ((mode (intern (concat val2 "-mode"))))
                             (when (fboundp (major-mode-remap mode))
                               (setq result mode))))
		    (cond ((eq var 'coding))
			  ((eq var 'lexical-binding)
			   (unless hack-local-variables--warned-lexical
			     (setq hack-local-variables--warned-lexical t)
			     (display-warning
                              'files
                              (format-message
                               "%s: `lexical-binding' at end of file unreliable"
                               (file-name-nondirectory
                                ;; We are called from
                                ;; 'with-temp-buffer', so we need
                                ;; to use 'thisbuf's name in the
                                ;; warning message.
                                (or (buffer-file-name thisbuf) ""))))))
                          ((eq var 'read-symbol-shorthands)
                           ;; Sort automatically by shorthand length
                           ;; in descending order.
                           (setq val (sort val
                                           (lambda (sh1 sh2) (> (length (car sh1))
                                                                (length (car sh2))))))
                           (push (cons 'read-symbol-shorthands val) result))
                          ((and (eq var 'mode) handle-mode))
			  (t
			   (ignore-errors
			     (push (cons (if (eq var 'eval)
					     'eval
					   (indirect-variable var))
				         val)
                                   result))))))
	        (forward-line 1)))))))
    result))

(defun hack-local-variables-apply ()
  "Apply the elements of `file-local-variables-alist'.
If there are any elements, runs `before-hack-local-variables-hook',
then calls `hack-one-local-variable' to apply the alist elements one by one.
Finishes by running `hack-local-variables-hook', regardless of whether
the alist is empty or not.

Note that this function ignores a `mode' entry if it specifies the same
major mode as the buffer already has."
  (when file-local-variables-alist
    ;; Any 'evals must run in the Right sequence.
    (setq file-local-variables-alist
	  (nreverse file-local-variables-alist))
    (run-hooks 'before-hack-local-variables-hook)
    (dolist (elt file-local-variables-alist)
      (hack-one-local-variable (car elt) (cdr elt))))
  (run-hooks 'hack-local-variables-hook))

(defun safe-local-variable-p (sym val)
  "Non-nil if SYM is safe as a file-local variable with value VAL.
It is safe if any of these conditions are met:

 * There is a matching entry (SYM . VAL) in the
   `safe-local-variable-values' user option.

 * The `safe-local-variable' property of SYM is a function that
   evaluates to a non-nil value with VAL as an argument."
  (or (member (cons sym val) safe-local-variable-values)
      (let ((safep (get sym 'safe-local-variable)))
        (and (functionp safep)
             ;; If the function signals an error, that means it
             ;; can't assure us that the value is safe.
             (with-demoted-errors "Local variable error: %S"
               (funcall safep val))))))

(defun risky-local-variable-p (sym &optional _ignored)
  "Non-nil if SYM could be dangerous as a file-local variable.
It is dangerous if either of these conditions are met:

 * Its `risky-local-variable' property is non-nil.

 * Its name ends with \"hook(s)\", \"function(s)\", \"form(s)\", \"map\",
   \"program\", \"command(s)\", \"predicate(s)\", \"frame-alist\",
   \"mode-alist\", \"font-lock-(syntactic-)keyword*\",
   \"map-alist\", or \"bindat-spec\"."
  ;; If this is an alias, check the base name.
  (condition-case nil
      (setq sym (indirect-variable sym))
    (error nil))
  (or (get sym 'risky-local-variable)
      (string-match "-hooks?$\\|-functions?$\\|-forms?$\\|-program$\\|\
-commands?$\\|-predicates?$\\|font-lock-keywords$\\|font-lock-keywords\
-[0-9]+$\\|font-lock-syntactic-keywords$\\|-frame-alist$\\|-mode-alist$\\|\
-map$\\|-map-alist$\\|-bindat-spec$" (symbol-name sym))))

(defun hack-one-local-variable-quotep (exp)
  (and (consp exp) (eq (car exp) 'quote) (consp (cdr exp))))

(define-obsolete-function-alias 'hack-one-local-variable-constantp
  #'macroexp-const-p "29.1")

(defun hack-one-local-variable-eval-safep (exp)
  "Return non-nil if it is safe to eval EXP when it is found in a file."
  (or (not (consp exp))
      ;; Detect certain `put' expressions.
      (and (eq (car exp) 'put)
	   (hack-one-local-variable-quotep (nth 1 exp))
	   (hack-one-local-variable-quotep (nth 2 exp))
	   (let ((prop (nth 1 (nth 2 exp)))
		 (val (nth 3 exp)))
	     (cond ((memq prop '(lisp-indent-hook
				 lisp-indent-function
				 scheme-indent-function))
		    ;; Allow only safe values (not functions).
		    (or (numberp val)
			(and (hack-one-local-variable-quotep val)
			     (eq (nth 1 val) 'defun))))
		   ((eq prop 'edebug-form-spec)
		    ;; Allow only indirect form specs.
		    ;; During bootstrapping, edebug-basic-spec might not be
		    ;; defined yet.
                    (and (fboundp 'edebug-basic-spec)
			 (hack-one-local-variable-quotep val)
                         (edebug-basic-spec (nth 1 val)))))))
      ;; Allow expressions that the user requested.
      (member exp safe-local-eval-forms)
      ;; Certain functions can be allowed with safe arguments
      ;; or can specify verification functions to try.
      (and (symbolp (car exp))
	   ;; Allow (minor)-modes calls with no arguments.
	   ;; This obsoletes the use of "mode:" for such things.  (Bug#8613)
	   (or (and (member (cdr exp) '(nil (1) (0) (-1)))
		    (string-match "-mode\\'" (symbol-name (car exp))))
	       (let ((prop (get (car exp) 'safe-local-eval-function)))
		 (cond ((eq prop t)
			(let ((ok t))
			  (dolist (arg (cdr exp))
			    (unless (macroexp-const-p arg)
			      (setq ok nil)))
			  ok))
		       ((functionp prop)
			(funcall prop exp))
		       ((listp prop)
			(let ((ok nil))
			  (dolist (function prop)
			    (if (funcall function exp)
				(setq ok t)))
			  ok))))))))

(defun hack-one-local-variable--obsolete (var)
  (let ((o (get var 'byte-obsolete-variable)))
    (when o
      (let ((instead (nth 0 o))
            (since (nth 2 o)))
        (message "%s is obsolete%s; %s"
                 var (if since (format " (since %s)" since))
                 (if (stringp instead)
                     (substitute-command-keys instead)
                   (format-message "use `%s' instead" instead)))))))

(defvar hack-local-variables--inhibit-eval nil
  "List of `eval' forms to ignore in file/dir local variables.")
(defun hack-one-local-variable (var val)
  "Set local variable VAR with value VAL.
If VAR is `mode', call `VAL-mode' as a function unless it's
already the major mode."
  (cond
   ((and (eq var 'eval) (member val hack-local-variables--inhibit-eval)) nil)
   ((eq var 'mode)
    (let ((mode (intern (concat (downcase (symbol-name val))
                                "-mode"))))
      (set-auto-mode-0 mode t)))
   ((eq var 'eval)
    (when (and (consp val) (eq (car val) 'add-hook)
               (consp (cdr val))
               (hack-one-local-variable-quotep (cadr val)))
      (hack-one-local-variable--obsolete (nth 1 (cadr val))))
    (let ((hack-local-variables--inhibit-eval ;; FIXME: Should be buffer-local!
           (cons val hack-local-variables--inhibit-eval)))
      (save-excursion (eval val t))))
   (t
    (hack-one-local-variable--obsolete var)
    ;; Make sure the string has no text properties.
    ;; Some text properties can get evaluated in various ways,
    ;; so it is risky to put them on with a local variable list.
    (if (stringp val)
        (set-text-properties 0 (length val) nil val))
    (set (make-local-variable var) val))))

(defun macroexp-const-p (exp)
  "Return non-nil if EXP will always evaluate to the same value."
  (cond ((consp exp) (memq (car exp) '(quote function)))
        ((symbolp exp) (or (keywordp exp) (memq exp '(nil t))))
        (t t)))

(defun set-buffer-major-mode (buffer)
  "Set an appropriate major mode for BUFFER.
For the *scratch* buffer, use `initial-major-mode', otherwise
choose the mode specified by the default value of `major-mode'."
  (with-current-buffer buffer
    (funcall (or (default-value 'major-mode) 'fundamental-mode))))

;; files.el: directory-local variables (.dir-locals.el), verbatim GNU
;; except `dir-locals--load-mode-if-needed' (pcase-dolist → dolist) and
;; `dir-locals-read-from-dir' (map-merge-with → remacs--dir-locals-merge).

(defvar dir-locals-class-alist '()
  "Alist mapping directory-local variable classes (symbols) to variable lists.")

(defvar dir-locals-directory-cache '()
  "List of cached directory roots for directory-local variable classes.
Each element in this list has the form (DIR CLASS MTIME).
DIR is the name of the directory.
CLASS is the name of a variable class (a symbol).
MTIME is the recorded modification time of the directory-local
variables file associated with this entry.  This time is a Lisp
timestamp (the same format as `current-time'), and is
used to test whether the cache entry is still valid.
Alternatively, MTIME can be nil, which means the entry is always
considered valid.")

(defsubst dir-locals-get-class-variables (class)
  "Return the variable list for CLASS."
  (cdr (assq class dir-locals-class-alist)))

(defun dir-locals-collect-mode-variables (mode-variables variables)
  "Collect directory-local variables from MODE-VARIABLES.
VARIABLES is the initial list of variables.
Returns the new list."
  (dolist (pair mode-variables variables)
    (let* ((variable (car pair))
	   (value (cdr pair))
	   (slot (assq variable variables)))
      ;; If variables are specified more than once, use only the last.  (Why?)
      ;; The pseudo-variables mode and eval are different (bug#3430).
      (if (and slot (not (memq variable '(mode eval))))
	  (setcdr slot value)
	;; Need a new cons in case we setcdr later.
	(push (cons variable value) variables)))))

(defun dir-locals--load-mode-if-needed (key alist)
  ;; If KEY is an extra parent it may remain not loaded
  ;; (hence with some of its mode-specific vars missing their
  ;; `safe-local-variable' property), leading to spurious
  ;; prompts about unsafe vars (bug#68246).
  (when (and (symbolp key) (autoloadp (indirect-function key)))
    (let ((unsafe nil))
      (dolist (pair alist)
        (let ((var (car pair)))
          (unless (or (memq var '(mode eval))
                      (get var 'safe-local-variable))
            (setq unsafe t))))
      (when unsafe
        (ignore-errors
          (autoload-do-load (indirect-function key)))))))

(defun dir-locals-collect-variables (class-variables root variables
                                                     &optional predicate)
  "Collect entries from CLASS-VARIABLES into VARIABLES.
ROOT is the root directory of the project.
Return the new variables list.
If PREDICATE is given, it is used to test a symbol key in the alist
to see whether it should be considered."
  (let* ((file-name (or (buffer-file-name)
			;; Handle non-file buffers, too.
			(expand-file-name default-directory)))
	 (sub-file-name (if (and file-name
                                 (file-name-absolute-p file-name))
                            ;; FIXME: Why not use file-relative-name?
			    (substring file-name (length root)))))
    (condition-case err
        (dolist (entry class-variables variables)
          (let ((key (car entry)))
            (cond
             ((stringp key)
              ;; Don't include this in the previous condition, because we
              ;; want to filter all strings before the next condition.
              (when (and sub-file-name
                         (>= (length sub-file-name) (length key))
                         (string-prefix-p key sub-file-name))
                (setq variables (dir-locals-collect-variables
                                 (cdr entry) root variables predicate))))
             ((if predicate
                  (funcall predicate key)
                (or (not key)
                    (derived-mode-p key)))
              (let* ((alist (cdr entry))
                     (subdirs (assq 'subdirs alist)))
                (when (or (not subdirs)
                        (progn
                          (setq alist (remq subdirs alist))
                          (cdr-safe subdirs))
                        ;; TODO someone might want to extend this to allow
                        ;; integer values for subdir, where N means
                        ;; variables apply to this directory and N levels
                        ;; below it (0 == nil).
                        (equal root (expand-file-name default-directory)))
                  (dir-locals--load-mode-if-needed key alist)
                    (setq variables (dir-locals-collect-mode-variables
                                     alist variables))))))))
      (error
       ;; The file's content might be invalid (e.g. have a merge conflict), but
       ;; that shouldn't prevent the user from opening the file.
       (message "%s error: %s" dir-locals-file (error-message-string err))
       nil))))

(defun dir-locals-set-directory-class (directory class &optional mtime)
  "Declare that the DIRECTORY root is an instance of CLASS.
DIRECTORY is the name of a directory, a string.
CLASS is the name of a project class, a symbol.
MTIME is either the modification time of the directory-local
variables file that defined this class, or nil.

When a file beneath DIRECTORY is visited, the mode-specific
variables from CLASS are applied to the buffer.  The variables
for a class are defined using `dir-locals-set-class-variables'."
  (setq directory (file-name-as-directory (expand-file-name directory)))
  (unless (assq class dir-locals-class-alist)
    (error "No such class `%s'" (symbol-name class)))
  (push (list directory class mtime) dir-locals-directory-cache))

(defun dir-locals-set-class-variables (class variables)
  "Map the type CLASS to a list of variable settings.
CLASS is the project class, a symbol.  VARIABLES is a list
that declares directory-local variables for the class.
An element in VARIABLES is either of the form:
    (MAJOR-MODE . ALIST)
or
    (DIRECTORY . LIST)

In the first form, MAJOR-MODE is a symbol, and ALIST is an alist
whose elements are of the form (VARIABLE . VALUE).

In the second form, DIRECTORY is a directory name (a string), and
LIST is a list of the form accepted by the function.

When a file is visited, the file's class is found.  A directory
may be assigned a class using `dir-locals-set-directory-class'.
Then variables are set in the file's buffer according to the
VARIABLES list of the class.  The list is processed in order.

* If the element is of the form (MAJOR-MODE . ALIST), and the
  buffer's major mode is derived from MAJOR-MODE (as determined
  by `derived-mode-p'), then all the variables in ALIST are
  applied.  A MAJOR-MODE of nil may be used to match any buffer.
  `make-local-variable' is called for each variable before it is
  set.

* If the element is of the form (DIRECTORY . LIST), and DIRECTORY
  is an initial substring of the file's directory, then LIST is
  applied by recursively following these rules."
  (setf (alist-get class dir-locals-class-alist) variables))

(defconst dir-locals-file ".dir-locals.el"
  "File that contains directory-local variables.
It has to be constant to enforce uniform values across different
environments and users.

A second dir-locals file can be used by a user to specify their
personal dir-local variables even if the current directory
already has a `dir-locals-file' that is shared with other
users (such as in a git repository).  The name of this second
file is derived by appending \"-2\" to the base name of
`dir-locals-file'.  With the default value of `dir-locals-file',
a \".dir-locals-2.el\" file in the same directory will override
the \".dir-locals.el\".

See Info node `(elisp)Directory Local Variables' for details.")

(defun dir-locals--all-files (directory &optional base-el-only)
  "Return a list of all readable dir-locals files in DIRECTORY.
The returned list is sorted by increasing priority.  That is,
values specified in the last file should take precedence over
those in the first."
  (when (file-readable-p directory)
    (let* ((file-1 (expand-file-name dir-locals-file directory))
           (file-2 (when (string-match "\\.el\\'" file-1)
                     (replace-match "-2.el" t nil file-1)))
           out)
      (dolist (f (or (and base-el-only (list file-1))
                     ;; The order here is important.
                     (list file-2 file-1)))
        (when (and f
                   (file-readable-p f)
                   (file-regular-p f))
          (push f out)))
      out)))

(defun dir-locals--base-file (directory)
  "Return readable `dir-locals-file' in DIRECTORY, or nil."
  (dir-locals--all-files directory 'base-el-only))

(defun dir-locals-find-file (file)
  "Find the directory-local variables for FILE.
This searches upward in the directory tree from FILE.
It stops at the first directory that has been registered in
`dir-locals-directory-cache' or contains a `dir-locals-file'.
If it finds an entry in the cache, it checks that it is valid.
A cache entry with no modification time element (normally, one that
has been assigned directly using `dir-locals-set-directory-class', not
set from a file) is always valid.
A cache entry based on a `dir-locals-file' is valid if the modification
time stored in the cache matches the current file modification time.
If not, the cache entry is cleared so that the file will be re-read.

This function returns either:
  - nil (no directory local variables found),
  - the matching entry from `dir-locals-directory-cache' (a list),
  - or the full path to the directory (a string) containing at
    least one `dir-locals-file' in the case of no valid cache
    entry."
  (setq file (expand-file-name file))
  (let* ((locals-dir (locate-dominating-file (file-name-directory file)
                                             #'dir-locals--base-file))
         dir-elt)
    ;; `locate-dominating-file' may have abbreviated the name.
    (when locals-dir
      (setq locals-dir (expand-file-name locals-dir)))
    ;; Find the best cached value in `dir-locals-directory-cache'.
    (dolist (elt dir-locals-directory-cache)
      (when (and (string-prefix-p (car elt) file)
                 (> (length (car elt)) (length (car dir-elt))))
        (setq dir-elt elt)))
    (if (and dir-elt
             (or (null locals-dir)
                 (<= (length locals-dir)
                     (length (car dir-elt)))))
        ;; Found a potential cache entry.  Check validity.
        ;; A cache entry with no MTIME is assumed to always be valid
        ;; (ie, set directly, not from a dir-locals file).
        ;; Note, we don't bother to check that there is a matching class
        ;; element in dir-locals-class-alist, since that's done by
        ;; dir-locals-set-directory-class.
        (if (or (null (nth 2 dir-elt))
                (let ((cached-files (dir-locals--all-files (car dir-elt))))
                  ;; The entry MTIME should match the most recent
                  ;; MTIME among matching files.
                  (and cached-files
		       (time-equal-p
			      (nth 2 dir-elt)
			      (let ((latest 0))
				(dolist (f cached-files latest)
				  (let ((f-time
					 (file-attribute-modification-time
					  (file-attributes f))))
				    (if (time-less-p latest f-time)
					(setq latest f-time)))))))))
            ;; This cache entry is OK.
            dir-elt
          ;; This cache entry is invalid; clear it.
          (setq dir-locals-directory-cache
                (delq dir-elt dir-locals-directory-cache))
          ;; Return the first existing dir-locals file.  Might be the same
          ;; as dir-elt's, might not (eg latter might have been deleted).
          locals-dir)
      ;; No cache entry.
      locals-dir)))

(defun dir-locals--get-sort-score (node)
  "Return a number used for sorting the definitions of dir locals.
NODE is assumed to be a cons cell where the car is either a
string or a symbol representing a mode name.

If it is a mode then the depth of the mode (ie, how many parents
that mode has) will be returned.

If it is a string then the length of the string plus 1000 will be
returned.

Otherwise it returns -1.

That way the value can be used to sort the list such that deeper
modes will be after the other modes.  This will be followed by
directory entries in order of length.  If the entries are all
applied in order then that means the more specific modes will
  override the values specified by the earlier modes and directory
variables will override modes."
  (let ((key (car node)))
    (cond ((null key) -1)
          ((symbolp key) (length (derived-mode-all-parents key)))
          ((stringp key)
           (+ 1000 (length key)))
          (t -2))))

(defun dir-locals--sort-variables (variables)
  "Sort VARIABLES so that applying them in order has the right effect.
The variables are compared by `dir-locals--get-sort-score'.
Directory entries are then recursively sorted using the same
criteria."
  (setq variables (sort variables
                        (lambda (a b)
                          (< (dir-locals--get-sort-score a)
                             (dir-locals--get-sort-score b)))))
  (dolist (n variables)
    (when (stringp (car n))
      (setcdr n (dir-locals--sort-variables (cdr n)))))

  variables)

;; Merge helpers replacing map-merge-with/map-merge/seq-group-by for
;; the list-of-alists merge in `dir-locals-read-from-dir'.
(defun remacs--dir-locals-merge-vars (a b)
  "Merge variable alists A and B: non-eval pairs of B shadow A's;
eval pairs from both are kept, A's first."
  (let ((aeval (seq-filter (lambda (e) (eq (car e) 'eval)) a))
        (beval (seq-filter (lambda (e) (eq (car e) 'eval)) b))
        (out (seq-filter (lambda (e) (not (eq (car e) 'eval))) a)))
    (dolist (e b)
      (unless (eq (car e) 'eval)
        (let ((slot (assoc (car e) out)))
          (if slot
              (setcdr slot (cdr e))
            (setq out (append out (list e)))))))
    (append out aeval beval)))

(defun remacs--dir-locals-merge (a b)
  "Merge two dir-locals top-level alists; B wins on equal keys."
  (let ((out (copy-sequence a)))
    (dolist (e b out)
      (let ((slot (assoc (car e) out)))
        (if slot
            (setcdr slot (remacs--dir-locals-merge-vars (cdr slot) (cdr e)))
          (setq out (append out (list e))))))))

(defun dir-locals-read-from-dir (dir)
  "Load all variables files in DIR and register a new class and instance.
DIR is the absolute name of a directory, which must contain at
least one dir-local file (which is a file holding variables to
apply).
Return the new class name, which is a symbol named DIR."
  (let* ((class-name (intern dir))
         (files (dir-locals--all-files dir))
	 ;; If there was a problem, use the values we could get but
	 ;; don't let the cache prevent future reads.
	 (latest 0) (success 0)
         (variables))
    (with-demoted-errors "Error reading dir-locals: %S"
      (dolist (file files)
	(let ((file-time (file-attribute-modification-time
			  (file-attributes (file-chase-links file)))))
	  (if (time-less-p latest file-time)
	    (setq latest file-time)))
        (with-temp-buffer
          (insert-file-contents file)
          (let ((newvars
                 (condition-case-unless-debug nil
                     ;; As a defensive measure, we do not allow
                     ;; circular data in the file/dir-local data.
                     (let ((read-circle nil))
                       (read (current-buffer)))
                   (end-of-file nil))))
            (unless (listp newvars)
              (message "Invalid data in %s: %s" file newvars)
              (setq newvars nil))
            (setq variables
                  ;; We want to make the variable setting from
                  ;; newvars (the second .dir-locals file) take
                  ;; precedence over the old variables, but we also
                  ;; want to preserve all `eval' elements as is from
                  ;; both lists.
                  (if (not (and newvars variables))
                      (or newvars variables)
                    (remacs--dir-locals-merge variables newvars))))))
      (setq success latest))
    (setq variables (dir-locals--sort-variables variables))
    (dir-locals-set-class-variables class-name variables)
    (dir-locals-set-directory-class dir class-name success)
    class-name))

(define-obsolete-function-alias 'dir-locals-read-from-file
  'dir-locals-read-from-dir "25.1")

(defcustom enable-remote-dir-locals nil
  "Non-nil means dir-local variables will be applied to remote files."
  :version "24.3"
  :type 'boolean
  :group 'find-file)

(defvar hack-dir-local-variables--warned-coding nil)

(defun hack-dir-local--get-variables (&optional predicate)
  "Read per-directory local variables for the current buffer.
Return a cons of the form (DIR . ALIST), where DIR is the
directory name (maybe nil) and ALIST is an alist of all variables
that might apply.  These will be filtered according to the
buffer's directory, but not according to its mode.
PREDICATE is passed to `dir-locals-collect-variables'."
  (when (and enable-local-variables
	     enable-dir-local-variables
	     (or enable-remote-dir-locals
		 (not (file-remote-p (or (buffer-file-name)
					 default-directory)))))
    ;; Find the variables file.
    (let ((dir-or-cache (dir-locals-find-file
                         (or (buffer-file-name) default-directory)))
	  (class nil)
	  (dir-name nil))
      (cond
       ((stringp dir-or-cache)
	(setq dir-name dir-or-cache
	      class (dir-locals-read-from-dir dir-or-cache)))
       ((consp dir-or-cache)
	(setq dir-name (nth 0 dir-or-cache))
	(setq class (nth 1 dir-or-cache))))
      (when class
        (cons dir-name
              (dir-locals-collect-variables
               (dir-locals-get-class-variables class)
               dir-name nil predicate))))))

(defvar hack-dir-local-get-variables-functions
  (list #'hack-dir-local--get-variables)
  "Special hook to compute the set of dir-local variables.
Every function is called without arguments and should return either
a cons of the form (DIR . ALIST) or a (possibly empty) list of such conses,
where ALIST is an alist of (VAR . VAL) settings.
DIR should be a string (a directory name) and is used to obey
`safe-local-variable-directories'.
This hook is run after the major mode has been setup.")

(defun hack-dir-local-variables ()
  "Read per-directory local variables for the current buffer.
Store the directory-local variables in `dir-local-variables-alist'
and `file-local-variables-alist', without applying them.

This does nothing if either `enable-local-variables' or
`enable-dir-local-variables' are nil."
  (let (items)
    (when (and enable-local-variables
	       enable-dir-local-variables
	       (or enable-remote-dir-locals
		   (not (file-remote-p (or (buffer-file-name)
					   default-directory)))))
      (run-hook-wrapped 'hack-dir-local-get-variables-functions
                        (lambda (fun)
                          (let ((res (funcall fun)))
                            (cond
                             ((null res))
                             ((consp (car-safe res))
                              (setq items (append res items)))
                             (t (push res items))))
			  nil)))
    ;; Sort the entries from nearest dir to furthest dir.
    (setq items (sort (nreverse items)
                      :key (lambda (x) (length (car-safe x))) :reverse t))
    ;; Filter out duplicates, preferring the settings from the nearest dir
    ;; and from the first hook function.
    (let ((seen nil))
      (dolist (item items)
        (when seen ;; Special case seen=nil since it's the most common case.
          (setcdr item (seq-filter (lambda (vv) (not (memq (car-safe vv) seen)))
                                   (cdr item))))
        (setq seen (nconc (seq-difference (mapcar #'car (cdr item))
                                          '(eval mode))
                          seen))))
    ;; Rather than a loop, maybe we should handle all the dirs
    ;; "together", e.g.  prompting the user only once.  But if so, we'd
    ;; probably want to also merge the prompt for file-local vars,
    ;; which comes from the call to `hack-local-variables-filter' in
    ;; `hack-local-variables'.
    (dolist (item items)
      (let ((dir-name (car item))
            (variables (cdr item)))
        (when variables
          (dolist (elt variables)
            (if (eq (car elt) 'coding)
                (unless hack-dir-local-variables--warned-coding
                  (setq hack-dir-local-variables--warned-coding t)
                  (display-warning 'files
                                   "Coding cannot be specified by dir-locals"))
              (unless (memq (car elt) '(eval mode))
                (setq dir-local-variables-alist
                      (assq-delete-all (car elt) dir-local-variables-alist)))
              (push elt dir-local-variables-alist)))
          (hack-local-variables-filter variables dir-name))))))

(defun hack-dir-local-variables-non-file-buffer ()
  "Apply directory-local variables to a non-file buffer.
For non-file buffers, such as Dired buffers, directory-local
variables are looked for in `default-directory' and its parent
directories."
  (hack-dir-local-variables)
  (hack-local-variables-apply))

(defsubst file-attribute-modification-time (attributes)
  "Return modification time of file ATTRIBUTES describes.
The file attribute list ATTRIBUTES must be a list returned by
`file-attributes'."
  (nth 5 attributes))

(defun seq-difference (sequence sequence2 &optional testfn)
  "Return a list of the elements of SEQUENCE that do not appear in SEQUENCE2.
SEQUENCE2 may be a list, vector, or string."
  (seq-filter (lambda (e)
                (not (seq-contains-p sequence2 e (or testfn #'equal))))
              sequence))

(defun image-type-auto-detected-p ()
  "Stub: no image support; always nil."
  nil)

(defvar enable-dir-local-variables t
  "Non-nil means read .dir-locals.el files.")

;; Mode stubs: correct mode symbol/name for `set-auto-mode' selection;
;; bodies are prog/text/special-derived approximations.
(define-derived-mode c-mode prog-mode "C")
(define-derived-mode c++-mode prog-mode "C++")
(define-derived-mode objc-mode prog-mode "ObjC")
(define-derived-mode java-mode prog-mode "Java")
(define-derived-mode javascript-mode prog-mode "Javascript")
(define-derived-mode js-json-mode prog-mode "JSON")
(define-derived-mode css-mode prog-mode "CSS")
(define-derived-mode mhtml-mode prog-mode "MHTML")
(define-derived-mode sgml-mode text-mode "SGML")
(define-derived-mode xml-mode prog-mode "XML")
(define-derived-mode conf-mode prog-mode "Conf")
(define-derived-mode conf-unix-mode prog-mode "Conf[Unix]")
(define-derived-mode conf-windows-mode prog-mode "Conf[Win]")
(define-derived-mode conf-space-mode prog-mode "Conf[Space]")
(define-derived-mode conf-colon-mode prog-mode "Conf[Colon]")
(define-derived-mode conf-desktop-mode prog-mode "Conf[Desktop]")
(define-derived-mode conf-javaprop-mode prog-mode "Conf[JavaProp]")
(define-derived-mode conf-ppd-mode prog-mode "Conf[PPD]")
(define-derived-mode conf-xdefaults-mode prog-mode "Conf[Xdefaults]")
(define-derived-mode conf-toml-mode prog-mode "Conf[TOML]")
(define-derived-mode conf-npmrc-mode prog-mode "Conf[NPMRC]")
(define-derived-mode makefile-mode prog-mode "Makefile")
(define-derived-mode makefile-gmake-mode prog-mode "GNUmakefile")
(define-derived-mode makefile-bsdmake-mode prog-mode "Makefile[BSD]")
(define-derived-mode makefile-imake-mode prog-mode "Makefile[imake]")
(define-derived-mode makefile-makepp-mode prog-mode "Makefile[makepp]")
(define-derived-mode makefile-automake-mode prog-mode "Makefile[automake]")
(define-derived-mode sh-mode prog-mode "Shell-script")
(defalias 'shell-script-mode 'sh-mode)
(define-derived-mode perl-mode prog-mode "Perl")
(define-derived-mode prolog-mode prog-mode "Prolog")
(define-derived-mode scheme-mode prog-mode "Scheme")
(define-derived-mode dsssl-mode prog-mode "DSSSL")
(define-derived-mode tcl-mode prog-mode "Tcl")
(define-derived-mode verilog-mode prog-mode "Verilog")
(define-derived-mode vhdl-mode prog-mode "VHDL")
(define-derived-mode m4-mode prog-mode "M4")
(define-derived-mode metafont-mode prog-mode "Metafont")
(define-derived-mode metapost-mode prog-mode "MetaPost")
(define-derived-mode simula-mode prog-mode "Simula")
(define-derived-mode opascal-mode prog-mode "OPascal")
(define-derived-mode m2-mode prog-mode "Modula-2")
(define-derived-mode icon-mode prog-mode "Icon")
(define-derived-mode dcl-mode prog-mode "DCL")
(define-derived-mode fortran-mode prog-mode "Fortran")
(define-derived-mode f90-mode prog-mode "F90")
(define-derived-mode asm-mode prog-mode "Assembler")
(define-derived-mode antlr-mode prog-mode "Antlr")
(define-derived-mode antlr-v4-mode prog-mode "Antlr-v4")
(define-derived-mode python-mode prog-mode "Python")
(define-derived-mode ruby-mode prog-mode "Ruby")
(define-derived-mode diff-mode prog-mode "Diff")
(define-derived-mode dns-mode prog-mode "DNS")
(define-derived-mode sql-mode prog-mode "SQL")
(define-derived-mode tex-mode text-mode "TeX")
(define-derived-mode latex-mode tex-mode "LaTeX")
(define-derived-mode doctex-mode tex-mode "DocTeX")
(define-derived-mode plain-tex-mode tex-mode "TeX")
(define-derived-mode slitex-mode tex-mode "SliTeX")
(define-derived-mode texinfo-mode text-mode "Texinfo")
(define-derived-mode nroff-mode text-mode "Nroff")
(define-derived-mode scribe-mode text-mode "Scribe")
(define-derived-mode mail-mode text-mode "Mail")
(define-derived-mode bibtex-mode text-mode "BibTeX")
(define-derived-mode bibtex-style-mode text-mode "BibTeX-Style")
(define-derived-mode change-log-mode text-mode "ChangeLog")
(define-derived-mode org-mode prog-mode "Org")
(define-derived-mode ps-mode prog-mode "PostScript")
(define-derived-mode mixal-mode prog-mode "MIXAL")
(define-derived-mode ses-mode prog-mode "SES")
(define-derived-mode sieve-mode prog-mode "Sieve")
(define-derived-mode lisp-data-mode prog-mode "Lisp-Data")
(define-derived-mode autoconf-mode prog-mode "Autoconf")
(define-derived-mode compilation-mode prog-mode "Compilation")
(define-derived-mode ebrowse-tree-mode prog-mode "Ebrowse-Tree")
(define-derived-mode erts-mode prog-mode "Erts")
(define-derived-mode gdb-script-mode prog-mode "GDB-Script")
(define-derived-mode ld-script-mode prog-mode "LD-Script")
(define-derived-mode bovine-grammar-mode prog-mode "Bovine-Grammar")
(define-derived-mode wisent-grammar-mode prog-mode "Wisent-Grammar")
(define-derived-mode srecode-template-mode prog-mode "SRecode")
(define-derived-mode snmp-mode prog-mode "SNMP")
(define-derived-mode snmpv2-mode prog-mode "SNMPv2")
(define-derived-mode authinfo-mode prog-mode "Authinfo")
(define-derived-mode archive-mode special-mode "Archive")
(define-derived-mode tar-mode special-mode "Tar")
(define-derived-mode image-mode special-mode "Image")
(define-derived-mode doc-view-mode special-mode "DocView")
(defun doc-view-mode-maybe ()
  "Stub: `doc-view-mode' approximation."
  (doc-view-mode))
(put 'doc-view-mode-maybe 'safe-local-eval-function nil)

;; remaining auto-mode symbols (fboundp-guarded stubs)
(unless (fboundp 'antlr-mode)
  (define-derived-mode antlr-mode prog-mode "Antlr"))
(unless (fboundp 'antlr-v4-mode)
  (define-derived-mode antlr-v4-mode prog-mode "Antlr-V4"))
(unless (fboundp 'archive-mode)
  (define-derived-mode archive-mode special-mode "Archive"))
(unless (fboundp 'asm-mode)
  (define-derived-mode asm-mode prog-mode "Asm"))
(unless (fboundp 'authinfo-mode)
  (define-derived-mode authinfo-mode prog-mode "Authinfo"))
(unless (fboundp 'autoconf-mode)
  (define-derived-mode autoconf-mode prog-mode "Autoconf"))
(unless (fboundp 'awk-mode)
  (define-derived-mode awk-mode prog-mode "AWK"))
(unless (fboundp 'bat-mode)
  (define-derived-mode bat-mode prog-mode "Bat"))
(unless (fboundp 'bibtex-mode)
  (define-derived-mode bibtex-mode text-mode "Bibtex"))
(unless (fboundp 'bibtex-style-mode)
  (define-derived-mode bibtex-style-mode text-mode "Bibtex-Style"))
(unless (fboundp 'bovine-grammar-mode)
  (define-derived-mode bovine-grammar-mode prog-mode "Bovine-Grammar"))
(unless (fboundp 'c++-mode)
  (define-derived-mode c++-mode prog-mode "C++"))
(unless (fboundp 'c-mode)
  (define-derived-mode c-mode prog-mode "C"))
(unless (fboundp 'c-or-c++-mode) (defun c-or-c++-mode () "Stub: pick c-mode." (c-mode)))
(unless (fboundp 'change-log-mode)
  (define-derived-mode change-log-mode text-mode "Change-Log"))
(unless (fboundp 'cmake-ts-mode-maybe)
  (defun cmake-ts-mode-maybe () "Stub for `cmake-ts-mode-maybe'."
    (if (fboundp 'cmake-mode) (funcall 'cmake-mode) (prog-mode))))
(unless (fboundp 'compilation-mode)
  (define-derived-mode compilation-mode prog-mode "Compilation"))
(unless (fboundp 'conf-colon-mode)
  (define-derived-mode conf-colon-mode prog-mode "Conf-Colon"))
(unless (fboundp 'conf-desktop-mode)
  (define-derived-mode conf-desktop-mode prog-mode "Conf-Desktop"))
(unless (fboundp 'conf-javaprop-mode)
  (define-derived-mode conf-javaprop-mode prog-mode "Conf-Javaprop"))
(unless (fboundp 'conf-mode)
  (define-derived-mode conf-mode prog-mode "Conf"))
(unless (fboundp 'conf-mode-maybe)
  (defun conf-mode-maybe () "Stub for `conf-mode-maybe'."
    (if (fboundp 'conf-mode) (funcall 'conf-mode) (prog-mode))))
(unless (fboundp 'conf-npmrc-mode)
  (define-derived-mode conf-npmrc-mode prog-mode "Conf-Npmrc"))
(unless (fboundp 'conf-ppd-mode)
  (define-derived-mode conf-ppd-mode prog-mode "Conf-Ppd"))
(unless (fboundp 'conf-space-mode)
  (define-derived-mode conf-space-mode prog-mode "Conf-Space"))
(unless (fboundp 'conf-toml-mode)
  (define-derived-mode conf-toml-mode prog-mode "Conf-Toml"))
(unless (fboundp 'conf-unix-mode)
  (define-derived-mode conf-unix-mode prog-mode "Conf-Unix"))
(unless (fboundp 'conf-windows-mode)
  (define-derived-mode conf-windows-mode prog-mode "Conf-Windows"))
(unless (fboundp 'conf-xdefaults-mode)
  (define-derived-mode conf-xdefaults-mode prog-mode "Conf-Xdefaults"))
(unless (fboundp 'csharp-mode)
  (define-derived-mode csharp-mode prog-mode "C#"))
(unless (fboundp 'css-mode)
  (define-derived-mode css-mode prog-mode "Css"))
(unless (fboundp 'dcl-mode)
  (define-derived-mode dcl-mode prog-mode "Dcl"))
(unless (fboundp 'diff-mode)
  (define-derived-mode diff-mode prog-mode "Diff"))
(unless (fboundp 'dns-mode)
  (define-derived-mode dns-mode prog-mode "Dns"))
(unless (fboundp 'doc-view-mode-maybe)
  (defun doc-view-mode-maybe () "Stub for `doc-view-mode-maybe'."
    (if (fboundp 'doc-view-mode) (funcall 'doc-view-mode) (prog-mode))))
(unless (fboundp 'dockerfile-ts-mode-maybe)
  (defun dockerfile-ts-mode-maybe () "Stub for `dockerfile-ts-mode-maybe'."
    (if (fboundp 'dockerfile-mode) (funcall 'dockerfile-mode) (prog-mode))))
(unless (fboundp 'doctex-mode)
  (define-derived-mode doctex-mode text-mode "Doctex"))
(unless (fboundp 'dsssl-mode)
  (define-derived-mode dsssl-mode prog-mode "Dsssl"))
(unless (fboundp 'ebrowse-tree-mode)
  (define-derived-mode ebrowse-tree-mode prog-mode "Ebrowse-Tree"))
(unless (fboundp 'editorconfig-conf-mode)
  (define-derived-mode editorconfig-conf-mode prog-mode "EditorConfig"))
(unless (fboundp 'elisp-byte-code-mode)
  (define-derived-mode elisp-byte-code-mode special-mode "Elisp-Byte-Code"))
(unless (fboundp 'elixir-ts-mode-maybe)
  (defun elixir-ts-mode-maybe () "Stub for `elixir-ts-mode-maybe'."
    (if (fboundp 'elixir-mode) (funcall 'elixir-mode) (prog-mode))))
(unless (fboundp 'emacs-lisp-mode)
  (define-derived-mode emacs-lisp-mode prog-mode "Emacs-Lisp"))
(unless (fboundp 'epa-file) (defun epa-file () "Stub: `epa-file'." nil))
(unless (fboundp 'erts-mode)
  (define-derived-mode erts-mode prog-mode "Erts"))
(unless (fboundp 'f90-mode)
  (define-derived-mode f90-mode prog-mode "F90"))
(unless (fboundp 'fortran-mode)
  (define-derived-mode fortran-mode prog-mode "Fortran"))
(unless (fboundp 'fundamental-mode)
  (define-derived-mode fundamental-mode prog-mode "Fundamental"))
(unless (fboundp 'gdb-script-mode)
  (define-derived-mode gdb-script-mode prog-mode "Gdb-Script"))
(unless (fboundp 'go-mod-ts-mode-maybe)
  (defun go-mod-ts-mode-maybe () "Stub for `go-mod-ts-mode-maybe'."
    (if (fboundp 'go-mod-mode) (funcall 'go-mod-mode) (prog-mode))))
(unless (fboundp 'go-ts-mode-maybe)
  (defun go-ts-mode-maybe () "Stub for `go-ts-mode-maybe'."
    (if (fboundp 'go-mode) (funcall 'go-mode) (prog-mode))))
(unless (fboundp 'go-work-ts-mode-maybe)
  (defun go-work-ts-mode-maybe () "Stub for `go-work-ts-mode-maybe'."
    (if (fboundp 'go-work-mode) (funcall 'go-work-mode) (prog-mode))))
(unless (fboundp 'heex-ts-mode-maybe)
  (defun heex-ts-mode-maybe () "Stub for `heex-ts-mode-maybe'."
    (if (fboundp 'heex-mode) (funcall 'heex-mode) (prog-mode))))
(unless (fboundp 'icon-mode)
  (define-derived-mode icon-mode prog-mode "Icon"))
(unless (fboundp 'idl-mode)
  (define-derived-mode idl-mode prog-mode "IDL"))
(unless (fboundp 'image-mode)
  (define-derived-mode image-mode special-mode "Image"))
(unless (fboundp 'java-mode)
  (define-derived-mode java-mode prog-mode "Java"))
(unless (fboundp 'javascript-mode)
  (define-derived-mode javascript-mode prog-mode "Javascript"))
(unless (fboundp 'jka-compr) (defun jka-compr () "Stub: `jka-compr'." nil))
(unless (fboundp 'js-json-mode)
  (define-derived-mode js-json-mode prog-mode "Js-Json"))
(unless (fboundp 'js-mode)
  (define-derived-mode js-mode prog-mode "JavaScript"))
(unless (fboundp 'latex-mode)
  (define-derived-mode latex-mode text-mode "Latex"))
(unless (fboundp 'ld-script-mode)
  (define-derived-mode ld-script-mode prog-mode "Ld-Script"))
(unless (fboundp 'less-css-mode)
  (define-derived-mode less-css-mode prog-mode "LESS"))
(unless (fboundp 'lisp-data-mode)
  (define-derived-mode lisp-data-mode prog-mode "Lisp-Data"))
(unless (fboundp 'lisp-mode)
  (define-derived-mode lisp-mode prog-mode "Lisp"))
(unless (fboundp 'lua-mode)
  (define-derived-mode lua-mode prog-mode "Lua"))
(unless (fboundp 'm2-mode)
  (define-derived-mode m2-mode prog-mode "M2"))
(unless (fboundp 'm4-mode)
  (define-derived-mode m4-mode prog-mode "M4"))
(unless (fboundp 'mail-mode)
  (define-derived-mode mail-mode text-mode "Mail"))
(unless (fboundp 'makefile-automake-mode)
  (define-derived-mode makefile-automake-mode prog-mode "Makefile-Automake"))
(unless (fboundp 'makefile-bsdmake-mode)
  (define-derived-mode makefile-bsdmake-mode prog-mode "Makefile-Bsdmake"))
(unless (fboundp 'makefile-gmake-mode)
  (define-derived-mode makefile-gmake-mode prog-mode "Makefile-Gmake"))
(unless (fboundp 'makefile-imake-mode)
  (define-derived-mode makefile-imake-mode prog-mode "Makefile-Imake"))
(unless (fboundp 'makefile-makepp-mode)
  (define-derived-mode makefile-makepp-mode prog-mode "Makefile-Makepp"))
(unless (fboundp 'metafont-mode)
  (define-derived-mode metafont-mode prog-mode "Metafont"))
(unless (fboundp 'metapost-mode)
  (define-derived-mode metapost-mode prog-mode "Metapost"))
(unless (fboundp 'mhtml-mode)
  (define-derived-mode mhtml-mode text-mode "Mhtml"))
(unless (fboundp 'mixal-mode)
  (define-derived-mode mixal-mode prog-mode "Mixal"))
(unless (fboundp 'nroff-mode)
  (define-derived-mode nroff-mode text-mode "Nroff"))
(unless (fboundp 'objc-mode)
  (define-derived-mode objc-mode prog-mode "Objc"))
(unless (fboundp 'octave-maybe-mode)
  (defun octave-maybe-mode () "Stub for `octave-maybe-mode'."
    (if (fboundp 'octave-mode) (funcall 'octave-mode) (prog-mode))))
(unless (fboundp 'octave-mode)
  (define-derived-mode octave-mode prog-mode "Octave"))
(unless (fboundp 'opascal-mode)
  (define-derived-mode opascal-mode prog-mode "Opascal"))
(unless (fboundp 'org-mode)
  (define-derived-mode org-mode prog-mode "Org"))
(unless (fboundp 'pascal-mode)
  (define-derived-mode pascal-mode prog-mode "Pascal"))
(unless (fboundp 'perl-mode)
  (define-derived-mode perl-mode prog-mode "Perl"))
(unless (fboundp 'php-ts-mode-maybe)
  (defun php-ts-mode-maybe () "Stub for `php-ts-mode-maybe'."
    (if (fboundp 'php-mode) (funcall 'php-mode) (prog-mode))))
(unless (fboundp 'pike-mode)
  (define-derived-mode pike-mode prog-mode "Pike"))
(unless (fboundp 'prolog-mode)
  (define-derived-mode prolog-mode prog-mode "Prolog"))
(unless (fboundp 'ps-mode)
  (define-derived-mode ps-mode prog-mode "Ps"))
(unless (fboundp 'python-mode)
  (define-derived-mode python-mode prog-mode "Python"))
(unless (fboundp 'rst-mode)
  (define-derived-mode rst-mode text-mode "reST"))
(unless (fboundp 'ruby-mode)
  (define-derived-mode ruby-mode prog-mode "Ruby"))
(unless (fboundp 'rust-ts-mode-maybe)
  (defun rust-ts-mode-maybe () "Stub for `rust-ts-mode-maybe'."
    (if (fboundp 'rust-mode) (funcall 'rust-mode) (prog-mode))))
(unless (fboundp 'scheme-mode)
  (define-derived-mode scheme-mode prog-mode "Scheme"))
(unless (fboundp 'scribe-mode)
  (define-derived-mode scribe-mode text-mode "Scribe"))
(unless (fboundp 'scss-mode)
  (define-derived-mode scss-mode prog-mode "SCSS"))
(unless (fboundp 'ses-mode)
  (define-derived-mode ses-mode prog-mode "Ses"))
(unless (fboundp 'sgml-mode)
  (define-derived-mode sgml-mode text-mode "Sgml"))
(unless (fboundp 'sh-mode)
  (define-derived-mode sh-mode prog-mode "Sh"))
(unless (fboundp 'sieve-mode)
  (define-derived-mode sieve-mode prog-mode "Sieve"))
(unless (fboundp 'simula-mode)
  (define-derived-mode simula-mode prog-mode "Simula"))
(unless (fboundp 'snmp-mode)
  (define-derived-mode snmp-mode prog-mode "Snmp"))
(unless (fboundp 'snmpv2-mode)
  (define-derived-mode snmpv2-mode prog-mode "Snmpv2"))
(unless (fboundp 'sql-mode)
  (define-derived-mode sql-mode prog-mode "Sql"))
(unless (fboundp 'srecode-template-mode)
  (define-derived-mode srecode-template-mode prog-mode "Srecode-Template"))
(unless (fboundp 'tar-mode)
  (define-derived-mode tar-mode special-mode "Tar"))
(unless (fboundp 'tcl-mode)
  (define-derived-mode tcl-mode prog-mode "Tcl"))
(unless (fboundp 'tex-mode)
  (define-derived-mode tex-mode text-mode "Tex"))
(unless (fboundp 'texinfo-mode)
  (define-derived-mode texinfo-mode text-mode "Texinfo"))
(unless (fboundp 'text-mode)
  (define-derived-mode text-mode prog-mode "Text"))
(unless (fboundp 'tsx-ts-mode-maybe)
  (defun tsx-ts-mode-maybe () "Stub for `tsx-ts-mode-maybe'."
    (if (fboundp 'tsx-mode) (funcall 'tsx-mode) (prog-mode))))
(unless (fboundp 'typescript-ts-mode-maybe)
  (defun typescript-ts-mode-maybe () "Stub for `typescript-ts-mode-maybe'."
    (if (fboundp 'typescript-mode) (funcall 'typescript-mode) (prog-mode))))
(unless (fboundp 'vera-mode)
  (define-derived-mode vera-mode prog-mode "Vera"))
(unless (fboundp 'verilog-mode)
  (define-derived-mode verilog-mode prog-mode "Verilog"))
(unless (fboundp 'vhdl-mode)
  (define-derived-mode vhdl-mode prog-mode "Vhdl"))
(unless (fboundp 'wisent-grammar-mode)
  (define-derived-mode wisent-grammar-mode prog-mode "Wisent-Grammar"))
(unless (fboundp 'xml-mode)
  (define-derived-mode xml-mode prog-mode "Xml"))
(unless (fboundp 'yaml-ts-mode-maybe)
  (defun yaml-ts-mode-maybe () "Stub for `yaml-ts-mode-maybe'."
    (if (fboundp 'yaml-mode) (funcall 'yaml-mode) (prog-mode))))

;; *scratch* starts in lisp-interaction-mode (GNU batch behavior too).
(when (get-buffer "*scratch*")
  (with-current-buffer "*scratch*"
    (lisp-interaction-mode)))
"##;
