//! File byte decoding, gzip expansion, and GNU compiled-header classification.

use super::source::eval_src_kind_with_codes;
use crate::lisp::{Interp, error::EvalResult, value::Value};

struct DecodedLoad {
    text: crate::lisp::text::EmacsText,
    oversized_codes: Option<crate::lisp::reader::OversizedCodes>,
}

/// Decode GNU utf-8-emacs without collapsing extended characters or raw octets.
fn decode_load_bytes(bytes: &[u8], compiled: bool) -> DecodedLoad {
    if !compiled {
        return DecodedLoad {
            text: crate::lisp::builtins::coding_legacy::internal_decode(bytes),
            oversized_codes: None,
        };
    }
    // Direct compiled streams use Emacs's internal encoding: unlike source
    // conversion, this accepts surrogate codes and C0/C1 byte8 sequences.
    let mut text = crate::lisp::text::EmacsText::new();
    let mut oversized_codes = rustc_hash::FxHashMap::default();
    let mut position = 0;
    while position < bytes.len() {
        let first = bytes[position];
        let width = match first {
            0..=0x7f => 1,
            0xc0..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf7 => 4,
            0xf8..=0xff => 5,
            _ => 0,
        };
        let complete = width > 0
            && position + width <= bytes.len()
            && bytes[position + 1..position + width]
                .iter()
                .all(|byte| byte & 0xc0 == 0x80);
        let (code, width) = if !complete {
            (0x3fff00 + u32::from(first), 1)
        } else if width == 2 && first < 0xc2 {
            (
                0x3fff80 + (u32::from(first & 1) << 6) + u32::from(bytes[position + 1] & 0x3f),
                2,
            )
        } else {
            // GNU's direct reader accepts noncanonical sequences and masks the
            // five-byte result to the character's 22-bit representation.
            let mut code = u32::from(first & [0, 0x7f, 0x1f, 0x0f, 0x07, 0x07][width]);
            for byte in &bytes[position + 1..position + width] {
                code = (code << 6) | u32::from(byte & 0x3f);
            }
            (code, width)
        };
        if code > 0x3fffff {
            oversized_codes.insert(text.len(), code);
        }
        let character = crate::lisp::text::EmacsChar::new(code & 0x3fffff).unwrap();
        text.push(character);
        position += width;
    }
    DecodedLoad {
        text,
        oversized_codes: (!oversized_codes.is_empty()).then(|| std::rc::Rc::new(oversized_codes)),
    }
}

/// Read the file at PATH and evaluate all forms in it.
/// Binds `load-file-name` and `load-in-progress` like Emacs `load`.
pub fn eval_file<P: crate::lisp::text::TextSource + ?Sized>(
    i: &mut Interp,
    path: &P,
) -> EvalResult {
    eval_file_lex(i, path, false)
}

/// `--script FILE`: like `load`, but `lexical-binding' is forced on —
/// GNU's `command-line--load-script' does `setq-local lexical-binding t'
/// before eval-buffer, independent of any file cookie.
pub fn eval_file_script<P: crate::lisp::text::TextSource + ?Sized>(
    i: &mut Interp,
    path: &P,
) -> EvalResult {
    eval_file_lex(i, path, true)
}

fn eval_file_lex<P: crate::lisp::text::TextSource + ?Sized>(
    i: &mut Interp,
    path: &P,
    force_lex: bool,
) -> EvalResult {
    eval_file_lex_dumped(i, path, force_lex, false)
}

pub(super) fn eval_file_lex_dumped<P: crate::lisp::text::TextSource + ?Sized>(
    i: &mut Interp,
    path: &P,
    force_lex: bool,
    dumped_like: bool,
) -> EvalResult {
    let path = path.with_text(|text| text.to_owned());
    let native = crate::editor::native_path(i, &path)?;
    // Compressed files follow jka-compr's source path: only an actual
    // .elc suffix forces rejection when the compiler header is absent.
    let elc = path.ends_with(".elc");
    if !force_lex
        && elc
        && std::fs::metadata(&native)
            .is_ok_and(|metadata| !metadata.is_file() && !metadata.is_dir())
    {
        return Err(i.error_q_text("File `%s' was not compiled in Emacs", &[path.clone()]));
    }
    let bytes = std::fs::read(&native).map_err(|error| {
        let symbol = if error.kind() == std::io::ErrorKind::NotFound {
            i.q("file-missing")
        } else {
            crate::lisp::sym::FILE_ERROR
        };
        i.signal_data(
            symbol,
            vec![
                Value::string("Opening input file"),
                Value::string(error.to_string()),
                Value::string(path.clone()),
            ],
        )
    })?;
    let bytes = if path.ends_with(".gz") {
        gunzip(&bytes).ok_or_else(|| {
            i.signal_data(
                crate::lisp::sym::FILE_ERROR,
                vec![
                    Value::string("Opening input file"),
                    Value::string("Malformed or misplaced compressed data"),
                    Value::string(path.clone()),
                ],
            )
        })?
    } else {
        bytes
    };
    // Fload validates the compiler header before reading/evaluating forms.
    // --script uses eval-buffer instead and does not impose this check.
    let version = if force_lex {
        0
    } else {
        compiled_file_version(i, &bytes)?
    };
    if !force_lex && elc && version == 0 {
        return Err(i.error_q_text("File `%s' was not compiled in Emacs", &[path.clone()]));
    }
    let compiled = !force_lex && (elc || version > 1);
    let src = decode_load_bytes(&bytes, compiled);
    // Track load-file-name / load-in-progress like Emacs does.  GNU
    // binds `load-file-name' to the filename `openp' located — the
    // tried path itself, not its truename.
    eval_src_kind_with_codes(
        i,
        &path,
        &src.text,
        force_lex,
        dumped_like,
        compiled,
        src.oversized_codes.as_ref(),
    )
}

/// GNU safe_to_load_version: inspect at most 512 raw bytes, searching
/// after the first newline. Empty files return 1; zero rejects .elc.
/// The marker alone is insufficient, and historical headers remain valid.
fn compiled_file_version(i: &mut Interp, bytes: &[u8]) -> Result<i32, crate::lisp::Flow> {
    let bytes = &bytes[..bytes.len().min(512)];
    if bytes.is_empty() {
        return Ok(1);
    }
    let Some(newline) = bytes.iter().position(|byte| *byte == b'\n') else {
        return Ok(0);
    };
    let variable = i.q("bytecomp-version-regexp");
    let pattern = i.symbol_value(variable);
    let Value::Str(pattern) = &pattern else {
        return Err(i.wrong_type_mut("stringp", &pattern));
    };
    let regexp =
        crate::lisp::regexp::compile_case(pattern.borrow().as_str(), true).map_err(|error| {
            let symbol = i.q("invalid-regexp");
            i.signal_data(symbol, vec![Value::string(error.0)])
        })?;
    let chars: Vec<_> = bytes[newline..]
        .iter()
        .map(|byte| char::from(*byte))
        .collect();
    let syntax = crate::editor::re_syntax(i);
    if crate::lisp::regexp::search(
        &regexp,
        &chars,
        0,
        &syntax,
        &crate::editor::re_category(i),
        crate::lisp::regexp::NO_POINT,
    )
    .is_none()
    {
        return Ok(0);
    }
    Ok(if newline > 4 {
        i32::from(bytes[4] as i8)
    } else {
        1
    })
}

/// Decompress gzip data; None on malformed input.
fn gunzip(bytes: &[u8]) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut d = flate2::read::GzDecoder::new(bytes);
    let mut out = Vec::new();
    d.read_to_end(&mut out).ok().map(|_| out)
}
