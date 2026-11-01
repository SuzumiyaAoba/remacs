use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r##"(progn
  (require 'vtable)
  ;; Load any lazy pixel helpers before installing the function stub.
  (require 'subr-x)
  (defvar remacs-vtable-3034-target-window-a nil)
  (defvar remacs-vtable-3034-target-window-b nil)
  (defvar remacs-vtable-3034-target-frame nil)
  (defvar remacs-vtable-3034-real-window-width nil)
  (defvar remacs-vtable-3034-real-frame-parameter nil)
  (defvar remacs-vtable-3034-real-display-graphic-p nil)
  (defvar remacs-vtable-3034-window nil)
  (defvar remacs-vtable-3034-columns 80)
  (defvar remacs-vtable-3034-pixels 640)
  (defvar remacs-vtable-3034-x-pixels 8)
  (defvar remacs-vtable-3034-cell-pixels 8)
  (defvar remacs-vtable-3034-graphic t)
  (defvar remacs-vtable-3034-font "Font-A")
  (defvar remacs-vtable-3034-fontset "Fontset-A")
  (defvar remacs-vtable-3034-scale 1.0)
  (defvar remacs-vtable-3034-getter-calls 0)
  (defun remacs-vtable-3034-assert (condition message)
    (unless condition (error "%s" message)))
  (defun remacs-vtable-3034-getter (object index _table)
    (setq remacs-vtable-3034-getter-calls
          (1+ remacs-vtable-3034-getter-calls))
    (aref object index))
  (defun remacs-vtable-3034-pixels (string &optional _buffer)
    (if (and (= (length string) 1) (= (aref string 0) ?x))
        remacs-vtable-3034-x-pixels
      (* (length string) remacs-vtable-3034-cell-pixels)))
  (defun remacs-vtable-3034-assert-pixel-stub ()
    (unless (eq (indirect-function 'string-pixel-width)
                (indirect-function 'remacs-vtable-3034-pixels))
      (error "string-pixel-width fixture stub was replaced")))
  (defun remacs-vtable-3034-widths (cache)
    (append (vtable--cache-widths cache) nil))
  (defun remacs-vtable-3034-run
      (table window frame columns pixels x-pixels graphic font fontset scale action)
    (setq remacs-vtable-3034-window window
          remacs-vtable-3034-columns columns
          remacs-vtable-3034-pixels pixels
          remacs-vtable-3034-x-pixels x-pixels
          remacs-vtable-3034-cell-pixels x-pixels
          remacs-vtable-3034-graphic graphic
          remacs-vtable-3034-font font
          remacs-vtable-3034-fontset fontset
          remacs-vtable-3034-scale scale)
    (select-window window)
    (remacs-vtable-3034-assert-pixel-stub)
    (if (eq action 'render)
        (progn
          (set-window-buffer window (vtable-buffer table))
          (with-current-buffer (vtable-buffer table)
            (let ((inhibit-read-only t)) (erase-buffer))
            (vtable--insert table))
          (remacs-vtable-3034-assert-pixel-stub)
          (list (slot-value table '-current-cache-key)
                (vtable--current-cache table)
                (remacs-vtable-3034-widths
                 (vtable--current-cache table))))
      (if table
          (with-current-buffer (vtable-buffer table)
            (set-window-buffer window (vtable-buffer table))
            (vtable--cache-key table))
        (vtable--cache-key))))
  (defun remacs-vtable-3034-install-metric-stubs ()
    ;; Keep real windows, frames and terminals.  Intercept only metric calls
    ;; for the two test windows and their shared frame; delegate elsewhere.
    (setq remacs-vtable-3034-real-window-width
          (symbol-function 'window-width)
          remacs-vtable-3034-real-frame-parameter
          (symbol-function 'frame-parameter)
          remacs-vtable-3034-real-display-graphic-p
          (symbol-function 'display-graphic-p))
    (cl-letf (((symbol-function 'window-width)
               (lambda (&optional window pixelwise)
                 (let ((window (or window (selected-window))))
                   (if (or (eq window remacs-vtable-3034-target-window-a)
                           (eq window remacs-vtable-3034-target-window-b))
                       (if pixelwise remacs-vtable-3034-pixels
                         remacs-vtable-3034-columns)
                     (funcall remacs-vtable-3034-real-window-width
                              window pixelwise)))))
              ((symbol-function 'frame-parameter)
               (lambda (&optional frame parameter)
                 (if (and (or (null frame)
                              (eq frame remacs-vtable-3034-target-frame))
                          (memq parameter '(font fontset scale-factor)))
                     (cond ((eq parameter 'font) remacs-vtable-3034-font)
                           ((eq parameter 'fontset) remacs-vtable-3034-fontset)
                           ((eq parameter 'scale-factor) remacs-vtable-3034-scale))
                   (funcall remacs-vtable-3034-real-frame-parameter
                            frame parameter))))
              ((symbol-function 'display-graphic-p)
               (lambda (&optional display)
                 (if (or (null display)
                         (eq display remacs-vtable-3034-target-frame))
                     remacs-vtable-3034-graphic
                   (funcall remacs-vtable-3034-real-display-graphic-p display))))
              ((symbol-function 'string-pixel-width)
               #'remacs-vtable-3034-pixels))
      (remacs-vtable-3034-assert-pixel-stub)
      (remacs-vtable-3034-assert
       (= (string-pixel-width "WW") 16)
       "pixel-width stub did not replace the metric boundary")
      (remacs-vtable-3034-check-geometry-and-reuse)
      (remacs-vtable-3034-check-context-and-snapshots)
      t))
  (defun remacs-vtable-3034-check-geometry-and-reuse ()
    (with-temp-buffer
      (setq-local face-remapping-alist nil
                  char-property-alias-alist nil
                  default-text-properties nil)
      (setq remacs-vtable-3034-getter-calls 0)
      (let* ((table
              (make-vtable
               :columns [(:name "Percent" :width "100%")
                         (:name "Numeric" :width 2)
                         (:name "Ex" :width "2ex")
                         (:name "Pixel" :width "20px")]
               :objects (list (vector "a" "b" "c" "d"))
               :getter #'remacs-vtable-3034-getter
               :face 'vtable
               :use-header-line nil
               :insert nil)))
        (vtable-set-buffer table (current-buffer))
        ;; Ignore the column-inference call made by make-vtable.
        (setq remacs-vtable-3034-getter-calls 0)
        (let* ((window-a remacs-vtable-3034-target-window-a)
               (window-b remacs-vtable-3034-target-window-b)
               (frame remacs-vtable-3034-target-frame)
               (first (remacs-vtable-3034-run
                       table window-a frame 80 640 8 t
                       "Font-A" "Fontset-A" 1.0 'render))
               (first-key (nth 0 first))
               (first-cache (nth 1 first)))
          (remacs-vtable-3034-assert
           (and (consp first-key) (= (cdr first-key) 80))
           "cache key did not keep character width in its cdr")
          (remacs-vtable-3034-assert
           (equal (nth 2 first) '(640 16 16 20))
           "initial percentage/numeric/ex/pixel widths are wrong")
          (remacs-vtable-3034-assert
           (= remacs-vtable-3034-getter-calls 4)
           "first geometry cache miss did not call four getters")
          ;; Two real windows in one frame with identical metrics share cache.
          (let ((second-window
                 (remacs-vtable-3034-run
                  table window-b frame 80 640 8 t
                  "Font-A" "Fontset-A" 1.0 'render)))
            (remacs-vtable-3034-assert
             (equal first-key (nth 0 second-window))
             "same-frame windows with equal metrics did not share a key")
            (remacs-vtable-3034-assert
             (eq first-cache (nth 1 second-window))
             "same-metric second window did not reuse the cache")
            (remacs-vtable-3034-assert
             (= remacs-vtable-3034-getter-calls 4)
             "same-metric second window reran getters"))
          ;; Same character count, distinct pixel body width.
          (let ((resized
                 (remacs-vtable-3034-run
                  table window-b frame 80 800 8 t
                  "Font-A" "Fontset-A" 1.0 'render)))
            (remacs-vtable-3034-assert
             (not (equal first-key (nth 0 resized)))
             "640px and 800px layouts shared a cache key")
            (remacs-vtable-3034-assert
             (equal (nth 2 resized) '(800 16 16 20))
             "100% width did not follow the pixel body width")
            (remacs-vtable-3034-assert
             (= remacs-vtable-3034-getter-calls 8)
             "geometry change did not build one new raw-cell cache"))
          ;; Same pixels, changed table character metric: numeric/ex differ.
          (let ((char-resized
                 (remacs-vtable-3034-run
                  table window-b frame 80 800 10 t
                  "Font-A" "Fontset-A" 1.0 'render)))
            (remacs-vtable-3034-assert
             (not (equal (remacs-vtable-3034-run
                          table window-b frame 80 800 8 t
                          "Font-A" "Fontset-A" 1.0 'key)
                         (nth 0 char-resized)))
             "changed character-pixel metric did not change the key")
            (remacs-vtable-3034-assert
             (equal (nth 2 char-resized) '(800 20 20 20))
             "numeric/ex widths did not follow the changed character metric")
            (remacs-vtable-3034-assert
             (= remacs-vtable-3034-getter-calls 12)
             "character metric change did not build one raw-cell cache"))
          (let ((tty
                 (remacs-vtable-3034-run
                  table window-b frame 80 800 10 nil
                  "Font-A" "Fontset-A" 1.0 'render)))
            (remacs-vtable-3034-assert
             (= (nth 3 (nth 2 tty)) 2)
             "TTY 20px conversion changed")
            (remacs-vtable-3034-assert
             (= remacs-vtable-3034-getter-calls 16)
             "TTY mode did not build one new raw-cell cache")))
      t)))
  (defun remacs-vtable-3034-check-context-and-snapshots ()
    (with-temp-buffer
      (setq-local face-remapping-alist nil
                  char-property-alias-alist nil
                  default-text-properties nil)
      (let* ((table (make-vtable :columns ["Value"] :objects nil
                                 :face 'vtable :insert nil)))
        (vtable-set-buffer table (current-buffer))
        (let* ((window remacs-vtable-3034-target-window-a)
               (frame remacs-vtable-3034-target-frame)
               (base (remacs-vtable-3034-run
                      table window frame 80 640 8 t
                      "Font-A" "Fontset-A" 1.0 'key))
               (no-table (remacs-vtable-3034-run
                          nil window frame 80 640 8 t
                          "Font-A" "Fontset-A" 1.0 'key)))
          (remacs-vtable-3034-assert
           (and (consp no-table) (= (cdr no-table) 80))
           "no-TABLE key did not measure default face or keep character width")
          (remacs-vtable-3034-assert
           (not (equal base no-table))
           "no-TABLE default face and table face keys were conflated")
          (setf (vtable-face table) nil)
          (remacs-vtable-3034-assert
           (not (equal base
                       (remacs-vtable-3034-run
                        table window frame 80 640 8 t
                        "Font-A" "Fontset-A" 1.0 'key)))
           "face:nil did not produce a distinct key")
          (setf (vtable-face table) 'vtable)
          (setq base (remacs-vtable-3034-run
                      table window frame 80 640 8 t
                      "Font-A" "Fontset-A" 1.0 'key))
          (remacs-vtable-3034-assert
           (not (equal base
                       (remacs-vtable-3034-run
                        table window frame 80 640 8 t
                        "Font-B" "Fontset-A" 1.0 'key)))
           "font change was absent from the immutable key")
          (remacs-vtable-3034-assert
           (not (equal base
                       (remacs-vtable-3034-run
                        table window frame 80 640 8 t
                        "Font-A" "Fontset-B" 1.0 'key)))
           "fontset change was absent from the immutable key")
          (remacs-vtable-3034-assert
           (not (equal base
                       (remacs-vtable-3034-run
                        table window frame 80 640 8 t
                        "Font-A" "Fontset-A" 2.0 'key)))
           "scale-factor change was absent from the immutable key")
          (let ((font-source (copy-sequence "Font-A"))
                (face-source (list :family (copy-sequence "Family-A")))
                (hash (make-hash-table :test #'equal)))
            (setf (vtable-face table) face-source)
            (let ((key (remacs-vtable-3034-run
                        table window frame 80 640 8 t
                        font-source "Fontset-A" 1.0 'key)))
              (puthash key 'saved hash)
              (aset font-source 0 ?X)
              (aset (cadr face-source) 0 ?X)
              (remacs-vtable-3034-assert
               (eq (gethash key hash) 'saved)
               "mutating source face/font strings corrupted the equal key")))
          (setf (vtable-face table) 'vtable)
          (setq base (remacs-vtable-3034-run
                      table window frame 80 640 8 t
                      "Font-A" "Fontset-A" 1.0 'key))
          (setq-local face-remapping-alist '((vtable (:height 1.5))))
          (remacs-vtable-3034-assert
           (not (equal base
                       (remacs-vtable-3034-run
                        table window frame 80 640 8 t
                        "Font-A" "Fontset-A" 1.0 'key)))
           "buffer face-remapping context was absent from the key")
          (let ((remapped-key
                 (remacs-vtable-3034-run
                  table window frame 80 640 8 t "Font-A" "Fontset-A" 1.0 'key)))
            (setq-local default-text-properties '(help-echo "unused"))
            (remacs-vtable-3034-assert
             (not (equal remapped-key
                         (remacs-vtable-3034-run
                          table window frame 80 640 8 t "Font-A" "Fontset-A" 1.0 'key)))
             "buffer default properties were absent from the key")
            (let ((default-key
                   (remacs-vtable-3034-run
                    table window frame 80 640 8 t "Font-A" "Fontset-A" 1.0 'key)))
              (setq-local char-property-alias-alist
                          '((remacs-vtable-3034-unused . display)))
              (remacs-vtable-3034-assert
               (not (equal default-key
                           (remacs-vtable-3034-run
                            table window frame 80 640 8 t "Font-A" "Fontset-A" 1.0 'key)))
               "buffer character-property aliases were absent from the key")))
          t))))
  (defun remacs-vtable-3034-check ()
    (require 'subr-x)
    (save-window-excursion
      (let* ((window-a (selected-window))
             (frame (window-frame window-a))
             (window-b (split-window window-a))
             (remacs-vtable-3034-target-window-a window-a)
             (remacs-vtable-3034-target-window-b window-b)
             (remacs-vtable-3034-target-frame frame))
        (remacs-vtable-3034-install-metric-stubs))))
  (remacs-vtable-3034-assert
   (remacs-vtable-3034-check)
   "minimal vtable cache-key regression failed")
  t)"##;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn eval(interp: &mut Interp, source: &str) -> String {
    let value = interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    format!("{}", interp.prin1_to_string(&value))
}

fn run_with_library(relative_path: &str) -> String {
    let mut interp = Interp::new();
    let library = PathBuf::from(env!("REMACS_SOURCE_ROOT")).join(relative_path);
    eval(
        &mut interp,
        &format!("(load {} nil t)", lisp_string(library.to_str().unwrap())),
    );
    eval(&mut interp, CASES)
}

#[test]
fn vtable_cache_key_tracks_pixel_width_and_character_metrics() {
    assert_eq!(run_with_library("lisp/vtable.el"), "t");
    assert_eq!(run_with_library("lisp-elc/vtable.elc"), "t");
}
