//! Portable interpreter images. The graph codec preserves object identity;
//! session roots and OS-backed resources are handled separately.

mod graph;
mod runtime;
mod session;
mod sources;

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::OnceLock;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::value::{Argv, Value};
use super::{Flow, Interp};
use session::Image;

const MAGIC: &[u8] = b"REMACS-PDMP\0V1\n";
const HEADER: usize = MAGIC.len() + 32 + 32 + 8;
static EXECUTABLE: OnceLock<[u8; 32]> = OnceLock::new();

#[derive(Clone)]
pub(crate) struct LoadedDump {
    pub file_name: String,
    pub load_time: f64,
}

#[derive(Debug)]
pub enum LoadError {
    Message(String),
    Exit(i128),
}
impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Message(message) => f.write_str(message),
            Self::Exit(code) => write!(f, "Startup exited with status {code}"),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Archive {
    version: u32,
    image: Image,
    track_referrers: bool,
    referrers: Option<Vec<Vec<u32>>>,
}

fn executable_fingerprint() -> std::result::Result<[u8; 32], String> {
    if let Some(hash) = EXECUTABLE.get() {
        return Ok(*hash);
    }
    let mut file = File::open(std::env::current_exe().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut bytes = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut bytes).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&bytes[..n]);
    }
    let hash: [u8; 32] = hasher.finalize().into();
    let _ = EXECUTABLE.set(hash);
    Ok(hash)
}

fn io_error(i: &mut Interp, label: &str, path: &str, error: std::io::Error) -> Flow {
    let name = match error.kind() {
        std::io::ErrorKind::NotFound => "file-missing",
        std::io::ErrorKind::PermissionDenied => "permission-denied",
        _ => "file-error",
    };
    let symbol = i.intern(name);
    i.signal_data(
        symbol,
        vec![
            Value::string(label),
            Value::string(crate::editor::file_error_string(&error)),
            Value::string(path),
        ],
    )
}

pub(crate) fn save(i: &mut Interp, args: Argv) -> super::EvalResult {
    if !i.noninteractive {
        return Err(i.error("Dumping Emacs currently works only in batch mode.  If you'd like it to work interactively, please consider contributing a patch to Emacs."));
    }
    if i.current_thread != 0 {
        return Err(i.error("This function can be called only in the main thread"));
    }
    if i.threads
        .iter()
        .filter(|thread| thread.borrow().alive)
        .count()
        > 1
    {
        return Err(i.error("No other Lisp threads can be running when this function is called"));
    }
    super::memory::collect_interp(i);
    let mark = i.specbind_depth();
    let processed = i.intern("command-line-processed");
    i.specbind(processed, Value::Nil)?;
    let result = (|| {
        let Value::Str(name) = &args[0] else {
            return Err(i.wrong_type_mut("stringp", &args[0]));
        };
        let name = name.borrow().clone();
        let path = crate::editor::expand_file_name_str(i, &name);
        let mut options = std::fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o666);
        }
        let mut file = options
            .open(&path)
            .map_err(|e| io_error(i, "Opening dump output", &path, e))?;
        let image = Image::capture(i).map_err(|e| i.error(e))?;
        let track_referrers = args.get(1).is_some_and(Value::truthy);
        let referrers = track_referrers.then(|| {
            let mut reverse = vec![Vec::new(); image.nodes.len()];
            for (index, node) in image.nodes.iter().enumerate() {
                for target in node.references() {
                    reverse[target as usize].push(index as u32);
                }
            }
            reverse
        });
        let archive = Archive {
            version: 1,
            image,
            track_referrers,
            referrers,
        };
        let mut compressor =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        serde_json::to_writer(&mut compressor, &archive)
            .map_err(|e| i.error(format!("Cannot encode portable dump: {e}")))?;
        let body = compressor
            .finish()
            .map_err(|e| io_error(i, "Writing dump output", &path, e))?;
        let fingerprint = executable_fingerprint()
            .map_err(|e| i.error(format!("Cannot fingerprint executable: {e}")))?;
        let checksum: [u8; 32] = Sha256::digest(&body).into();
        let mut header = Vec::with_capacity(HEADER);
        header.extend_from_slice(MAGIC);
        header[0] = b'!';
        header.extend_from_slice(&fingerprint);
        header.extend_from_slice(&checksum);
        header.extend_from_slice(&(body.len() as u64).to_le_bytes());
        file.write_all(&header)
            .and_then(|_| file.write_all(&body))
            .and_then(|_| file.flush())
            .map_err(|e| io_error(i, "Writing dump output", &path, e))?;
        // A partially written file must never look like a complete image.
        file.seek(SeekFrom::Start(0))
            .and_then(|_| file.write_all(&MAGIC[..1]))
            .and_then(|_| file.flush())
            .map_err(|e| io_error(i, "Finishing dump output", &path, e))?;
        Ok(Value::Nil)
    })();
    i.unbind_to(mark)?;
    result
}

pub(crate) fn stats(i: &mut Interp, _args: Argv) -> super::EvalResult {
    let Some(info) = i.loaded_dump.clone() else {
        return Ok(Value::Nil);
    };
    Ok(Value::list(vec![
        Value::cons(Value::Sym(i.intern("dumped-with-pdumper")), Value::t()),
        Value::cons(
            Value::Sym(i.intern("load-time")),
            Value::float(info.load_time),
        ),
        Value::cons(
            Value::Sym(i.intern("dump-file-name")),
            Value::string(info.file_name),
        ),
    ]))
}

const STARTUP_VALUES: &[&str] = &[
    "command-line-args",
    "command-line-args-left",
    "command-line-processed",
    "process-environment",
    "initial-environment",
    "invocation-name",
    "invocation-directory",
    "installation-directory",
    "noninteractive",
    "window-system",
    "initial-window-system",
    "before-init-time",
    "after-init-time",
    "standard-input",
    "standard-output",
    "load-file-name",
    "load-true-file-name",
    "load-in-progress",
    "current-load-list",
    "lexical-binding",
    "last-event-frame",
];

/// Restore a fresh interpreter. Images never mutate an already executing
/// Lisp stack, and no raw heap/function addresses are read from the file.
pub fn load(path: &Path) -> std::result::Result<Interp, LoadError> {
    load_with_arguments(path, None)
}

pub fn load_with_arguments(
    path: &Path,
    arguments: Option<&[String]>,
) -> std::result::Result<Interp, LoadError> {
    let start = Instant::now();
    let fail = |message: String| LoadError::Message(format!("{}: {message}", path.display()));
    let mut file = File::open(path).map_err(|e| fail(e.to_string()))?;
    let mut header = vec![0u8; HEADER];
    file.read_exact(&mut header)
        .map_err(|e| fail(format!("Incomplete portable dump: {e}")))?;
    if &header[..MAGIC.len()] != MAGIC {
        return Err(fail("Invalid portable dump format".into()));
    }
    let fingerprint = executable_fingerprint().map_err(&fail)?;
    if header[MAGIC.len()..MAGIC.len() + 32] != fingerprint {
        return Err(fail(
            "Portable dump belongs to another executable build".into(),
        ));
    }
    let length = u64::from_le_bytes(header[HEADER - 8..].try_into().unwrap());
    if length.checked_add(HEADER as u64)
        != Some(file.metadata().map_err(|e| fail(e.to_string()))?.len())
    {
        return Err(fail("Incorrect portable dump length".into()));
    }
    let mut body = Vec::new();
    file.read_to_end(&mut body)
        .map_err(|e| fail(e.to_string()))?;
    let checksum: [u8; 32] = Sha256::digest(&body).into();
    if header[MAGIC.len() + 32..MAGIC.len() + 64] != checksum {
        return Err(fail("Portable dump checksum mismatch".into()));
    }
    let decompressor = flate2::read::ZlibDecoder::new(body.as_slice());
    let mut archive: Archive = serde_json::from_reader(decompressor)
        .map_err(|e| fail(format!("Invalid portable dump data: {e}")))?;
    if archive.version != 1 {
        return Err(fail("Unsupported portable dump version".into()));
    }
    let mut i = Interp::new();
    let load_path = i.intern("load-path");
    let fresh_path = i.symbol_value(load_path);
    let current_cache = super::embedded_cache::directory()
        .map(|p| p.to_string_lossy().into_owned())
        .or_else(|| {
            fresh_path
                .list_to_vec()
                .ok()?
                .into_iter()
                .find_map(|v| match v {
                    Value::Str(s) => {
                        let path = std::path::PathBuf::from(s.borrow().as_str());
                        ["simple.el", "simple.elc", "simple.el.gz"]
                            .iter()
                            .any(|name| path.join(name).is_file())
                            .then(|| path.to_string_lossy().into_owned())
                    }
                    _ => None,
                })
        });
    if let Some(cache) = current_cache {
        archive.image.relocate_source_cache(&cache);
    }
    archive.image.relocate_bundled_sources().map_err(&fail)?;
    let fresh: Vec<_> = STARTUP_VALUES
        .iter()
        .filter_map(|name| {
            i.intern_soft(name)
                .map(|id| ((*name).to_owned(), i.obarray.symbol(id).value.clone()))
        })
        .collect();
    let file_name = crate::editor::expand_file_name_str(&mut i, &path.to_string_lossy());
    i.reset_dump_execution();
    archive.image.restore(&mut i).map_err(&fail)?;
    for (name, value) in fresh {
        let id = i.intern(&name);
        i.obarray.symbol_mut(id).value = value;
    }
    if std::env::var_os("EMACSLOADPATH").is_some() {
        let id = i.intern("load-path");
        i.obarray.symbol_mut(id).value = fresh_path;
    }
    i.loaded_dump = Some(LoadedDump {
        file_name,
        load_time: start.elapsed().as_secs_f64(),
    });
    if let Some(arguments) = arguments {
        i.reset_dump_arguments(arguments);
    }
    match super::builtins::evalfn::call_hook(&mut i, "after-pdump-load-hook") {
        Ok(_) => Ok(i),
        Err(Flow::Exit(code)) => Err(LoadError::Exit(code)),
        Err(flow) => {
            let message = match flow {
                Flow::Signal(symbol, data, _) => format!(
                    "{} {}",
                    i.prin1_to_string(&symbol),
                    i.prin1_to_string(&data)
                ),
                other => format!("{other:?}"),
            };
            Err(fail(format!("after-pdump-load-hook: {message}")))
        }
    }
}
