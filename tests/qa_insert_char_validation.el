;;; qa_insert_char_validation.el --- Validate before byte conversion -*- lexical-binding: t; -*-

(defun qa318-character (character unibyte &optional count inherit)
  (with-temp-buffer
    (when unibyte (set-buffer-multibyte nil))
    (let ((observed nil))
      (let ((after-change-functions (list (lambda (&rest _) (push 'after observed)))))
        (list (condition-case error (insert-char character count inherit)
                (error error))
              (string-to-list (buffer-string)) (point) observed)))))

;; multibyte character -1
(qa (qa318-character -1 nil)
    '((wrong-type-argument characterp -1) nil 1 nil))

;; multibyte character -4294967231
(qa (qa318-character -4294967231 nil)
    '((wrong-type-argument characterp -4294967231) nil 1 nil))

;; multibyte character 4194304
(qa (qa318-character 4194304 nil)
    '((wrong-type-argument characterp 4194304) nil 1 nil))

;; multibyte character 4294967361
(qa (qa318-character 4294967361 nil)
    '((wrong-type-argument characterp 4294967361) nil 1 nil))

;; multibyte character 4294967296
(qa (qa318-character 4294967296 nil)
    '((wrong-type-argument characterp 4294967296) nil 1 nil))

;; multibyte character 0
(qa (qa318-character 0 nil)
    '(nil (0) 2 (after)))

;; multibyte character 65
(qa (qa318-character 65 nil)
    '(nil (65) 2 (after)))

;; multibyte character 255
(qa (qa318-character 255 nil)
    '(nil (255) 2 (after)))

;; multibyte character 256
(qa (qa318-character 256 nil)
    '(nil (256) 2 (after)))

;; multibyte character 1114111
(qa (qa318-character 1114111 nil)
    '(nil (1114111) 2 (after)))

;; multibyte character 1114112
(qa (qa318-character 1114112 nil)
    '(nil (1114112) 2 (after)))

;; multibyte character 4194303
(qa (qa318-character 4194303 nil)
    '(nil (4194303) 2 (after)))

;; multibyte invalid nil
(qa (qa318-character nil nil)
    '((wrong-type-argument characterp nil) nil 1 nil))

;; multibyte invalid 65.0
(qa (qa318-character 65.0 nil)
    '((wrong-type-argument characterp 65.0) nil 1 nil))

;; multibyte invalid 'a
(qa (qa318-character 'a nil)
    '((wrong-type-argument characterp a) nil 1 nil))

;; multibyte invalid 1152921504606846976
(qa (qa318-character 1152921504606846976 nil)
    '((wrong-type-argument characterp 1152921504606846976) nil 1 nil))

;; character checked before zero count
(qa (qa318-character -1 nil 0 t)
    '((wrong-type-argument characterp -1) nil 1 nil))

;; character checked before invalid count
(qa (qa318-character 4194304 nil 'bad t)
    '((wrong-type-argument characterp 4194304) nil 1 nil))

;; unibyte character -1
(qa (qa318-character -1 t)
    '((wrong-type-argument characterp -1) nil 1 nil))

;; unibyte character -4294967231
(qa (qa318-character -4294967231 t)
    '((wrong-type-argument characterp -4294967231) nil 1 nil))

;; unibyte character 4194304
(qa (qa318-character 4194304 t)
    '((wrong-type-argument characterp 4194304) nil 1 nil))

;; unibyte character 4294967361
(qa (qa318-character 4294967361 t)
    '((wrong-type-argument characterp 4294967361) nil 1 nil))

;; unibyte character 4294967296
(qa (qa318-character 4294967296 t)
    '((wrong-type-argument characterp 4294967296) nil 1 nil))

;; unibyte character 0
(qa (qa318-character 0 t)
    '(nil (0) 2 (after)))

;; unibyte character 65
(qa (qa318-character 65 t)
    '(nil (65) 2 (after)))

;; unibyte character 255
(qa (qa318-character 255 t)
    '(nil (255) 2 (after)))

;; unibyte character 256
(qa (qa318-character 256 t)
    '(nil (0) 2 (after)))

;; unibyte character 1114111
(qa (qa318-character 1114111 t)
    '(nil (255) 2 (after)))

;; unibyte character 1114112
(qa (qa318-character 1114112 t)
    '(nil (0) 2 (after)))

;; unibyte character 4194303
(qa (qa318-character 4194303 t)
    '(nil (255) 2 (after)))

;; unibyte invalid nil
(qa (qa318-character nil t)
    '((wrong-type-argument characterp nil) nil 1 nil))

;; unibyte invalid 65.0
(qa (qa318-character 65.0 t)
    '((wrong-type-argument characterp 65.0) nil 1 nil))

;; unibyte invalid 'a
(qa (qa318-character 'a t)
    '((wrong-type-argument characterp a) nil 1 nil))

;; unibyte invalid 1152921504606846976
(qa (qa318-character 1152921504606846976 t)
    '((wrong-type-argument characterp 1152921504606846976) nil 1 nil))

;; character checked before zero count
(qa (qa318-character -1 t 0 t)
    '((wrong-type-argument characterp -1) nil 1 nil))

;; character checked before invalid count
(qa (qa318-character 4194304 t 'bad t)
    '((wrong-type-argument characterp 4194304) nil 1 nil))

(princ (qa--done))
