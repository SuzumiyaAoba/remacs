//! Readable source paths for libraries carried only by the executable.
//! Publish files by rename: another interpreter must never load a partial
//! source. The private directory is shared only within this process.

use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static DIRECTORY: Mutex<Option<PathBuf>> = Mutex::new(None);

pub(crate) fn directory() -> Option<PathBuf> {
    DIRECTORY
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

pub(crate) fn materialize(key: &str, source: &str) -> io::Result<PathBuf> {
    let mut directory = DIRECTORY.lock().unwrap_or_else(|error| error.into_inner());
    if directory.is_none() {
        let temp = std::env::temp_dir();
        let temp = if temp.is_absolute() {
            temp
        } else {
            std::env::current_dir()?.join(temp)
        };
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for attempt in 0..64 {
            let candidate = temp.join(format!(
                "remacs-lisp-{}-{nonce}-{attempt}",
                std::process::id()
            ));
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&candidate) {
                Ok(()) => {
                    *directory = Some(candidate);
                    break;
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }
    let root = directory.as_ref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "Cannot create embedded source directory",
        )
    })?;
    let name = format!("{}.el", key.replace('/', "-"));
    let path = root.join(&name);
    if path.is_file() {
        return Ok(path);
    }
    let temporary = root.join(format!(".{name}.tmp"));
    let result = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(source.as_bytes())?;
        drop(file);
        std::fs::rename(&temporary, &path)?;
        Ok(path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}
