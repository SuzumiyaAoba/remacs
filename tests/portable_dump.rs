//! Portable images are exercised through separate copied executables, not
//! an in-process serializer round trip. Every fixture has independent paths.
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "remacs-pdump-{}-{nonce}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        for directory in ["bin", "work", "producer-cache", "consumer-cache"] {
            std::fs::create_dir_all(root.join(directory)).unwrap();
        }
        std::fs::copy(env!("CARGO_BIN_EXE_remacs"), root.join("bin/remacs")).unwrap();
        Self(root)
    }
    fn binary(&self) -> PathBuf {
        self.0.join("bin/remacs")
    }
    fn image(&self) -> PathBuf {
        self.0.join("work/image.pdmp")
    }
    fn eval(&self, cache: &str, args: &[&str], code: &str) -> Output {
        Command::new(self.binary())
            .current_dir(self.0.join("work"))
            .env("TMPDIR", self.0.join(cache))
            .env_remove("EMACSLOADPATH")
            .env_remove("REMACS_NO_ELC")
            .args(["-Q", "--batch"])
            .args(args)
            .args(["--eval", code])
            .output()
            .unwrap()
    }
    fn ok(output: Output) -> String {
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn literal(path: &Path) -> String {
    format!("{:?}", path.to_string_lossy())
}

#[test]
fn image_restores_shared_lisp_state_buffers_and_reports_actual_startup() {
    let fixture = Fixture::new();
    let code = format!(
        r#"(progn
      (setq dump112-cycle (list 'payload)) (setcdr dump112-cycle dump112-cycle)
      (setq dump112-alias dump112-cycle)
      (setq dump112-shared (eval '(let ((n 6)) (list (lambda () (setq n (1+ n))) (lambda () n))) t))
      (defun dump112-function (arg) (+ arg 5))
      (defalias 'dump112-function-alias 'dump112-function)
      (advice-add 'dump112-function :filter-return (lambda (value) (* value 2)))
      (setq dump112-float 1.25 dump112-hash (make-hash-table :test 'eq))
      (puthash dump112-float dump112-cycle dump112-hash)
      (setq dump112-symbol (make-symbol "dump112-name"))
      (setq dump124-name (propertize (string-to-multibyte "dump124-name") 'dump124-tag 124))
      (setq dump124-symbol (make-symbol dump124-name))
      (setq dump112-obarray (obarray-make))
      (setq dump112-other-symbol (intern "dump112-only-custom" dump112-obarray))
      (setq dump-byte-name (concat "dump-byte-key-" (unibyte-string 255))
            dump-multi-name (concat "dump-byte-key-" (string #x3fffff))
            dump-byte-symbol (intern dump-byte-name)
            dump-multi-symbol (intern dump-multi-name))
      (setq dump-mutated-symbol (intern (copy-sequence "dump-mutated-name-x")))
      (aset (symbol-name dump-mutated-symbol) 18 ?y)
      (setq dump-original-name-symbol (intern "dump-mutated-name-x"))
      (string #x110005) (setq dump112-mule (string #x110007 #x110001))
      (setq dump112-bytes (unibyte-string 0 127 128 255))
      (setq dump112-buffer (get-buffer-create "dump112-buffer"))
      (with-current-buffer dump112-buffer
        (insert "日本語 abc") (put-text-property 1 3 'dump-property dump112-cycle)
        (narrow-to-region 2 7) (goto-char 4)
        (set (make-local-variable 'dump-local) dump112-cycle))
      (setq dump112-indirect (make-indirect-buffer dump112-buffer "dump112-indirect"))
      (setq dump114-unibyte (get-buffer-create "dump114-unibyte"))
      (with-current-buffer dump114-unibyte
        (set-buffer-multibyte nil) (insert (unibyte-string 128 255)))
      (setq dump112-marker (with-current-buffer dump112-buffer (copy-marker 4)))
      (setq dump112-overlay (with-current-buffer dump112-buffer (make-overlay 2 3)))
      (overlay-put dump112-overlay 'dump-property dump112-cycle) (delete-overlay dump112-overlay)
      (setq dump128-overlay (with-current-buffer dump112-buffer (make-overlay 2 3)))
      (overlay-put dump128-overlay 'dump-property dump112-cycle) (delete-overlay dump128-overlay)
      (setq dump128-index (make-hash-table :test 'equal))
      (puthash dump112-overlay 'overlay-value dump128-index)
      (let ((source (generate-new-buffer " *dump307-source*")))
        (with-current-buffer source (insert "abcd"))
        (setq dump307-killed (make-overlay 1 3 source t t))
        (overlay-put dump307-killed 'dump-property dump112-cycle)
        (kill-buffer source))
      (setq after-pdump-load-hook (list (lambda () (setq dump112-hook-ran t))))
      (let ((original (symbol-function 'format)) (active nil))
        (unwind-protect
            (progn
              (fset 'format (lambda (&rest args)
                              (when active
                                (setq dump-gensym-format (car args) dump-gensym-prefix (cadr args)))
                              (apply original args)))
              (setq active t) (gensym))
          (setq active nil) (fset 'format original)))
      (aset dump-gensym-prefix 0 ?p)
      (dump-emacs-portable {} t)
      (prin1 (list (pdumper-stats) (> (file-attribute-size (file-attributes {})) 100))))"#,
        literal(&fixture.image()),
        literal(&fixture.image())
    );
    assert_eq!(
        Fixture::ok(fixture.eval("producer-cache", &[], &code)),
        "(nil t)"
    );
    std::fs::remove_dir_all(fixture.0.join("producer-cache")).unwrap();
    let code = format!(
        r#"(progn
      (require 'avl-tree)
      (let ((stats (pdumper-stats)))
        (prin1 (list
          (eq dump112-cycle dump112-alias)
          (eq dump112-cycle (cdr dump112-cycle))
          (= (dump112-function-alias 3) 16)
          (= (funcall (car dump112-shared)) 7) (= (funcall (cadr dump112-shared)) 7)
          (eq (gethash dump112-float dump112-hash) dump112-cycle)
          (not (intern-soft (symbol-name dump112-symbol)))
          (eq dump124-name (symbol-name dump124-symbol))
          (= (get-text-property 0 'dump124-tag (symbol-name dump124-symbol)) 124)
          (multibyte-string-p (symbol-name dump124-symbol))
          (progn (put-text-property 0 1 'dump124-later t dump124-name)
                 (get-text-property 0 'dump124-later (symbol-name dump124-symbol)))
          (eq (intern-soft "dump112-only-custom" dump112-obarray) dump112-other-symbol)
          (not (intern-soft "dump112-only-custom"))
          (not (eq dump-byte-symbol dump-multi-symbol))
          (eq dump-byte-symbol (intern dump-byte-name))
          (eq dump-multi-symbol (intern dump-multi-name))
          (eq dump-byte-name (symbol-name dump-byte-symbol))
          (eq dump-multi-name (symbol-name dump-multi-symbol))
          (not (eq dump-mutated-symbol dump-original-name-symbol))
          (eq (intern "dump-mutated-name-x") dump-original-name-symbol)
          (let ((original (symbol-function 'format)) (active nil) identity (gensym-counter 200))
            (unwind-protect
                (progn
                  (fset 'format (lambda (&rest args)
                                  (when active
                                    (setq identity (list (eq (car args) dump-gensym-format)
                                                         (eq (cadr args) dump-gensym-prefix))))
                                  (apply original args)))
                  (setq active t)
                  (and (equal (symbol-name (gensym)) "p200") (equal identity '(t t))))
              (setq active nil) (fset 'format original)))
          (equal (string-to-list dump112-mule) '(1114119 1114113))
          (not (multibyte-string-p dump112-bytes)) (= (aref dump112-bytes 2) 128) (= (aref dump112-bytes 3) 255)
          (and (with-current-buffer dump112-buffer
            (and (= (point) 4) (= (point-min) 2) (= (point-max) 7)
                 (= (gap-position) 8) (= (gap-size) 7)
                 (eq dump-local dump112-cycle) (eq (get-text-property 2 'dump-property) dump112-cycle)))
               (with-current-buffer dump114-unibyte
                 (and (not enable-multibyte-characters) (= (gap-position) 3) (= (gap-size) 18))))
          (eq (buffer-base-buffer dump112-indirect) dump112-buffer)
          (eq (marker-buffer dump112-marker) dump112-buffer) (= (marker-position dump112-marker) 4)
          (overlayp dump112-overlay) (not (overlay-buffer dump112-overlay))
          (eq (overlay-get dump112-overlay 'dump-property) dump112-cycle)
          (equal dump112-overlay dump128-overlay)
          (eq (gethash dump128-overlay dump128-index) 'overlay-value)
          (overlayp dump307-killed) (not (overlay-buffer dump307-killed))
          (eq (overlay-get dump307-killed 'dump-property) dump112-cycle)
          (with-temp-buffer
            (insert "abcd") (move-overlay dump307-killed 2 4)
            (and (eq (overlay-buffer dump307-killed) (current-buffer))
                 (= (overlay-start dump307-killed) 2)
                 (= (overlay-end dump307-killed) 4)
                 (eq (overlay-get dump307-killed 'dump-property) dump112-cycle)))
          dump112-hook-ran
          (eq (cdr (assq 'dumped-with-pdumper stats)) t)
          (floatp (cdr (assq 'load-time stats))) (> (cdr (assq 'load-time stats)) 0)
          (equal (cdr (assq 'dump-file-name stats)) {})
          (not (member "--dump-file" command-line-args))
          (let ((path (locate-library "avl-tree")))
            (and (file-readable-p path)
                 (member (directory-file-name (file-name-directory path))
                         (mapcar #'directory-file-name load-path)) t))))))"#,
        literal(&fixture.image())
    );
    let result = Fixture::ok(fixture.eval(
        "consumer-cache",
        &["--dump-file", fixture.image().to_str().unwrap()],
        &code,
    ));
    assert_eq!(result, format!("({})", vec!["t"; 45].join(" ")));
    let equals = format!("--dump-file={}", fixture.image().display());
    assert_eq!(
        Fixture::ok(fixture.eval(
            "consumer-cache",
            &[&equals],
            "(prin1 (list (dump112-function 3) (cdr (assq 'dumped-with-pdumper (pdumper-stats)))))"
        )),
        "(16 t)"
    );
    let sidecar = fixture.binary().with_extension("pdmp");
    std::fs::copy(fixture.image(), &sidecar).unwrap();
    let code = format!(
        "(prin1 (list (dump112-function 3) (equal (cdr (assq 'dump-file-name (pdumper-stats))) {})))",
        literal(&sidecar)
    );
    assert_eq!(
        Fixture::ok(fixture.eval("consumer-cache", &[], &code)),
        "(16 t)"
    );
}

#[test]
fn invalid_images_fail_before_command_execution() {
    let fixture = Fixture::new();
    let code = format!("(dump-emacs-portable {})", literal(&fixture.image()));
    Fixture::ok(fixture.eval("producer-cache", &[], &code));
    let bytes = std::fs::read(fixture.image()).unwrap();
    for case in ["magic", "fingerprint", "checksum", "truncated"] {
        let path = fixture.0.join(format!("{case}.pdmp"));
        let mut data = bytes.clone();
        match case {
            "magic" => data[0] ^= 1,
            "fingerprint" => data[16] ^= 1,
            "checksum" => {
                let last = data.len() - 1;
                data[last] ^= 1;
            }
            _ => {
                data.truncate(data.len() - 7);
            }
        }
        std::fs::write(&path, data).unwrap();
        let output = fixture.eval(
            "consumer-cache",
            &["--dump-file", path.to_str().unwrap()],
            "(princ \"COMMAND-RAN\")",
        );
        assert!(!output.status.success(), "accepted {case}");
        assert!(!String::from_utf8_lossy(&output.stdout).contains("COMMAND-RAN"));
    }
}

#[test]
fn image_restores_arbitrary_integers_and_identity_keys() {
    let fixture = Fixture::new();
    let producer = format!(
        "(progn (setq dump2-number (expt 2 130) dump2-alias dump2-number \
                       dump2-distinct (expt 2 130) dump2-index (make-hash-table :test 'eq)) \
                (puthash dump2-number 'saved dump2-index) \
                (dump-emacs-portable {} t) (princ \"created\"))",
        literal(&fixture.image())
    );
    assert_eq!(
        Fixture::ok(fixture.eval("producer-cache", &[], &producer)),
        "created"
    );
    let consumer = "(prin1 (list (integerp dump2-number) (bignump dump2-number) \
                            (eq dump2-number dump2-alias) (eq dump2-number dump2-distinct) \
                            (eql dump2-number dump2-distinct) \
                            (gethash dump2-number dump2-index) (gethash dump2-distinct dump2-index) \
                            (number-to-string (1+ dump2-number))))";
    assert_eq!(
        Fixture::ok(fixture.eval(
            "consumer-cache",
            &["--dump-file", fixture.image().to_str().unwrap()],
            consumer
        )),
        r#"(t t t nil t saved nil "1361129467683753853853498429727072845825")"#
    );
}

#[test]
fn image_restores_equal_hash_keys_for_distinct_obarrays() {
    let fixture = Fixture::new();
    let producer = format!(
        "(progn (setq dump479-left (obarray-make) dump479-right (obarray-make) \
                      dump479-table (make-hash-table :test 'equal)) \
                (puthash dump479-left 'left dump479-table) \
                (puthash dump479-right 'right dump479-table) \
                (dump-emacs-portable {} t) (princ \"created\"))",
        literal(&fixture.image())
    );
    assert_eq!(
        Fixture::ok(fixture.eval("producer-cache", &[], &producer)),
        "created"
    );
    let restored = fixture.eval(
        "consumer-cache",
        &["--dump-file", fixture.image().to_str().unwrap()],
        "(prin1 (list (equal dump479-left dump479-right)\
                      (hash-table-count dump479-table)\
                      (gethash dump479-left dump479-table)\
                      (gethash dump479-right dump479-table)))",
    );
    assert_eq!(Fixture::ok(restored), "(nil 2 left right)");
}

#[test]
fn image_restores_autosave_counters_for_base_and_indirect_buffers() {
    let fixture = Fixture::new();
    let producer = format!(
        r#"(progn
          (setq dump155-base (get-buffer-create "dump155-base"))
          (with-current-buffer dump155-base
            (insert "x") (restore-buffer-modified-p 'autosaved))
          (setq dump155-indirect (make-indirect-buffer dump155-base "dump155-indirect"))
          (dump-emacs-portable {} t)
          (prin1 (list (buffer-modified-p dump155-base)
                       (buffer-modified-p dump155-indirect))))"#,
        literal(&fixture.image())
    );
    assert_eq!(
        Fixture::ok(fixture.eval("producer-cache", &[], &producer)),
        "(autosaved t)"
    );
    std::fs::remove_dir_all(fixture.0.join("producer-cache")).unwrap();
    let consumer = r#"(prin1 (list
        (buffer-modified-p dump155-base)
        (buffer-modified-p dump155-indirect)
        (with-current-buffer dump155-indirect
          (insert "y")
          (list (buffer-modified-p dump155-base)
                (buffer-modified-p dump155-indirect)))))"#;
    assert_eq!(
        Fixture::ok(fixture.eval(
            "consumer-cache",
            &["--dump-file", fixture.image().to_str().unwrap()],
            consumer
        )),
        "(autosaved t (t t))"
    );
}

#[test]
fn restored_weak_tables_keep_live_keys_and_release_unreachable_keys() {
    let fixture = Fixture::new();
    let producer = format!(
        r#"(progn
          (setq dump194-table (make-hash-table :test 'eq :weakness 'key))
          (setq dump194-keep (vector 'keep))
          (puthash dump194-keep 'retained dump194-table)
          (dotimes (n 1000) (puthash (vector n) n dump194-table))
          (dump-emacs-portable {} t) (prin1 'ready))"#,
        literal(&fixture.image())
    );
    assert_eq!(
        Fixture::ok(fixture.eval("producer-cache", &[], &producer)),
        "ready"
    );
    std::fs::remove_dir_all(fixture.0.join("producer-cache")).unwrap();
    let consumer = "(let ((capacity (hash-table-size dump194-table))) \
        (garbage-collect) (garbage-collect) \
        (prin1 (list capacity (hash-table-size dump194-table) \
          (hash-table-count dump194-table) (gethash dump194-keep dump194-table))))";
    assert_eq!(
        Fixture::ok(fixture.eval(
            "consumer-cache",
            &["--dump-file", fixture.image().to_str().unwrap()],
            consumer
        )),
        "(1536 1536 1 retained)"
    );
}

#[test]
fn image_restores_recursive_mutex_ownership() {
    let fixture = Fixture::new();
    let producer = format!(
        "(progn (setq dump196-mutex (make-mutex \"recursive\")) \
          (mutex-lock dump196-mutex) (mutex-lock dump196-mutex) \
          (dump-emacs-portable {} t) (princ \"ready\"))",
        literal(&fixture.image())
    );
    assert_eq!(
        Fixture::ok(fixture.eval("producer-cache", &[], &producer)),
        "ready"
    );
    let consumer = "(progn (mutex-unlock dump196-mutex) \
        (let ((worker (make-thread (lambda () (with-mutex dump196-mutex 42))))) \
          (thread-yield) \
          (let ((blocked (eq (thread--blocker worker) dump196-mutex))) \
            (mutex-unlock dump196-mutex) (prin1 (list blocked (thread-join worker))))))";
    assert_eq!(
        Fixture::ok(fixture.eval(
            "consumer-cache",
            &["--dump-file", fixture.image().to_str().unwrap()],
            consumer
        )),
        "(t 42)"
    );
}
