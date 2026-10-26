//! Byte-preserving digest/base64 primitives.

use crate::lisp::builtins::{arg, want_string};
use crate::lisp::{
    Interp,
    error::{EvalResult, Flow},
    obarray::sym,
    value::Value,
};

// ---------- hashing / base64 ----------

fn hash_bytes(algo: &str, bytes: &[u8]) -> Result<Vec<u8>, Flow> {
    use sha2::Digest;
    let out = match algo {
        "md5" => md5::Md5::digest(bytes).to_vec(),
        "sha1" => sha1::Sha1::digest(bytes).to_vec(),
        "sha224" => sha2::Sha224::digest(bytes).to_vec(),
        "sha256" => sha2::Sha256::digest(bytes).to_vec(),
        "sha384" => sha2::Sha384::digest(bytes).to_vec(),
        "sha512" => sha2::Sha512::digest(bytes).to_vec(),
        "sha3-224" => sha3::Sha3_224::digest(bytes).to_vec(),
        "sha3-256" => sha3::Sha3_256::digest(bytes).to_vec(),
        "sha3-384" => sha3::Sha3_384::digest(bytes).to_vec(),
        "sha3-512" => sha3::Sha3_512::digest(bytes).to_vec(),
        _ => return Err(Flow::signal(Value::Nil, Value::Nil)),
    };
    Ok(out)
}

fn digest_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hash_hex(algo: &str, bytes: &[u8]) -> Result<String, Flow> {
    hash_bytes(algo, bytes).map(|digest| digest_hex(&digest))
}

fn secure_hash_bytes(i: &mut Interp, args: &[Value]) -> Result<Vec<u8>, Flow> {
    // GNU Fsecure_hash: CHECK_SYMBOL first — a non-symbol algorithm
    // signals `wrong-type-argument symbolp'.
    let algo_sym = match i.sym_id(&args[0]) {
        Some(s) => s,
        None => return Err(i.wrong_type_mut("symbolp", &args[0])),
    };
    let algo = i.symbol_name(algo_sym);
    let obj = &args[1];
    // GNU hashes the string's byte representation: unibyte strings
    // hash their raw bytes, multibyte their internal multibyte form
    // (eight-bit chars as single 0x80–0xFF bytes).  Raw UTF-8 of the
    // PUA proxy is WRONG — `lisp_string_bytes' knows the mapping.
    let mut bytes: Vec<u8> = match obj {
        Value::Str(_) => lisp_string_bytes(i, obj),
        Value::Buffer(b) => {
            // Same byte mapping as lisp_string_bytes but over buffer
            // text (buffer may hold eight-bit proxy chars).
            let mut out = Vec::new();
            for c in b.borrow().text.text().chars() {
                if let Some(byte) = crate::lisp::value::eight_bit_byte(c) {
                    out.push(byte);
                } else {
                    c.encode_internal(&mut out);
                }
            }
            out
        }
        Value::Nil => {
            // GNU Fsecure_hash: nil isn't the current buffer — it
            // falls past STRINGP/BUFFERP/OVERLAYP into
            // (error "Invalid object argument" "nil").
            return Err(i.signal_data(
                sym::ERROR,
                vec![
                    Value::string("Invalid object argument"),
                    Value::string("nil"),
                ],
            ));
        }
        _ => {
            // GNU signal_error builds data = cons(message, OBJECT):
            // a LIST object splices into the data tail, an ATOM
            // stays as the single last element:
            // `(sha1 '(a b))' → ("Invalid object argument" a b),
            // `(sha1 'a)' → ("Invalid object argument" a).
            let tail = match &obj {
                Value::Cons(_) => obj.clone(),
                other => Value::cons((*other).clone(), Value::Nil),
            };
            return Err(i.signal(
                sym::ERROR,
                Value::cons(Value::string("Invalid object argument"), tail),
            ));
        }
    };
    // GNU: (secure-hash ALGORITHM OBJECT &optional START END BINARY) —
    // hash only OBJECT[START..END] (byte offsets); nil means default.
    if let Some(st) = args.get(2) {
        // GNU validate_subarray uses CHECK_INTEGER on START/END —
        // `integerp' (floats, conses all rejected).
        let start = match st {
            Value::Nil => 0usize,
            Value::Int(n) => (*n).max(0) as usize,
            other => return Err(i.wrong_type_mut("integerp", other)),
        };
        let end = match args.get(3) {
            Some(Value::Nil) | None => bytes.len(),
            Some(Value::Int(n)) => (*n).max(0) as usize,
            Some(v) => return Err(i.wrong_type_mut("integerp", v)),
        };
        if start > bytes.len() || end > bytes.len() || start > end {
            return Err(i.signal_data(
                sym::ARGS_OUT_OF_RANGE,
                vec![
                    obj.clone(),
                    Value::Int(start as i128),
                    Value::Int(end as i128),
                ],
            ));
        }
        bytes = bytes[start..end].to_vec();
    }
    hash_bytes(&algo, &bytes).map_err(|_| {
        i.signal_data(
            sym::ERROR,
            vec![Value::string(format!("Invalid algorithm arg: {}", algo))],
        )
    })
}

pub(super) fn f_secure_hash(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let digest = secure_hash_bytes(i, &args)?;
    if arg(&args, 4).truthy() {
        // Digest octets are unibyte characters, including NUL and 0x80..0xff.
        let value = Value::string(digest.iter().map(|&byte| byte as char).collect::<String>());
        if let Value::Str(string) = &value {
            i.mark_unibyte(string);
        }
        Ok(value)
    } else {
        Ok(Value::string(digest_hex(&digest)))
    }
}

pub(super) fn f_md5(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let mut a = vec![Value::Sym(i.q("md5"))];
    a.extend_from_slice(args.as_slice());
    // MD5's optional coding-system argument must not become BINARY.
    Ok(Value::string(digest_hex(&secure_hash_bytes(i, &a)?)))
}

/// The byte sequence a Lisp string stands for: unibyte strings use
/// their chars' values directly; multibyte strings encode each char
/// as UTF-8 (eight-bit chars emit their raw byte).
pub(crate) fn lisp_string_bytes(i: &Interp, value: &Value) -> Vec<u8> {
    let Value::Str(reference) = value else {
        return Vec::new();
    };
    let unibyte = i.is_unibyte_str(reference);
    let text = reference.borrow();
    if unibyte {
        text.chars()
            .map(|c| c.byte8().unwrap_or(c.code() as u8))
            .collect()
    } else {
        text.to_external_bytes()
    }
}

pub(super) fn f_b64_encode_string(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    use base64::Engine;
    let _s = want_string(i, &args[0])?;
    let no_break = arg(&args, 1).truthy();
    // GNU pads unconditionally; NO-LINE-BREAK only suppresses the
    // 76-column line wrapping.
    let enc = base64::engine::general_purpose::STANDARD.encode(lisp_string_bytes(i, &args[0]));
    let out = if no_break || enc.len() <= 76 {
        enc
    } else {
        let mut wrapped = String::with_capacity(enc.len() + enc.len() / 76);
        for chunk in enc.as_bytes().chunks(76) {
            wrapped.push_str(std::str::from_utf8(chunk).unwrap_or(""));
            wrapped.push('\n');
        }
        wrapped.pop();
        wrapped
    };
    Ok(Value::string(out))
}

pub(super) fn f_b64_decode_string(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU base64_decode_1 (lib/base64.c via fns.c):
    // - optional BASE64URL switches to the RFC 4648 URL alphabet,
    // - optional IGNORE-INVALID drops non-alphabet chars instead of
    //   erroring,
    // - whitespace is always skipped,
    // - '=' padding ends the input; a trailing partial group of 2 or
    //   3 chars still decodes (1 leftover char signals), and padding
    //   is *not required* — GNU pads implicitly.
    let s = want_string(i, &args[0])?;
    let url = arg(&args, 1).truthy();
    let ignore_invalid = arg(&args, 2).truthy();
    // Alphabet lookup: index of c or 0x100/0x101 sentinel.
    let b64val = |c: u8| -> u32 {
        if url {
            match c {
                b'A'..=b'Z' => (c - b'A') as u32,
                b'a'..=b'z' => (c - b'a' + 26) as u32,
                b'0'..=b'9' => (c - b'0' + 52) as u32,
                b'-' => 62,
                b'_' => 63,
                _ => 0x100,
            }
        } else {
            match c {
                b'A'..=b'Z' => (c - b'A') as u32,
                b'a'..=b'z' => (c - b'a' + 26) as u32,
                b'0'..=b'9' => (c - b'0' + 52) as u32,
                b'+' => 62,
                b'/' => 63,
                _ => 0x100,
            }
        }
    };
    let mut out: Vec<u8> = Vec::new();
    let mut quad: [u32; 4] = [0; 4];
    let mut n = 0usize;
    let invalid =
        || -> Flow { i.signal_data(sym::ERROR, vec![Value::string("Invalid base64 data")]) };
    let mut pad_await = false; // '=' seen at quad pos 2: await closing '='
    for c in s.to_external_bytes() {
        // Whitespace is always ignored.
        if matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c) {
            continue;
        }
        if c == b'=' {
            // GNU pad rules: '=' is legal only inside an open quad.
            // At position 3 it emits 2 bytes and closes the quad; at
            // position 2 it must be followed by one more '=' (the
            // canonical "…==" tail) which then emits 1 byte.  A bare
            // '=' with no pending chars — or data arriving while a pad
            // is awaited — is invalid.
            if pad_await {
                let w = (quad[0] << 18) | (quad[1] << 12);
                out.push((w >> 16) as u8);
                pad_await = false;
                n = 0;
            } else if n == 3 {
                let w = (quad[0] << 18) | (quad[1] << 12) | (quad[2] << 6);
                out.push((w >> 16) as u8);
                out.push((w >> 8) as u8);
                n = 0;
            } else if n == 2 {
                pad_await = true;
            } else {
                return Err(invalid());
            }
            continue;
        }
        if pad_await {
            // Inside "XX==" only the closing '=' is legal.
            return Err(invalid());
        }
        let v = b64val(c);
        if v == 0x100 {
            if ignore_invalid {
                continue;
            }
            return Err(invalid());
        }
        quad[n] = v;
        n += 1;
        if n == 4 {
            let w = (quad[0] << 18) | (quad[1] << 12) | (quad[2] << 6) | quad[3];
            out.push((w >> 16) as u8);
            out.push((w >> 8) as u8);
            out.push(w as u8);
            n = 0;
        }
    }
    // EOF: an awaited-but-unclosed "XX=" pad is invalid, as is a
    // leftover single char; a 2/3-char tail is invalid in strict mode
    // but GNU emits its partial bytes under IGNORE-INVALID.
    if pad_await || n == 1 || (n > 0 && !ignore_invalid) {
        return Err(invalid());
    }
    match n {
        2 => {
            let w = (quad[0] << 18) | (quad[1] << 12);
            out.push((w >> 16) as u8);
        }
        3 => {
            let w = (quad[0] << 18) | (quad[1] << 12) | (quad[2] << 6);
            out.push((w >> 16) as u8);
            out.push((w >> 8) as u8);
        }
        _ => {}
    }
    let v = Value::string(out.iter().map(|&b| b as char).collect::<String>());
    if let Value::Str(r) = &v {
        i.mark_unibyte(r);
    }
    Ok(v)
}

pub(super) fn f_buffer_hash(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU: (buffer-hash BUFFER) = sha1 hex of the buffer's text.
    let b = match args.get(0) {
        // GNU decodes via Fget_buffer: nil -> current buffer, a name
        // resolves or signals `No buffer named X'.  Non-string non-buffer
        // args signal wrong-type-argument STRINGP (Fget_buffer's check).
        None | Some(Value::Nil) => i
            .current_buffer_ref()
            .ok_or_else(|| i.error("No current buffer"))?,
        Some(Value::Buffer(b)) => b.clone(),
        Some(Value::Str(name)) => {
            let name = name.borrow().clone();
            match i.buffers.by_name(&name).and_then(|id| i.buffers.get(id)) {
                Some(b) => b,
                None => return Err(i.error(format!("No buffer named {}", name))),
            }
        }
        Some(other) => return Err(i.wrong_type_mut("stringp", other)),
    };
    let text = b.borrow().text.text();
    match hash_hex("sha1", &text.to_external_bytes()) {
        Ok(h) => Ok(Value::string(h)),
        Err(_) => unreachable!(),
    }
}

pub(super) fn f_secure_hash_algorithms(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let names = [
        "md5", "sha1", "sha224", "sha256", "sha384", "sha512", "sha3-224", "sha3-256", "sha3-384",
        "sha3-512",
    ];
    Ok(Value::list(
        names.iter().map(|n| Value::Sym(i.intern(n))).collect(),
    ))
}
