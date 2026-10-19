#[path = "../src/lisp/embedded_cache.rs"]
mod embedded_cache;

#[test]
fn concurrent_source_publication_never_exposes_partial_files() {
    let anchor = embedded_cache::materialize("qa53-cache-anchor", "nil\n").unwrap();
    let directory = embedded_cache::directory().unwrap();
    assert_eq!(anchor.parent(), Some(directory.as_path()));
    let source = "; source payload\n".repeat(262_144);
    let expected = source.clone();
    let path = directory.join("qa53-cache-concurrent.el");
    let writer = std::thread::spawn(move || {
        embedded_cache::materialize("qa53/cache-concurrent", &source).unwrap()
    });
    while !writer.is_finished() {
        if let Ok(bytes) = std::fs::read(&path) {
            assert_eq!(bytes, expected.as_bytes(), "partially published source");
        }
        std::thread::yield_now();
    }
    assert_eq!(writer.join().unwrap(), path);
    assert_eq!(std::fs::read(&path).unwrap(), expected.as_bytes());
    let readers: Vec<_> = (0..8)
        .map(|_| {
            let source = expected.clone();
            std::thread::spawn(move || {
                embedded_cache::materialize("qa53/cache-concurrent", &source).unwrap()
            })
        })
        .collect();
    for reader in readers {
        assert_eq!(reader.join().unwrap(), path);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn failed_publication_is_retryable_and_existing_sources_are_preserved() {
    let anchor = embedded_cache::materialize("qa53-error-anchor", "nil\n").unwrap();
    let root = anchor.parent().unwrap();
    let destination = root.join("qa53-cache-error.el");
    std::fs::create_dir(&destination).unwrap();
    assert!(embedded_cache::materialize("qa53/cache-error", "nil\n").is_err());
    assert!(!root.join(".qa53-cache-error.el.tmp").exists());
    std::fs::remove_dir(&destination).unwrap();
    let path = embedded_cache::materialize("qa53/cache-error", "original\n").unwrap();
    std::fs::write(&path, "edited\n").unwrap();
    assert_eq!(
        embedded_cache::materialize("qa53/cache-error", "original\n").unwrap(),
        path
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), "edited\n");
}
