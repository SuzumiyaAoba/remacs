;;; qa_gdb_mi_disassembly.el --- GDB/MI disassembly request regression -*- lexical-binding:t -*-

(require 'cl-lib)

(defun qa-gdb-load-disassembly-trigger ()
  "Load the current checkout's GDB/MI trigger definition."
  (load (expand-file-name
         "../lisp/gdb-mi.el"
         (file-name-directory load-file-name)))
  'loaded)

(qa (qa-gdb-load-disassembly-trigger) 'loaded)

(defun qa-gdb-disassembly-command (frame)
  "Return the command produced for FRAME without connecting to GDB."
  (let (command)
    (cl-letf (((symbol-function 'gdb-current-buffer-frame)
               (lambda () frame))
              ((symbol-function 'gdb-input)
               (lambda (input _handler &optional _trigger)
                 (setq command input))))
      (gdb-invalidate-disassembly 'start))
    command))

(qa
 (qa-gdb-disassembly-command '((fullname . "/src/file.c")))
 "-data-disassemble -s $pc -e \"$pc + 150\" -- 0")

(qa
 (qa-gdb-disassembly-command
  '((fullname . "/src/file.c") (line . "12")))
 "-data-disassemble -f /src/file.c -l 12 -n -1 -- 0")

(princ (qa--done))
