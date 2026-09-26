//! GnuTLS primitives, a port of GNU `src/gnutls.c'.
//!
//! `libgnutls' is loaded dynamically (see `dynlib'), so the crypto
//! primitives (`gnutls-ciphers', `gnutls-macs', `gnutls-digests',
//! `gnutls-hash-mac', `gnutls-hash-digest',
//! `gnutls-symmetric-encrypt'/`-decrypt', `gnutls-available-p') and the
//! error helpers work whenever a GnuTLS shared library is reachable.
//!
//! Session primitives (`gnutls-boot' and friends) integrate with
//! `lisp::process' network streams — see the `Session' plumbing below.

use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::OnceLock;

use super::{arg, want_int, S};
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::eval::{plist_get, Interp};
use crate::lisp::value::{SymId, Value};

// ---------------------------------------------------------------------------
// libgnutls FFI
// ---------------------------------------------------------------------------

type Datum = GnutlsDatum;
#[repr(C)]
#[derive(Clone, Copy)]
struct GnutlsDatum {
    data: *mut u8,
    size: u32,
}

type Hd = *mut core::ffi::c_void;

type CipherList = unsafe extern "C" fn() -> *const i32;
type CipherGetName = unsafe extern "C" fn(i32) -> *const c_char;
type CipherGetSize = unsafe extern "C" fn(i32) -> usize;
type CipherInit = unsafe extern "C" fn(*mut Hd, i32, *const Datum, *const Datum) -> i32;
type CipherSetIv = unsafe extern "C" fn(Hd, *const u8, usize);
type CipherCrypt2 = unsafe extern "C" fn(Hd, *const u8, usize, *mut u8, usize) -> i32;
type CipherDeinit = unsafe extern "C" fn(Hd);
type AeadInit = unsafe extern "C" fn(*mut Hd, i32, *const Datum) -> i32;
type AeadCrypt = unsafe extern "C" fn(
    Hd,
    *const u8,
    usize,
    *const u8,
    usize,
    usize,
    *const u8,
    usize,
    *mut u8,
    *mut usize,
) -> i32;
type AeadDeinit = unsafe extern "C" fn(Hd);
type MacList = unsafe extern "C" fn() -> *const i32;
type MacGetName = unsafe extern "C" fn(i32) -> *const c_char;
type HmacGetLen = unsafe extern "C" fn(i32) -> usize;
type MacGetKeySize = unsafe extern "C" fn(i32) -> usize;
type MacGetNonceSize = unsafe extern "C" fn(i32) -> usize;
type HmacInit = unsafe extern "C" fn(*mut Hd, i32, *const u8, usize) -> i32;
type HmacHash = unsafe extern "C" fn(Hd, *const u8, usize) -> i32;
type HmacOutput = unsafe extern "C" fn(Hd, *mut u8);
type HmacDeinit = unsafe extern "C" fn(Hd, *mut u8);
type DigestList = unsafe extern "C" fn() -> *const i32;
type DigestGetName = unsafe extern "C" fn(i32) -> *const c_char;
type HashGetLen = unsafe extern "C" fn(i32) -> usize;
type HashInit = unsafe extern "C" fn(*mut Hd, i32) -> i32;
type HashHash = unsafe extern "C" fn(Hd, *const u8, usize) -> i32;
type HashOutput = unsafe extern "C" fn(Hd, *mut u8);
type HashDeinit = unsafe extern "C" fn(Hd, *mut u8);
type StrError = unsafe extern "C" fn(i32) -> *const c_char;
type ErrorIsFatal = unsafe extern "C" fn(i32) -> i32;
type ExtGetName = unsafe extern "C" fn(u32) -> *const c_char;

// TLS session API (`gnutls-boot' and friends).
type GlobalInit = unsafe extern "C" fn() -> i32;
type SessionInit = unsafe extern "C" fn(*mut Hd, u32) -> i32;
type SessionDeinit = unsafe extern "C" fn(Hd);
type PrioritySetDirect = unsafe extern "C" fn(Hd, *const c_char, *mut *const c_char) -> i32;
type CertAllocCred = unsafe extern "C" fn(*mut Hd) -> i32;
type CertFreeCred = unsafe extern "C" fn(Hd);
type CertSetX509TrustFile = unsafe extern "C" fn(Hd, *const c_char, u32) -> i32;
type CertSetX509CrlFile = unsafe extern "C" fn(Hd, *const c_char, u32) -> i32;
type CertSetX509KeyFile = unsafe extern "C" fn(Hd, *const c_char, *const c_char, u32) -> i32;
type CertSetVerifyFlags = unsafe extern "C" fn(Hd, u32);
type CertSetSystemTrust = unsafe extern "C" fn(Hd) -> i32;
type AnonAllocCred = unsafe extern "C" fn(*mut Hd) -> i32;
type AnonFreeCred = unsafe extern "C" fn(Hd);
type CredentialsSet = unsafe extern "C" fn(Hd, u32, *mut core::ffi::c_void) -> i32;
type ServerNameSet = unsafe extern "C" fn(Hd, u32, *const core::ffi::c_void, usize) -> i32;
type DhSetPrimeBits = unsafe extern "C" fn(Hd, u32);
type DhGetPrimeBits = unsafe extern "C" fn(Hd) -> i32;
type TransportSetPtr2 = unsafe extern "C" fn(Hd, isize, isize);
type Handshake = unsafe extern "C" fn(Hd) -> i32;
type Bye = unsafe extern "C" fn(Hd, u32) -> i32;
type RecordSend = unsafe extern "C" fn(Hd, *const u8, usize) -> isize;
type RecordRecv = unsafe extern "C" fn(Hd, *mut u8, usize) -> isize;
type CertificateVerifyPeers2 = unsafe extern "C" fn(Hd, *mut u32) -> i32;
type CertificateGetPeers = unsafe extern "C" fn(Hd, *mut u32) -> *const Datum;
type CertificateTypeGet = unsafe extern "C" fn(Hd) -> i32;
type X509CrtInit = unsafe extern "C" fn(*mut Hd) -> i32;
type X509CrtImport = unsafe extern "C" fn(Hd, *const Datum, u32) -> i32;
type X509CrtPrint = unsafe extern "C" fn(Hd, u32, *mut Datum) -> i32;
type X509CrtDeinit = unsafe extern "C" fn(Hd);
type X509CrtGetDn = unsafe extern "C" fn(Hd, *mut u8, *mut usize) -> i32;
type X509CrtGetIssuerDn = unsafe extern "C" fn(Hd, *mut u8, *mut usize) -> i32;
type X509CrtCheckHostname = unsafe extern "C" fn(Hd, *const c_char) -> i32;
type X509CrtCheckIssuer = unsafe extern "C" fn(Hd, Hd) -> i32;
type X509CrtGetVersion = unsafe extern "C" fn(Hd) -> i32;
type X509CrtGetSerial = unsafe extern "C" fn(Hd, *mut u8, *mut usize) -> i32;
type X509CrtGetTime = unsafe extern "C" fn(Hd) -> i64;
type X509CrtGetPkAlgo = unsafe extern "C" fn(Hd, *mut u32) -> i32;
type X509CrtGetUniqId = unsafe extern "C" fn(Hd, *mut u8, *mut usize) -> i32;
type X509CrtGetSignAlgo = unsafe extern "C" fn(Hd) -> i32;
type X509CrtGetKeyId = unsafe extern "C" fn(Hd, u32, *mut u8, *mut usize) -> i32;
type X509CrtGetFingerprint = unsafe extern "C" fn(Hd, i32, *mut u8, *mut usize) -> i32;
type X509CrtExport = unsafe extern "C" fn(Hd, u32, *mut u8, *mut usize) -> i32;
type PkAlgoGetName = unsafe extern "C" fn(i32) -> *const c_char;
type SignGetName = unsafe extern "C" fn(i32) -> *const c_char;
type PkBitsToSecParam = unsafe extern "C" fn(i32, u32) -> i32;
type SecParamGetName = unsafe extern "C" fn(i32) -> *const c_char;
type ProtocolGetVersion = unsafe extern "C" fn(Hd) -> i32;
type ProtocolGetName = unsafe extern "C" fn(i32) -> *const c_char;
type CipherGet = unsafe extern "C" fn(Hd) -> i32;
type KxGet = unsafe extern "C" fn(Hd) -> i32;
type KxGetName = unsafe extern "C" fn(i32) -> *const c_char;
type MacGet = unsafe extern "C" fn(Hd) -> i32;
type CompressionGet = unsafe extern "C" fn(Hd) -> i32;
type CompressionGetName = unsafe extern "C" fn(i32) -> *const c_char;
type SessionEtmStatus = unsafe extern "C" fn(Hd) -> u32;
type SafeRenegotiationStatus = unsafe extern "C" fn(Hd) -> u32;
type Free = unsafe extern "C" fn(*mut core::ffi::c_void);
pub(crate) struct GnutlsFns {
    _lib: libloading::Library,
    cipher_list: CipherList,
    cipher_get_name: CipherGetName,
    cipher_get_tag_size: CipherGetSize,
    cipher_get_block_size: CipherGetSize,
    cipher_get_key_size: CipherGetSize,
    cipher_get_iv_size: CipherGetSize,
    cipher_init: CipherInit,
    cipher_set_iv: CipherSetIv,
    cipher_encrypt2: CipherCrypt2,
    cipher_decrypt2: CipherCrypt2,
    cipher_deinit: CipherDeinit,
    aead_cipher_init: Option<AeadInit>,
    aead_cipher_encrypt: Option<AeadCrypt>,
    aead_cipher_decrypt: Option<AeadCrypt>,
    aead_cipher_deinit: Option<AeadDeinit>,
    mac_list: MacList,
    mac_get_name: MacGetName,
    hmac_get_len: HmacGetLen,
    mac_get_key_size: MacGetKeySize,
    mac_get_nonce_size: Option<MacGetNonceSize>,
    hmac_init: HmacInit,
    hmac: HmacHash,
    hmac_output: HmacOutput,
    hmac_deinit: HmacDeinit,
    digest_list: DigestList,
    digest_get_name: DigestGetName,
    hash_get_len: HashGetLen,
    hash_init: HashInit,
    hash: HashHash,
    hash_output: HashOutput,
    hash_deinit: HashDeinit,
    strerror: StrError,
    error_is_fatal: ErrorIsFatal,
    ext_get_name: Option<ExtGetName>,
    // session API
    global_init: Option<GlobalInit>,
    session_init: Option<SessionInit>,
    session_deinit: Option<SessionDeinit>,
    priority_set_direct: Option<PrioritySetDirect>,
    cert_alloc_cred: Option<CertAllocCred>,
    cert_free_cred: Option<CertFreeCred>,
    cert_set_x509_trust_file: Option<CertSetX509TrustFile>,
    cert_set_x509_crl_file: Option<CertSetX509CrlFile>,
    cert_set_x509_key_file: Option<CertSetX509KeyFile>,
    cert_set_verify_flags: Option<CertSetVerifyFlags>,
    cert_set_system_trust: Option<CertSetSystemTrust>,
    anon_alloc_cred: Option<AnonAllocCred>,
    anon_free_cred: Option<AnonFreeCred>,
    credentials_set: Option<CredentialsSet>,
    server_name_set: Option<ServerNameSet>,
    dh_set_prime_bits: Option<DhSetPrimeBits>,
    dh_get_prime_bits: Option<DhGetPrimeBits>,
    transport_set_ptr2: Option<TransportSetPtr2>,
    handshake: Option<Handshake>,
    bye: Option<Bye>,
    record_send: Option<RecordSend>,
    record_recv: Option<RecordRecv>,
    certificate_verify_peers2: Option<CertificateVerifyPeers2>,
    certificate_get_peers: Option<CertificateGetPeers>,
    certificate_type_get: Option<CertificateTypeGet>,
    x509_crt_init: Option<X509CrtInit>,
    x509_crt_import: Option<X509CrtImport>,
    x509_crt_print: Option<X509CrtPrint>,
    x509_crt_deinit: Option<X509CrtDeinit>,
    x509_crt_get_dn: Option<X509CrtGetDn>,
    x509_crt_get_issuer_dn: Option<X509CrtGetIssuerDn>,
    x509_crt_check_hostname: Option<X509CrtCheckHostname>,
    x509_crt_check_issuer: Option<X509CrtCheckIssuer>,
    x509_crt_get_version: Option<X509CrtGetVersion>,
    x509_crt_get_serial: Option<X509CrtGetSerial>,
    x509_crt_get_activation_time: Option<X509CrtGetTime>,
    x509_crt_get_expiration_time: Option<X509CrtGetTime>,
    x509_crt_get_pk_algorithm: Option<X509CrtGetPkAlgo>,
    x509_crt_get_issuer_unique_id: Option<X509CrtGetUniqId>,
    x509_crt_get_subject_unique_id: Option<X509CrtGetUniqId>,
    x509_crt_get_signature_algorithm: Option<X509CrtGetSignAlgo>,
    x509_crt_get_key_id: Option<X509CrtGetKeyId>,
    x509_crt_get_fingerprint: Option<X509CrtGetFingerprint>,
    x509_crt_export: Option<X509CrtExport>,
    pk_algorithm_get_name: Option<PkAlgoGetName>,
    sign_get_name: Option<SignGetName>,
    pk_bits_to_sec_param: Option<PkBitsToSecParam>,
    sec_param_get_name: Option<SecParamGetName>,
    protocol_get_version: Option<ProtocolGetVersion>,
    protocol_get_name: Option<ProtocolGetName>,
    cipher_get: Option<CipherGet>,
    kx_get: Option<KxGet>,
    kx_get_name: Option<KxGetName>,
    mac_get: Option<MacGet>,
    compression_get: Option<CompressionGet>,
    compression_get_name: Option<CompressionGetName>,
    session_etm_status: Option<SessionEtmStatus>,
    safe_renegotiation_status: Option<SafeRenegotiationStatus>,
    free: Option<Free>,
}

unsafe impl Send for GnutlsFns {}
unsafe impl Sync for GnutlsFns {}

static GNUTLS: OnceLock<Option<GnutlsFns>> = OnceLock::new();

pub(crate) fn gnutls() -> Option<&'static GnutlsFns> {
    GNUTLS
        .get_or_init(|| {
            let lib = crate::lisp::dynlib::open_library(
                "REMACS_GNUTLS_LIBRARY",
                &[
                    "libgnutls.30.dylib",
                    "libgnutls.so.30",
                    "libgnutls.dylib",
                    "libgnutls.so",
                ],
                "gnutls",
                "libgnutls.dylib",
            )?;
            use crate::lisp::dynlib::sym;
            let f = unsafe {
                GnutlsFns {
                    cipher_list: sym(&lib, b"gnutls_cipher_list\0")?,
                    cipher_get_name: sym(&lib, b"gnutls_cipher_get_name\0")?,
                    cipher_get_tag_size: sym(&lib, b"gnutls_cipher_get_tag_size\0")?,
                    cipher_get_block_size: sym(&lib, b"gnutls_cipher_get_block_size\0")?,
                    cipher_get_key_size: sym(&lib, b"gnutls_cipher_get_key_size\0")?,
                    cipher_get_iv_size: sym(&lib, b"gnutls_cipher_get_iv_size\0")?,
                    cipher_init: sym(&lib, b"gnutls_cipher_init\0")?,
                    cipher_set_iv: sym(&lib, b"gnutls_cipher_set_iv\0")?,
                    cipher_encrypt2: sym(&lib, b"gnutls_cipher_encrypt2\0")?,
                    cipher_decrypt2: sym(&lib, b"gnutls_cipher_decrypt2\0")?,
                    cipher_deinit: sym(&lib, b"gnutls_cipher_deinit\0")?,
                    aead_cipher_init: sym(&lib, b"gnutls_aead_cipher_init\0"),
                    aead_cipher_encrypt: sym(&lib, b"gnutls_aead_cipher_encrypt\0"),
                    aead_cipher_decrypt: sym(&lib, b"gnutls_aead_cipher_decrypt\0"),
                    aead_cipher_deinit: sym(&lib, b"gnutls_aead_cipher_deinit\0"),
                    mac_list: sym(&lib, b"gnutls_mac_list\0")?,
                    mac_get_name: sym(&lib, b"gnutls_mac_get_name\0")?,
                    hmac_get_len: sym(&lib, b"gnutls_hmac_get_len\0")?,
                    mac_get_key_size: sym(&lib, b"gnutls_mac_get_key_size\0")?,
                    mac_get_nonce_size: sym(&lib, b"gnutls_mac_get_nonce_size\0"),
                    hmac_init: sym(&lib, b"gnutls_hmac_init\0")?,
                    hmac: sym(&lib, b"gnutls_hmac\0")?,
                    hmac_output: sym(&lib, b"gnutls_hmac_output\0")?,
                    hmac_deinit: sym(&lib, b"gnutls_hmac_deinit\0")?,
                    digest_list: sym(&lib, b"gnutls_digest_list\0")?,
                    digest_get_name: sym(&lib, b"gnutls_digest_get_name\0")?,
                    hash_get_len: sym(&lib, b"gnutls_hash_get_len\0")?,
                    hash_init: sym(&lib, b"gnutls_hash_init\0")?,
                    hash: sym(&lib, b"gnutls_hash\0")?,
                    hash_output: sym(&lib, b"gnutls_hash_output\0")?,
                    hash_deinit: sym(&lib, b"gnutls_hash_deinit\0")?,
                    strerror: sym(&lib, b"gnutls_strerror\0")?,
                    error_is_fatal: sym(&lib, b"gnutls_error_is_fatal\0")?,
                    ext_get_name: sym(&lib, b"gnutls_ext_get_name\0"),
                    global_init: sym(&lib, b"gnutls_global_init\0"),
                    session_init: sym(&lib, b"gnutls_init\0"),
                    session_deinit: sym(&lib, b"gnutls_deinit\0"),
                    priority_set_direct: sym(&lib, b"gnutls_priority_set_direct\0"),
                    cert_alloc_cred: sym(&lib, b"gnutls_certificate_allocate_credentials\0"),
                    cert_free_cred: sym(&lib, b"gnutls_certificate_free_credentials\0"),
                    cert_set_x509_trust_file: sym(
                        &lib,
                        b"gnutls_certificate_set_x509_trust_file\0",
                    ),
                    cert_set_x509_crl_file: sym(
                        &lib,
                        b"gnutls_certificate_set_x509_crl_file\0",
                    ),
                    cert_set_x509_key_file: sym(
                        &lib,
                        b"gnutls_certificate_set_x509_key_file\0",
                    ),
                    cert_set_verify_flags: sym(
                        &lib,
                        b"gnutls_certificate_set_verify_flags\0",
                    ),
                    cert_set_system_trust: sym(
                        &lib,
                        b"gnutls_certificate_set_x509_system_trust\0",
                    ),
                    anon_alloc_cred: sym(
                        &lib,
                        b"gnutls_anon_allocate_client_credentials\0",
                    ),
                    anon_free_cred: sym(
                        &lib,
                        b"gnutls_anon_free_client_credentials\0",
                    ),
                    credentials_set: sym(&lib, b"gnutls_credentials_set\0"),
                    server_name_set: sym(&lib, b"gnutls_server_name_set\0"),
                    dh_set_prime_bits: sym(&lib, b"gnutls_dh_set_prime_bits\0"),
                    dh_get_prime_bits: sym(&lib, b"gnutls_dh_get_prime_bits\0"),
                    transport_set_ptr2: sym(&lib, b"gnutls_transport_set_ptr2\0"),
                    handshake: sym(&lib, b"gnutls_handshake\0"),
                    bye: sym(&lib, b"gnutls_bye\0"),
                    record_send: sym(&lib, b"gnutls_record_send\0"),
                    record_recv: sym(&lib, b"gnutls_record_recv\0"),

                    certificate_verify_peers2: sym(
                        &lib,
                        b"gnutls_certificate_verify_peers2\0",
                    ),
                    certificate_get_peers: sym(&lib, b"gnutls_certificate_get_peers\0"),
                    certificate_type_get: sym(&lib, b"gnutls_certificate_type_get\0"),
                    x509_crt_init: sym(&lib, b"gnutls_x509_crt_init\0"),
                    x509_crt_import: sym(&lib, b"gnutls_x509_crt_import\0"),
                    x509_crt_print: sym(&lib, b"gnutls_x509_crt_print\0"),
                    x509_crt_deinit: sym(&lib, b"gnutls_x509_crt_deinit\0"),
                    x509_crt_get_dn: sym(&lib, b"gnutls_x509_crt_get_dn\0"),
                    x509_crt_get_issuer_dn: sym(&lib, b"gnutls_x509_crt_get_issuer_dn\0"),
                    x509_crt_check_hostname: sym(
                        &lib,
                        b"gnutls_x509_crt_check_hostname\0",
                    ),
                    x509_crt_check_issuer: sym(&lib, b"gnutls_x509_crt_check_issuer\0"),
                    x509_crt_get_version: sym(&lib, b"gnutls_x509_crt_get_version\0"),
                    x509_crt_get_serial: sym(&lib, b"gnutls_x509_crt_get_serial\0"),
                    x509_crt_get_activation_time: sym(
                        &lib,
                        b"gnutls_x509_crt_get_activation_time\0",
                    ),
                    x509_crt_get_expiration_time: sym(
                        &lib,
                        b"gnutls_x509_crt_get_expiration_time\0",
                    ),
                    x509_crt_get_pk_algorithm: sym(
                        &lib,
                        b"gnutls_x509_crt_get_pk_algorithm\0",
                    ),
                    x509_crt_get_issuer_unique_id: sym(
                        &lib,
                        b"gnutls_x509_crt_get_issuer_unique_id\0",
                    ),
                    x509_crt_get_subject_unique_id: sym(
                        &lib,
                        b"gnutls_x509_crt_get_subject_unique_id\0",
                    ),
                    x509_crt_get_signature_algorithm: sym(
                        &lib,
                        b"gnutls_x509_crt_get_signature_algorithm\0",
                    ),
                    x509_crt_get_key_id: sym(&lib, b"gnutls_x509_crt_get_key_id\0"),
                    x509_crt_get_fingerprint: sym(
                        &lib,
                        b"gnutls_x509_crt_get_fingerprint\0",
                    ),
                    x509_crt_export: sym(&lib, b"gnutls_x509_crt_export\0"),
                    pk_algorithm_get_name: sym(&lib, b"gnutls_pk_algorithm_get_name\0"),
                    sign_get_name: sym(&lib, b"gnutls_sign_get_name\0"),
                    pk_bits_to_sec_param: sym(&lib, b"gnutls_pk_bits_to_sec_param\0"),
                    sec_param_get_name: sym(&lib, b"gnutls_sec_param_get_name\0"),
                    protocol_get_version: sym(&lib, b"gnutls_protocol_get_version\0"),
                    protocol_get_name: sym(&lib, b"gnutls_protocol_get_name\0"),
                    cipher_get: sym(&lib, b"gnutls_cipher_get\0"),
                    kx_get: sym(&lib, b"gnutls_kx_get\0"),
                    kx_get_name: sym(&lib, b"gnutls_kx_get_name\0"),
                    mac_get: sym(&lib, b"gnutls_mac_get\0"),
                    compression_get: sym(&lib, b"gnutls_compression_get\0"),
                    compression_get_name: sym(&lib, b"gnutls_compression_get_name\0"),
                    session_etm_status: sym(&lib, b"gnutls_session_etm_status\0"),
                    safe_renegotiation_status: sym(
                        &lib,
                        b"gnutls_safe_renegotiation_status\0",
                    ),
                    // `gnutls_free' is a function-pointer *variable*
                    // (gnutls_free_function), not a function symbol —
                    // read the pointer stored at its address.
                    free: {
                        let var: Option<*mut core::ffi::c_void> =
                            sym(&lib, b"gnutls_free\0");
                        var.and_then(|p| {
                            let fp = *(p as *const usize);
                            if fp == 0 {
                                None
                            } else {
                                Some(std::mem::transmute::<usize, Free>(fp))
                            }
                        })
                    },
                    _lib: lib,
                }
            };
            Some(f)
        })
        .as_ref()
}

// GnuTLS constants (from <gnutls/gnutls.h> enums — stable ABI values).
const GNUTLS_E_SUCCESS: i32 = 0;
const GNUTLS_E_AGAIN: i32 = -28;
const GNUTLS_E_INTERRUPTED: i32 = -52;
const GNUTLS_E_INVALID_SESSION: i32 = -75;
const GNUTLS_E_APPLICATION_ERROR_MIN: i32 = -1200;

fn cstr(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

fn strerror(g: &GnutlsFns, err: i32) -> String {
    cstr(unsafe { (g.strerror)(err) })
}

// ---------------------------------------------------------------------------
// Lisp helpers
// ---------------------------------------------------------------------------

fn intern(i: &mut Interp, s: &str) -> SymId {
    i.intern(s)
}

fn unibyte(i: &mut Interp, bytes: &[u8]) -> Value {
    let s: String = bytes.iter().map(|&b| b as char).collect();
    let v = Value::string(s);
    if let Value::Str(r) = &v {
        i.mark_unibyte(r);
    }
    v
}

fn gnutls_make_error(i: &mut Interp, err: i32) -> Value {
    match err {
        GNUTLS_E_SUCCESS => Value::t(),
        GNUTLS_E_AGAIN => Value::Sym(intern(i, "gnutls-e-again")),
        GNUTLS_E_INTERRUPTED => Value::Sym(intern(i, "gnutls-e-interrupted")),
        GNUTLS_E_INVALID_SESSION => Value::Sym(intern(i, "gnutls-e-invalid-session")),
        _ => Value::Int(err as i128),
    }
}

/// GNU `extract_data_from_object', restricted to the forms the crypto
/// primitives actually see: `(OBJECT [START] [END] [CODING] [NOERROR])'
/// where OBJECT is a string or buffer.  Returns the byte payload.
fn extract_data(i: &mut Interp, spec: &Value) -> Result<Vec<u8>, Flow> {
    let mut parts: Vec<Value> = Vec::new();
    let mut cur = spec.clone();
    loop {
        match cur {
            Value::Cons(c) => {
                let (car, cdr) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                parts.push(car);
                cur = cdr;
            }
            _ => break,
        }
    }
    let obj = match parts.first() {
        Some(o) => o.clone(),
        None => return Err(i.wrong_type_mut("consp", spec)),
    };
    // Whole-object bytes: strings via the unibyte/UTF-8 rule shared
    // with `secure-hash'; buffers via their raw text bytes.
    let (bytes, nchars): (Vec<u8>, usize) = match &obj {
        Value::Str(_) => {
            let b = super::misc::lisp_string_bytes(i, &obj);
            let n = match &obj {
                Value::Str(r) => r.borrow().chars().count(),
                _ => 0,
            };
            (b, n)
        }
        Value::Buffer(b) => {
            let bb = b.borrow();
            let text = bb.text.text();
            let n = text.chars().count();
            (text.into_bytes(), n)
        }
        _ => {
            return Err(i.signal_data(
                crate::lisp::obarray::sym::ERROR,
                vec![
                    Value::string("Invalid object argument"),
                    obj.clone(),
                ],
            ))
        }
    };
    // START/END are character positions; map to byte offsets by
    // re-encoding the char slice (exact for unibyte/UTF-8 sources).
    let start = match parts.get(1) {
        None | Some(Value::Nil) => 0usize,
        Some(v) => (want_int(i, v)?).max(0) as usize,
    };
    let end = match parts.get(2) {
        None | Some(Value::Nil) => nchars,
        Some(v) => (want_int(i, v)?).max(0) as usize,
    };
    if start > end || end > nchars {
        return Err(i.signal_data(
            crate::lisp::obarray::sym::ARGS_OUT_OF_RANGE,
            vec![
                obj.clone(),
                Value::Int(start as i128),
                Value::Int(end as i128),
            ],
        ));
    }
    if start == 0 && end == nchars {
        return Ok(bytes);
    }
    // Byte offsets of char boundaries.
    let chars: Vec<char> = match &obj {
        Value::Str(r) => r.borrow().chars().collect(),
        Value::Buffer(b) => b.borrow().text.text().chars().collect(),
        _ => Vec::new(),
    };
    let unibyte_src = match &obj {
        Value::Str(r) => i.is_unibyte_str(r),
        _ => false,
    };
    let mut prefix = 0usize;
    let mut bstart = bytes.len();
    let mut bend = bytes.len();
    for (idx, c) in chars.iter().enumerate() {
        let w = if unibyte_src || crate::lisp::value::eight_bit_byte(*c).is_some() {
            1
        } else {
            c.len_utf8()
        };
        if idx == start {
            bstart = prefix;
        }
        if idx == end {
            bend = prefix;
            break;
        }
        prefix += w;
    }
    if start == nchars {
        bstart = bytes.len();
    }
    if end == nchars {
        bend = bytes.len();
    }
    Ok(bytes[bstart.min(bytes.len())..bend.min(bytes.len())].to_vec())
}

/// Wrap OBJECT in a one-element list like GNU's `list1' at the top of
/// each crypto DEFUN; then require a cons spec.
fn specify(obj: &Value) -> Value {
    match obj {
        Value::Str(_) | Value::Buffer(_) => Value::list(vec![obj.clone()]),
        _ => obj.clone(),
    }
}

fn wipe_key(key: &Value) {
    // GNU wipes the key string after use (`Fclear_string').  Keep the
    // character count: the unibyte string may hold 2-byte UTF-8 chars,
    // so zeroing the raw buffer would change its length.
    if let Value::Cons(c) = key {
        if let Value::Str(s) = &c.borrow().car {
            let mut b = s.borrow_mut();
            let n = b.chars().count();
            *b = "\0".repeat(n);
        }
    }
}

// ---------------------------------------------------------------------------
// Algorithm alists
// ---------------------------------------------------------------------------

fn f_ciphers(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let Some(g) = gnutls() else {
        return Ok(Value::Nil);
    };
    let list = unsafe { (g.cipher_list)() };
    let mut out = Value::Nil;
    if list.is_null() {
        return Ok(out);
    }
    // Build in the same Fcons order as GNU: prepend each entry.
    unsafe {
        let mut pos = 0;
        while *list.add(pos) != 0 {
            let gca = *list.add(pos);
            pos += 1;
            if gca == 1 {
                continue; // GNUTLS_CIPHER_NULL
            }
            let name = cstr((g.cipher_get_name)(gca));
            if name.is_empty() {
                continue;
            }
            let tag_size = (g.cipher_get_tag_size)(gca);
            let entry = Value::list(vec![
                Value::Sym(intern(i, &name)),
                Value::Sym(intern(i, ":cipher-id")),
                Value::Int(gca as i128),
                Value::Sym(intern(i, ":type")),
                Value::Sym(intern(i, "gnutls-symmetric-cipher")),
                Value::Sym(intern(i, ":cipher-aead-capable")),
                if tag_size == 0 { Value::Nil } else { Value::t() },
                Value::Sym(intern(i, ":cipher-tagsize")),
                Value::Int(tag_size as i128),
                Value::Sym(intern(i, ":cipher-blocksize")),
                Value::Int((g.cipher_get_block_size)(gca) as i128),
                Value::Sym(intern(i, ":cipher-keysize")),
                Value::Int((g.cipher_get_key_size)(gca) as i128),
                Value::Sym(intern(i, ":cipher-ivsize")),
                Value::Int((g.cipher_get_iv_size)(gca) as i128),
            ]);
            out = Value::cons(entry, out);
        }
    }
    Ok(out)
}

fn f_macs(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let Some(g) = gnutls() else {
        return Ok(Value::Nil);
    };
    let list = unsafe { (g.mac_list)() };
    let mut out = Value::Nil;
    if list.is_null() {
        return Ok(out);
    }
    unsafe {
        let mut pos = 0;
        while *list.add(pos) != 0 {
            let gma = *list.add(pos);
            pos += 1;
            let name = cstr((g.mac_get_name)(gma));
            let nonce = g
                .mac_get_nonce_size
                .map(|f| f(gma))
                .unwrap_or(0);
            let entry = Value::list(vec![
                Value::Sym(intern(i, &name)),
                Value::Sym(intern(i, ":mac-algorithm-id")),
                Value::Int(gma as i128),
                Value::Sym(intern(i, ":type")),
                Value::Sym(intern(i, "gnutls-mac-algorithm")),
                Value::Sym(intern(i, ":mac-algorithm-length")),
                Value::Int((g.hmac_get_len)(gma) as i128),
                Value::Sym(intern(i, ":mac-algorithm-keysize")),
                Value::Int((g.mac_get_key_size)(gma) as i128),
                Value::Sym(intern(i, ":mac-algorithm-noncesize")),
                Value::Int(nonce as i128),
            ]);
            out = Value::cons(entry, out);
        }
    }
    Ok(out)
}

fn f_digests(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let Some(g) = gnutls() else {
        return Ok(Value::Nil);
    };
    let list = unsafe { (g.digest_list)() };
    let mut out = Value::Nil;
    if list.is_null() {
        return Ok(out);
    }
    unsafe {
        let mut pos = 0;
        while *list.add(pos) != 0 {
            let gda = *list.add(pos);
            pos += 1;
            let name = cstr((g.digest_get_name)(gda));
            let entry = Value::list(vec![
                Value::Sym(intern(i, &name)),
                Value::Sym(intern(i, ":digest-algorithm-id")),
                Value::Int(gda as i128),
                Value::Sym(intern(i, ":type")),
                Value::Sym(intern(i, "gnutls-digest-algorithm")),
                Value::Sym(intern(i, ":digest-algorithm-length")),
                Value::Int((g.hash_get_len)(gda) as i128),
            ]);
            out = Value::cons(entry, out);
        }
    }
    Ok(out)
}

/// Resolve METHOD (string|symbol|plist|integer) to an algorithm id via
/// ALIST-FN and PROP.  Mirrors GNU's three-way dispatch.
fn algo_id(
    i: &mut Interp,
    method: &Value,
    alist: &Value,
    prop: &str,
    err_msg: &str,
) -> Result<i32, Flow> {
    let mut info = Value::Nil;
    let mut id: i64 = -1;
    let mut m = method.clone();
    if let Value::Str(s) = &m {
        let name = s.borrow().clone();
        m = Value::Sym(intern(i, &name));
    }
    match &m {
        Value::Sym(s) => {
            // assq over the alist entries.
            let mut cur = alist.clone();
            let mut found = Value::Nil;
            while let Value::Cons(c) = cur {
                let (car, cdr) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if let Value::Cons(e) = &car {
                    if matches!(&e.borrow().car, Value::Sym(es) if *es == *s) {
                        found = car.clone();
                        break;
                    }
                }
                cur = cdr;
            }
            if !matches!(found, Value::Cons(_)) {
                return Err(i.error_obj(err_msg, &m));
            }
            if let Value::Cons(e) = &found {
                info = e.borrow().cdr.clone();
            }
        }
        Value::Int(n) => id = *n as i64,
        _ => info = m.clone(),
    }
    if let Value::Cons(_) = &info {
        let v = plist_get(&info, intern(i, prop));
        if let Value::Int(n) = v {
            id = n as i64;
        }
    }
    Ok(id as i32)
}

// ---------------------------------------------------------------------------
// Crypto DEFUNs
// ---------------------------------------------------------------------------

fn f_hash_digest(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let Some(g) = gnutls() else {
        return Ok(Value::Nil);
    };
    let input = specify(&a[1]);
    if !matches!(input, Value::Cons(_)) {
        return Err(i.wrong_type_mut("consp", &input));
    }
    let digests = f_digests(i, vec![])?;
    let gda = algo_id(
        i,
        &a[0],
        &digests,
        ":digest-algorithm-id",
        "GnuTLS digest-method is invalid or not found",
    )?;
    let digest_length = unsafe { (g.hash_get_len)(gda) };
    if digest_length == 0 {
        return Err(i.error_obj(
            "GnuTLS digest-method is invalid or not found",
            &a[0],
        ));
    }
    let mut hd: Hd = std::ptr::null_mut();
    let ret = unsafe { (g.hash_init)(&mut hd, gda) };
    if ret < GNUTLS_E_SUCCESS {
        return Err(i.error(&format!(
            "GnuTLS digest initialization failed: {}",
            strerror(g, ret)
        )));
    }
    let idata = extract_data(i, &input)?;
    let ret = unsafe { (g.hash)(hd, idata.as_ptr(), idata.len()) };
    if ret < GNUTLS_E_SUCCESS {
        unsafe { (g.hash_deinit)(hd, std::ptr::null_mut()) };
        return Err(i.error(&format!(
            "GnuTLS digest application failed: {}",
            strerror(g, ret)
        )));
    }
    let mut out = vec![0u8; digest_length];
    unsafe {
        (g.hash_output)(hd, out.as_mut_ptr());
        (g.hash_deinit)(hd, std::ptr::null_mut());
    }
    Ok(unibyte(i, &out))
}

fn f_hash_mac(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let Some(g) = gnutls() else {
        return Ok(Value::Nil);
    };
    let input = specify(&a[2]);
    if !matches!(input, Value::Cons(_)) {
        return Err(i.wrong_type_mut("consp", &input));
    }
    let key = specify(&a[1]);
    if !matches!(key, Value::Cons(_)) {
        return Err(i.wrong_type_mut("consp", &key));
    }
    let macs = f_macs(i, vec![])?;
    let gma = algo_id(
        i,
        &a[0],
        &macs,
        ":mac-algorithm-id",
        "GnuTLS MAC-method is invalid or not found",
    )?;
    let digest_length = unsafe { (g.hmac_get_len)(gma) };
    if digest_length == 0 {
        return Err(i.error_obj(
            "GnuTLS MAC-method is invalid or not found",
            &a[0],
        ));
    }
    let mut hd: Hd = std::ptr::null_mut();
    let kdata = extract_data(i, &key)?;
    let ret = unsafe { (g.hmac_init)(&mut hd, gma, kdata.as_ptr(), kdata.len()) };
    wipe_key(&key);
    if ret < GNUTLS_E_SUCCESS {
        return Err(i.error(&format!(
            "GnuTLS HMAC initialization failed: {}",
            strerror(g, ret)
        )));
    }
    let idata = extract_data(i, &input)?;
    let ret = unsafe { (g.hmac)(hd, idata.as_ptr(), idata.len()) };
    if ret < GNUTLS_E_SUCCESS {
        unsafe { (g.hmac_deinit)(hd, std::ptr::null_mut()) };
        return Err(i.error(&format!(
            "GnuTLS HMAC application failed: {}",
            strerror(g, ret)
        )));
    }
    let mut out = vec![0u8; digest_length];
    unsafe {
        (g.hmac_output)(hd, out.as_mut_ptr());
        (g.hmac_deinit)(hd, std::ptr::null_mut());
    }
    Ok(unibyte(i, &out))
}

fn gnutls_symmetric(i: &mut Interp, encrypting: bool, a: Vec<Value>) -> EvalResult {
    let Some(g) = gnutls() else {
        return Ok(Value::Nil);
    };
    let desc = if encrypting { "encrypt" } else { "decrypt" };
    let key = specify(&a[1]);
    if !matches!(key, Value::Cons(_)) {
        return Err(i.wrong_type_mut("consp", &key));
    }
    let input = specify(&a[3]);
    if !matches!(input, Value::Cons(_)) {
        return Err(i.wrong_type_mut("consp", &input));
    }
    let iv = specify(&a[2]);
    if !matches!(iv, Value::Cons(_)) {
        return Err(i.wrong_type_mut("consp", &iv));
    }

    let ciphers = f_ciphers(i, vec![])?;
    let gca = algo_id(
        i,
        &a[0],
        &ciphers,
        ":cipher-id",
        "GnuTLS cipher is invalid or not found",
    )?;

    let key_size = unsafe { (g.cipher_get_key_size)(gca) };
    if key_size == 0 {
        return Err(i.error_obj("GnuTLS cipher is invalid or not found", &a[0]));
    }
    let kdata = extract_data(i, &key)?;
    if kdata.len() != key_size {
        return Err(i.error(&format!(
            "GnuTLS cipher {} key length {} is not equal to the required {}",
            cstr(unsafe { (g.cipher_get_name)(gca) }),
            kdata.len(),
            key_size
        )));
    }
    let vdata = extract_data(i, &iv)?;
    let iv_size = unsafe { (g.cipher_get_iv_size)(gca) };
    if vdata.len() != iv_size {
        return Err(i.error(&format!(
            "GnuTLS cipher {} IV length {} is not equal to the required {}",
            cstr(unsafe { (g.cipher_get_name)(gca) }),
            vdata.len(),
            iv_size
        )));
    }
    let actual_iv = unibyte(i, &vdata);
    let idata = extract_data(i, &input)?;

    // AEAD?
    let tag_size = unsafe { (g.cipher_get_tag_size)(gca) };
    if tag_size > 0 {
        let (Some(ainit), Some(acrypt), Some(adeinit)) = (
            g.aead_cipher_init,
            if encrypting {
                g.aead_cipher_encrypt
            } else {
                g.aead_cipher_decrypt
            },
            g.aead_cipher_deinit,
        ) else {
            return Err(i.error(&format!(
                "GnuTLS AEAD cipher {} is invalid or not found",
                gca
            )));
        };
        let mut hd: Hd = std::ptr::null_mut();
        let kd = Datum {
            data: kdata.as_ptr() as *mut u8,
            size: kdata.len() as u32,
        };
        let ret = unsafe { ainit(&mut hd, gca, &kd) };
        if ret < GNUTLS_E_SUCCESS {
            return Err(i.error(&format!(
                "GnuTLS AEAD cipher {} initialization failed: {}",
                cstr(unsafe { (g.cipher_get_name)(gca) }),
                strerror(g, ret)
            )));
        }
        let auth = arg(&a, 4);
        let (adata, asize) = if auth.truthy() {
            let aspec = specify(&auth);
            if !matches!(aspec, Value::Cons(_)) {
                return Err(i.wrong_type_mut("consp", &aspec));
            }
            let d = extract_data(i, &aspec)?;
            let n = d.len();
            (d, n)
        } else {
            (Vec::new(), 0)
        };
        let mut out = vec![0u8; idata.len() + tag_size];
        let mut outlen = out.len();
        let ret = unsafe {
            acrypt(
                hd,
                vdata.as_ptr(),
                vdata.len(),
                if asize > 0 { adata.as_ptr() } else { std::ptr::null() },
                asize,
                tag_size,
                idata.as_ptr(),
                idata.len(),
                out.as_mut_ptr(),
                &mut outlen,
            )
        };
        wipe_key(&key);
        unsafe { adeinit(hd) };
        if ret < GNUTLS_E_SUCCESS {
            return Err(i.error(&format!(
                "GnuTLS AEAD cipher {} {}cryption failed: {}",
                cstr(unsafe { (g.cipher_get_name)(gca) }),
                if encrypting { "en" } else { "de" },
                strerror(g, ret)
            )));
        }
        out.truncate(outlen);
        let storage = unibyte(i, &out);
        return Ok(Value::list(vec![storage, actual_iv]));
    }

    let block = unsafe { (g.cipher_get_block_size)(gca) };
    if block > 0 && idata.len() % block != 0 {
        return Err(i.error(&format!(
            "GnuTLS cipher {} {} input block length {} is not a multiple of the required {}",
            cstr(unsafe { (g.cipher_get_name)(gca) }),
            desc,
            idata.len(),
            block
        )));
    }
    let mut hd: Hd = std::ptr::null_mut();
    let kd = Datum {
        data: kdata.as_ptr() as *mut u8,
        size: kdata.len() as u32,
    };
    let ret = unsafe { (g.cipher_init)(&mut hd, gca, &kd, std::ptr::null()) };
    if ret < GNUTLS_E_SUCCESS {
        return Err(i.error(&format!(
            "GnuTLS cipher {} {} initialization failed: {}",
            cstr(unsafe { (g.cipher_get_name)(gca) }),
            desc,
            strerror(g, ret)
        )));
    }
    unsafe { (g.cipher_set_iv)(hd, vdata.as_ptr(), vdata.len()) };
    let mut out = vec![0u8; idata.len()];
    let crypt = if encrypting {
        g.cipher_encrypt2
    } else {
        g.cipher_decrypt2
    };
    let ret = unsafe { crypt(hd, idata.as_ptr(), idata.len(), out.as_mut_ptr(), out.len()) };
    wipe_key(&key);
    unsafe { (g.cipher_deinit)(hd) };
    if ret < GNUTLS_E_SUCCESS {
        return Err(i.error(&format!(
            "GnuTLS cipher {} {}cryption failed: {}",
            cstr(unsafe { (g.cipher_get_name)(gca) }),
            if encrypting { "en" } else { "de" },
            strerror(g, ret)
        )));
    }
    Ok(Value::list(vec![unibyte(i, &out), actual_iv]))
}

fn f_symmetric_encrypt(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    gnutls_symmetric(i, true, a)
}

fn f_symmetric_decrypt(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    gnutls_symmetric(i, false, a)
}

// ---------------------------------------------------------------------------
// Error helpers
// ---------------------------------------------------------------------------

fn f_errorp(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU: everything except `t' and `gnutls-e-again' counts as an
    // error (including plain integers and unrelated objects).
    match &a[0] {
        Value::Sym(s) if *s == crate::lisp::obarray::sym::T => Ok(Value::Nil),
        Value::Sym(s) if *s == intern(i, "gnutls-e-again") => Ok(Value::Nil),
        _ => Ok(Value::t()),
    }
}

fn f_error_fatalp(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let err = match &a[0] {
        Value::Sym(s) if *s == crate::lisp::obarray::sym::T => return Ok(Value::Nil),
        Value::Sym(s) => {
            let prop = intern(i, "gnutls-code");
            let code = i.get_prop(*s, prop);
            match code {
                Value::Int(_) => code,
                _ => return Err(i.error("Symbol has no numeric gnutls-code property")),
            }
        }
        v => v.clone(),
    };
    let n = match err {
        Value::Int(n) => n as i32,
        _ => return Err(i.error("Not an error symbol or code")),
    };
    let fatal = gnutls()
        .map(|g| unsafe { (g.error_is_fatal)(n) != 0 })
        .unwrap_or(false);
    Ok(if fatal { Value::t() } else { Value::Nil })
}

fn f_error_string(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let err = match &a[0] {
        Value::Sym(s) if *s == crate::lisp::obarray::sym::T => {
            return Ok(Value::string("Not an error"))
        }
        Value::Sym(s) => {
            let prop = intern(i, "gnutls-code");
            let code = i.get_prop(*s, prop);
            match code {
                Value::Int(_) => code,
                _ => {
                    return Ok(Value::string(
                        "Symbol has no numeric gnutls-code property",
                    ))
                }
            }
        }
        v => v.clone(),
    };
    let n = match err {
        Value::Int(n) => n as i32,
        _ => return Ok(Value::string("Not an error symbol or code")),
    };
    match gnutls() {
        Some(g) => Ok(Value::string(strerror(g, n))),
        None => Ok(Value::string(format!("GnuTLS error {}", n))),
    }
}

// ---------------------------------------------------------------------------
// available-p
// ---------------------------------------------------------------------------

fn f_available_p(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let Some(g) = gnutls() else {
        return Ok(Value::Nil);
    };
    let mut caps = Value::Nil;
    let push = |caps: &mut Value, i: &mut Interp, name: &str| {
        *caps = Value::cons(Value::Sym(intern(i, name)), caps.clone());
    };
    push(&mut caps, i, "gnutls");
    push(&mut caps, i, "gnutls3");
    push(&mut caps, i, "digests");
    push(&mut caps, i, "ciphers");
    if g.aead_cipher_init.is_some() {
        push(&mut caps, i, "AEAD-ciphers");
    }
    push(&mut caps, i, "macs");
    if let Some(ext_get_name) = g.ext_get_name {
        for ext in 0u32..100 {
            let name = cstr(unsafe { ext_get_name(ext) });
            if name.is_empty() {
                continue;
            }
            let sym = intern(i, &name);
            // Fmemq check — GNU dedups.
            let mut dup = false;
            let mut cur = caps.clone();
            while let Value::Cons(c) = cur {
                let (car, cdr) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if matches!(car, Value::Sym(s) if s == sym) {
                    dup = true;
                    break;
                }
                cur = cdr;
            }
            if !dup {
                caps = Value::cons(Value::Sym(sym), caps);
            }
        }
    }
    Ok(caps)
}

// ---------------------------------------------------------------------------
// Session stubs (full TLS transport lands with process integration)
// ---------------------------------------------------------------------------

fn want_proc_v(i: &mut Interp, v: &Value) -> Result<(), Flow> {
    match v {
        Value::Process(_) => Ok(()),
        _ => Err(i.wrong_type_mut("processp", v)),
    }
}

fn f_get_initstage(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    want_proc_v(i, &a[0])?;
    // GNU reads p->gnutls_init_stage (0 unless a session was booted).
    if let Value::Process(p) = &a[0] {
        return Ok(Value::Int(p.borrow().gnutls_initstage as i128));
    }
    Ok(Value::Int(0))
}

fn f_async_params(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    want_proc_v(i, &a[0])?;
    if let Value::Process(p) = &a[0] {
        let mut pb = p.borrow_mut();
        pb.gnutls_async_connected = a[1].clone();
        pb.gnutls_async_signalled = a[2].clone();
    }
    Ok(Value::Nil)
}

/// GNU `emacs_gnutls_deinit': free creds + session, clear the state.
/// Returns t when the process had a live GnuTLS context.
pub(crate) fn deinit_proc_state(pb: &mut crate::lisp::process::Proc) -> bool {
    let Some(state) = pb.gnutls.take() else {
        return false;
    };
    if let Some(g) = gnutls() {
        unsafe {
            if !state.cred.is_null() {
                if state.anon {
                    if let Some(f) = g.anon_free_cred {
                        f(state.cred);
                    }
                } else if let Some(f) = g.cert_free_cred {
                    f(state.cred);
                }
            }
            if let Some(f) = g.session_deinit {
                if !state.session.is_null() {
                    f(state.session);
                }
            }
        }
    }
    if pb.gnutls_initstage >= 1 {
        pb.gnutls_initstage = 0;
    }
    true
}

fn f_deinit(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    want_proc_v(i, &a[0])?;
    if let Value::Process(p) = &a[0] {
        if deinit_proc_state(&mut p.borrow_mut()) {
            return Ok(Value::t());
        }
    }
    Ok(Value::Nil)
}

fn f_bye(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    want_proc_v(i, &a[0])?;
    // GNU calls `gnutls_bye' unconditionally — a NULL session returns
    // GNUTLS_E_INVALID_SESSION → `gnutls-e-invalid-session'.
    let hd = match &a[0] {
        Value::Process(p) => p.borrow().gnutls_session(),
        _ => None,
    };
    let ret = match (gnutls(), hd) {
        (Some(g), Some(hd)) => match g.bye {
            Some(bye) => unsafe { bye(hd, bye_how(&a[1])) },
            None => GNUTLS_E_INVALID_SESSION,
        },
        _ => GNUTLS_E_INVALID_SESSION,
    };
    if let Value::Process(p) = &a[0] {
        let mut pb = p.borrow_mut();
        if let Some(s) = pb.gnutls.as_mut() {
            s.certificates.clear();
        }
    }
    Ok(gnutls_make_error(i, ret))
}

fn bye_how(cont: &Value) -> u32 {
    // GNU: NILP(cont) → GNUTLS_SHUT_RDWR(2), else GNUTLS_SHUT_WR(1).
    match cont {
        Value::Nil => 2,
        _ => 1,
    }
}

fn f_peer_status(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    want_proc_v(i, &a[0])?;
    let Value::Process(p) = &a[0] else {
        return Ok(Value::Nil);
    };
    let pb = p.borrow();
    // GNU requires GNUTLS_STAGE_READY.
    if pb.gnutls.is_none() || pb.gnutls_initstage != STAGE_READY {
        return Ok(Value::Nil);
    }
    let Some(g) = gnutls() else {
        return Ok(Value::Nil);
    };
    let state = pb.gnutls.as_ref().unwrap();
    let hd = state.session;
    let verification = state.verify_status;
    let host_mismatch = state.host_mismatch;
    let certs = state.certificates.clone();
    drop(pb);

    // Warnings — GNU's cons order, each check prepending.
    let mut warnings = Value::Nil;
    let warn = |i: &mut Interp, warnings: &mut Value, name: &str| {
        *warnings = Value::cons(Value::Sym(intern(i, name)), warnings.clone());
    };
    // gnutls_certificate_status_t (gnutls/gnutls.h).
    const CERT_INVALID: u32 = 1 << 1;
    const CERT_REVOKED: u32 = 1 << 5;
    const CERT_SIGNER_NOT_FOUND: u32 = 1 << 6;
    const CERT_SIGNER_NOT_CA: u32 = 1 << 7;
    const CERT_INSECURE_ALGORITHM: u32 = 1 << 8;
    const CERT_NOT_ACTIVATED: u32 = 1 << 9;
    const CERT_EXPIRED: u32 = 1 << 10;
    const CERT_SIGNATURE_FAILURE: u32 = 1 << 11;
    const CERT_REVOCATION_DATA_SUPERSEDED: u32 = 1 << 12;
    const CERT_REVOCATION_DATA_ISSUED_IN_FUTURE: u32 = 1 << 15;
    const CERT_SIGNER_CONSTRAINTS_FAILURE: u32 = 1 << 16;
    const CERT_MISMATCH: u32 = 1 << 17;
    const CERT_PURPOSE_MISMATCH: u32 = 1 << 18;
    const CERT_MISSING_OCSP_STATUS: u32 = 1 << 19;
    const CERT_INVALID_OCSP_STATUS: u32 = 1 << 20;
    for (bit, name) in [
        (CERT_INVALID, ":invalid"),
        (CERT_REVOKED, ":revoked"),
        (CERT_SIGNER_NOT_FOUND, ":unknown-ca"),
        (CERT_SIGNER_NOT_CA, ":not-ca"),
        (CERT_INSECURE_ALGORITHM, ":insecure"),
        (CERT_NOT_ACTIVATED, ":not-activated"),
        (CERT_EXPIRED, ":expired"),
        (CERT_SIGNATURE_FAILURE, ":signature-failure"),
        (CERT_REVOCATION_DATA_SUPERSEDED, ":revocation-data-superseded"),
        (CERT_REVOCATION_DATA_ISSUED_IN_FUTURE, ":revocation-data-issued-in-future"),
        (CERT_SIGNER_CONSTRAINTS_FAILURE, ":signer-constraints-failure"),
        (CERT_PURPOSE_MISMATCH, ":purpose-mismatch"),
        (CERT_MISSING_OCSP_STATUS, ":missing-ocsp-status"),
        (CERT_INVALID_OCSP_STATUS, ":invalid-ocsp-status"),
    ] {
        if verification & bit != 0 {
            warn(i, &mut warnings, name);
        }
    }
    let _ = CERT_MISMATCH;
    if host_mismatch {
        warn(i, &mut warnings, ":no-host-match");
    }
    // Self-signed check via the first chain cert.
    if !certs.is_empty() {
        if let (Some(init), Some(import), Some(chk), Some(deinit)) = (
            g.x509_crt_init,
            g.x509_crt_import,
            g.x509_crt_check_issuer,
            g.x509_crt_deinit,
        ) {
            let mut crt: Hd = std::ptr::null_mut();
            unsafe {
                if init(&mut crt) >= 0 {
                    let d = Datum {
                        data: certs[0].as_ptr() as *mut u8,
                        size: certs[0].len() as u32,
                    };
                    if import(crt, &d, GNUTLS_X509_FMT_DER) >= 0 && chk(crt, crt) != 0 {
                        warn(i, &mut warnings, ":self-signed");
                    }
                    deinit(crt);
                }
            }
        }
    }

    let mut result = if warnings.is_nil() {
        Value::Nil
    } else {
        Value::list(vec![Value::Sym(intern(i, ":warnings")), warnings])
    };
    let mut kv = |i: &mut Interp, k: &str, v: Value| {
        let key = Value::Sym(intern(i, k));
        append_in_place(i, &mut result, Value::list(vec![key, v]));
    };

    if !certs.is_empty() {
        let mut plist_chain = Vec::new();
        for der in &certs {
            if let Some(cp) = certificate_details(i, g, der) {
                plist_chain.push(cp);
            }
        }
        let certs_v = Value::list(plist_chain);
        kv(i, ":certificates", certs_v.clone());
        kv(i, ":certificate", car_safe(&certs_v));
    }

    // Connection properties (all optional like GNU's feature ifdefs).
    unsafe {
        if let Some(f) = g.dh_get_prime_bits {
            let bits = f(hd);
            if bits >= 0 {
                kv(i, ":diffie-hellman-prime-bits", Value::Int(bits as i128));
            }
        }
        if let (Some(kxg), Some(kxn)) = (g.kx_get, g.kx_get_name) {
            let s = Value::string(cstr(kxn(kxg(hd))));
            kv(i, ":key-exchange", s);
        }
        if let (Some(pv), Some(pn)) = (g.protocol_get_version, g.protocol_get_name) {
            let s = Value::string(cstr(pn(pv(hd))));
            kv(i, ":protocol", s);
        }
        if let Some(cget) = g.cipher_get {
            let s = Value::string(cstr((g.cipher_get_name)(cget(hd))));
            kv(i, ":cipher", s);
        }
        if let Some(mget) = g.mac_get {
            let s = Value::string(cstr((g.mac_get_name)(mget(hd))));
            kv(i, ":mac", s);
        }
        if let (Some(cget), Some(cname)) = (g.compression_get, g.compression_get_name) {
            let s = Value::string(cstr(cname(cget(hd))));
            kv(i, ":compression", s);
        }
        if let Some(f) = g.session_etm_status {
            let v = if f(hd) != 0 { Value::t() } else { Value::Nil };
            kv(i, ":encrypt-then-mac", v);
        }
        if let Some(f) = g.safe_renegotiation_status {
            let v = if f(hd) != 0 { Value::t() } else { Value::Nil };
            kv(i, ":safe-renegotiation", v);
        }
    }
    Ok(result)
}

/// Append PAIR (a fresh list) to the end of list RESULT.
fn append_in_place(_i: &mut Interp, result: &mut Value, pair: Value) {
    if result.is_nil() {
        *result = pair;
        return;
    }
    let mut cur = result.clone();
    loop {
        let next = match &cur {
            Value::Cons(c) => c.borrow().cdr.clone(),
            _ => break,
        };
        match next {
            Value::Nil => {
                if let Value::Cons(c) = &cur {
                    c.borrow_mut().cdr = pair;
                }
                return;
            }
            _ => cur = next,
        }
    }
}

/// GNU `emacs_gnutls_certificate_details' — build the per-certificate
/// plist from the stored DER blob.
fn certificate_details(i: &mut Interp, g: &GnutlsFns, der: &[u8]) -> Option<Value> {
    let (init, import, deinit) = (
        g.x509_crt_init?,
        g.x509_crt_import?,
        g.x509_crt_deinit?,
    );
    let mut crt: Hd = std::ptr::null_mut();
    unsafe {
        if init(&mut crt) < 0 {
            return None;
        }
        let d = Datum {
            data: der.as_ptr() as *mut u8,
            size: der.len() as u32,
        };
        let ir = import(crt, &d, GNUTLS_X509_FMT_DER);
        if std::env::var("GNUTLS_DEBUG").is_ok() {
            eprintln!("crt import ret={ir} der_len={}", der.len());
        }
        if ir < 0 {
            deinit(crt);
            return None;
        }
    }
    let mut res = Value::Nil;
    let kv = |i: &mut Interp, res: &mut Value, k: &str, v: Value| {
        let key = Value::Sym(intern(i, k));
        append_in_place(i, res, Value::list(vec![key, v]));
    };

    unsafe {
        // :version
        if let Some(f) = g.x509_crt_get_version {
            let v = f(crt);
            if v > 0 {
                kv(i, &mut res, ":version", Value::Int(v as i128));
            }
        }
        // :serial-number (hex)
        if let Some(f) = g.x509_crt_get_serial {
            if let Some(buf) = sized_buf(|b, n| f(crt, b, n)) {
                kv(i, &mut res, ":serial-number", Value::string(hex(&buf, "")));
            }
        }
        // :issuer
        if let Some(f) = g.x509_crt_get_issuer_dn {
            if let Some(buf) = sized_buf(|b, n| f(crt, b, n)) {
                kv(
                    i,
                    &mut res,
                    ":issuer",
                    Value::string(String::from_utf8_lossy(&buf).into_owned()),
                );
            }
        }
        // :valid-from / :valid-to (%Y-%m-%d UTC)
        for (k, f) in [
            (":valid-from", g.x509_crt_get_activation_time),
            (":valid-to", g.x509_crt_get_expiration_time),
        ] {
            if let Some(f) = f {
                let t = f(crt);
                if t >= 0 {
                    if let Some(s) = fmt_ymd(t) {
                        kv(i, &mut res, k, Value::string(s));
                    }
                }
            }
        }
        // :subject
        if let Some(f) = g.x509_crt_get_dn {
            if let Some(buf) = sized_buf(|b, n| f(crt, b, n)) {
                kv(
                    i,
                    &mut res,
                    ":subject",
                    Value::string(String::from_utf8_lossy(&buf).into_owned()),
                );
            }
        }
        // :public-key-algorithm / :certificate-security-level
        if let Some(f) = g.x509_crt_get_pk_algorithm {
            let mut bits: u32 = 0;
            let pk = f(crt, &mut bits);
            if pk >= 0 {
                if let Some(n) = g.pk_algorithm_get_name {
                    let name = cstr(n(pk));
                    if !name.is_empty() {
                        kv(i, &mut res, ":public-key-algorithm", Value::string(name));
                    }
                }
                if let (Some(b2s), Some(sn)) = (g.pk_bits_to_sec_param, g.sec_param_get_name) {
                    let name = cstr(sn(b2s(pk, bits)));
                    if !name.is_empty() {
                        kv(i, &mut res, ":certificate-security-level", Value::string(name));
                    }
                }
            }
        }
        // :issuer-unique-id / :subject-unique-id
        for (k, f) in [
            (":issuer-unique-id", g.x509_crt_get_issuer_unique_id),
            (":subject-unique-id", g.x509_crt_get_subject_unique_id),
        ] {
            if let Some(f) = f {
                if let Some(buf) = sized_buf(|b, n| f(crt, b, n)) {
                    kv(
                        i,
                        &mut res,
                        k,
                        Value::string(String::from_utf8_lossy(&buf).into_owned()),
                    );
                }
            }
        }
        // :signature-algorithm
        if let (Some(f), Some(sn)) = (g.x509_crt_get_signature_algorithm, g.sign_get_name) {
            let sa = f(crt);
            if sa >= 0 {
                kv(
                    i,
                    &mut res,
                    ":signature-algorithm",
                    Value::string(cstr(sn(sa))),
                );
            }
        }
        // :public-key-id (sha1 hex) / :public-key-id-sha256
        if let Some(f) = g.x509_crt_get_key_id {
            if let Some(buf) = sized_buf(|b, n| f(crt, 0, b, n)) {
                kv(i, &mut res, ":public-key-id", Value::string(hex(&buf, "sha1:")));
            }
            if let Some(buf) = sized_buf(|b, n| f(crt, 1 /* KEYID_USE_SHA256 */, b, n)) {
                kv(
                    i,
                    &mut res,
                    ":public-key-id-sha256",
                    Value::string(hex(&buf, "sha256:")),
                );
            }
        }
        // :certificate-id (SHA-1 fingerprint) / sha256 variant is GNU's
        // :certificate-id plus :pem.
        if let Some(f) = g.x509_crt_get_fingerprint {
            // GNUTLS_DIG_SHA1 = 3.
            if let Some(buf) = sized_buf(|b, n| f(crt, 3, b, n)) {
                kv(i, &mut res, ":certificate-id", Value::string(hex(&buf, "sha1:")));
            }
        }
        // :pem
        if let Some(f) = g.x509_crt_export {
            if let Some(buf) = sized_buf(|b, n| f(crt, GNUTLS_X509_FMT_PEM, b, n)) {
                kv(
                    i,
                    &mut res,
                    ":pem",
                    Value::string(String::from_utf8_lossy(&buf).into_owned()),
                );
            }
        }
        deinit(crt);
    }
    Some(res)
}

/// Call FN with a growing buffer until it succeeds — the GnuTLS
/// `*_get_*' pattern returning GNUTLS_E_SHORT_MEMORY_BUFFER (-51)
/// with the required size.
fn sized_buf(f: impl Fn(*mut u8, *mut usize) -> i32) -> Option<Vec<u8>> {
    const SHORT: i32 = E_SHORT_MEMORY_BUFFER;
    let mut n: usize = 0;
    let r = f(std::ptr::null_mut(), &mut n);
    if r != SHORT || n == 0 {
        return None;
    }
    let mut buf = vec![0u8; n];
    let r = f(buf.as_mut_ptr(), &mut n);
    if r < 0 {
        return None;
    }
    buf.truncate(n);
    // DN getters NUL-terminate.
    if buf.last() == Some(&0) {
        buf.pop();
    }
    Some(buf)
}

fn hex(bytes: &[u8], prefix: &str) -> String {
    let mut s = String::from(prefix);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Format `time_t' as "%Y-%m-%d" in UTC (GNU's valid-from/to format).
fn fmt_ymd(t: i64) -> Option<String> {
    let days = t.div_euclid(86400);
    let secs = t.rem_euclid(86400);
    let _ = secs;
    // civil-from-days algorithm.
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    Some(format!("{:04}-{:02}-{:02}", y, m, d))
}

fn f_peer_status_warn_desc(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let Value::Sym(s) = &a[0] else {
        return Err(i.wrong_type_mut("symbolp", &a[0]));
    };
    let name = i.symbol_name(*s);
    let msg = match name.as_str() {
        ":invalid" => "certificate could not be verified",
        ":revoked" => "certificate was revoked (CRL)",
        ":self-signed" => "certificate signer was not found (self-signed)",
        ":unknown-ca" => "the certificate was signed by an unknown and therefore untrusted authority",
        ":not-ca" => "certificate signer is not a CA",
        ":insecure" => "certificate was signed with an insecure algorithm",
        ":not-activated" => "certificate is not yet activated",
        ":expired" => "certificate has expired",
        ":no-host-match" => "certificate host does not match hostname",
        ":signature-failure" => "certificate signature could not be verified",
        ":revocation-data-superseded" => {
            "certificate revocation data are old and have been superseded"
        }
        ":revocation-data-issued-in-future" => {
            "certificate revocation data have a future issue date"
        }
        ":signer-constraints-failure" => "certificate signer constraints were violated",
        ":purpose-mismatch" => "certificate does not match the intended purpose",
        ":missing-ocsp-status" => {
            "certificate requires the server to send a OCSP certificate status, but no status was received"
        }
        ":invalid-ocsp-status" => "the received OCSP certificate status is invalid",
        _ => return Ok(Value::Nil),
    };
    Ok(Value::string(msg))
}

fn f_format_certificate(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let cert = super::want_string(i, &a[0])?;
    let Some(g) = gnutls() else {
        return Ok(Value::Nil);
    };
    let (Some(init), Some(import), Some(print), Some(deinit)) = (
        g.x509_crt_init,
        g.x509_crt_import,
        g.x509_crt_print,
        g.x509_crt_deinit,
    ) else {
        return Ok(Value::Nil);
    };
    let mut crt: Hd = std::ptr::null_mut();
    unsafe {
        let ret = init(&mut crt);
        if ret < 0 {
            return Err(i.error(&format!(
                "gnutls-format-certificate error: {}",
                strerror(g, ret)
            )));
        }
        // GNU uses strlen() on the PEM string — import as PEM only.
        let bytes = cert.as_bytes();
        let d = Datum {
            data: bytes.as_ptr() as *mut u8,
            size: bytes.len() as u32,
        };
        let ret = import(crt, &d, GNUTLS_X509_FMT_PEM);
        if ret < 0 {
            deinit(crt);
            return Err(i.error(&format!(
                "gnutls-format-certificate error: {}",
                strerror(g, ret)
            )));
        }
        let mut out = Datum {
            data: std::ptr::null_mut(),
            size: 0,
        };
        let ret = print(crt, 0 /* GNUTLS_CRT_PRINT_FULL */, &mut out);
        deinit(crt);
        if ret < 0 {
            return Err(i.error(&format!(
                "gnutls-format-certificate error: {}",
                strerror(g, ret)
            )));
        }
        if out.data.is_null() {
            return Ok(Value::Nil);
        }
        let s = {
            let slice = std::slice::from_raw_parts(out.data, out.size as usize);
            String::from_utf8_lossy(slice).into_owned()
        };
        if let Some(f) = g.free {
            f(out.data as *mut core::ffi::c_void);
        }
        Ok(Value::string(s))
    }
}

// ---------------------------------------------------------------------------
// gnutls-boot
// ---------------------------------------------------------------------------

/// GNU stage constants (src/gnutls.h).
const STAGE_EMPTY: i32 = 0;
const STAGE_CRED_ALLOC: i32 = 1;
const STAGE_FILES: i32 = 2;
const STAGE_CALLBACKS: i32 = 3;
const STAGE_INIT: i32 = 4;
const STAGE_PRIORITY: i32 = 5;
const STAGE_CRED_SET: i32 = 6;
const STAGE_HANDSHAKE_CANDO: i32 = STAGE_CRED_SET;
const STAGE_TRANSPORT_PTRS: i32 = 7;
const STAGE_HANDSHAKE_TRIED: i32 = 8;
const STAGE_READY: i32 = 9;

// gnutls_init flags (gnutls_connection_end_t in modern GnuTLS).
#[allow(dead_code)]
const GNUTLS_SERVER: u32 = 1;
const GNUTLS_CLIENT: u32 = 1 << 1;
const GNUTLS_CRD_CERTIFICATE: u32 = 1;
const GNUTLS_CRD_ANON: u32 = 2;
const GNUTLS_NAME_DNS: u32 = 1;
// gnutls_x509_crt_fmt_t.
const GNUTLS_X509_FMT_DER: u32 = 0;
const GNUTLS_X509_FMT_PEM: u32 = 1;
// GNUTLS_E_SHORT_MEMORY_BUFFER.
const E_SHORT_MEMORY_BUFFER: i32 = -51;
// GNU's default gnutls_verify_flags (GNUTLS_VERIFY_ALLOW_X509_V1_CA_CRT,
// spelled GNUTLS_VERIFY_ALLOW_ANY_X509_V1_CA_CRT in modern GnuTLS).
const GNUTLS_VERIFY_ALLOW_X509_V1_CA_CRT: u32 = 1 << 3;
const GNUTLS_CRT_X509: i32 = 1;
const GNUTLS_E_UNEXPECTED_PACKET_LENGTH: i32 = -9;

/// GNU `boot_error': for our synchronous processes it signals `error'.
fn boot_error(i: &mut Interp, msg: &str) -> Flow {
    i.error(msg)
}

fn is_ip_literal(host: &str) -> bool {
    host.parse::<std::net::IpAddr>().is_ok()
}

fn f_boot(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    want_proc_v(i, &a[0])?;
    let type_id = match &a[1] {
        Value::Sym(s) => *s,
        _ => return Err(i.wrong_type_mut("symbolp", &a[1])),
    };
    let proplist = &a[2];
    if !matches!(proplist, Value::Cons(_) | Value::Nil) {
        return Err(i.wrong_type_mut("listp", proplist));
    }

    if f_available_p(i, vec![])?.is_nil() {
        return Err(boot_error(i, "GnuTLS not available"));
    }
    let g = gnutls().unwrap();

    let t_x509 = intern(i, "gnutls-x509pki");
    let t_anon = intern(i, "gnutls-anon");
    if type_id != t_x509 && type_id != t_anon {
        return Err(boot_error(i, "Invalid GnuTLS credential type"));
    }

    let mut pl = |key: &str| plist_get(proplist, intern(i, key));
    let hostname = pl(":hostname");
    let priority_string = pl(":priority");
    let trustfiles = pl(":trustfiles");
    let keylist = pl(":keylist");
    let crlfiles = pl(":crlfiles");
    let loglevel = pl(":loglevel");
    let prime_bits = pl(":min-prime-bits");
    let _pass = pl(":pass");
    let _flags = pl(":flags");

    let hostname_s = match &hostname {
        Value::Str(s) => s.borrow().clone(),
        _ => {
            return Err(boot_error(
                i,
                "gnutls-boot: invalid :hostname parameter (not a string)",
            ))
        }
    };

    if let Value::Int(l) = &loglevel {
        if let Value::Process(p) = &a[0] {
            p.borrow_mut().gnutls_log_level = (*l).clamp(i32::MIN as i128, i32::MAX as i128) as i32;
        }
    }

    // Global init (once).
    if let Some(ginit) = g.global_init {
        static GLOBAL_INITED: OnceLock<bool> = OnceLock::new();
        GLOBAL_INITED.get_or_init(|| unsafe {
            ginit();
            true
        });
    }

    // Drop any existing credentials/state.
    if let Value::Process(p) = &a[0] {
        deinit_proc_state(&mut p.borrow_mut());
        p.borrow_mut().gnutls_initstage = STAGE_EMPTY;
    }

    // Allocate credentials.
    let mut cred: Hd = std::ptr::null_mut();
    if type_id == t_x509 {
        let Some(alloc) = g.cert_alloc_cred else {
            return Err(boot_error(i, "GnuTLS x509 credentials unavailable"));
        };
        let ret = unsafe { alloc(&mut cred) };
        if ret < 0 {
            return Err(i.error(&format!(
                "memory exhausted: {}",
                strerror(g, ret)
            )));
        }
        let verify_flags = pl(":verify-flags");
        let vf = match &verify_flags {
            Value::Int(n) => *n as u32,
            _ => GNUTLS_VERIFY_ALLOW_X509_V1_CA_CRT,
        };
        if let Some(svf) = g.cert_set_verify_flags {
            unsafe { svf(cred, vf) };
        }
    } else {
        let Some(alloc) = g.anon_alloc_cred else {
            return Err(boot_error(i, "GnuTLS anon credentials unavailable"));
        };
        let ret = unsafe { alloc(&mut cred) };
        if ret < 0 {
            return Err(i.error(&format!(
                "memory exhausted: {}",
                strerror(g, ret)
            )));
        }
    }

    // Stash partial state so deinit can free it on later failure.
    if let Value::Process(p) = &a[0] {
        let mut pb = p.borrow_mut();
        pb.gnutls = Some(Box::new(crate::lisp::process::GnutlsState {
            session: std::ptr::null_mut(),
            cred,
            hostname: hostname_s.clone(),
            verify_status: 0,
            certificates: Vec::new(),
            host_mismatch: false,
            anon: type_id == t_anon,
        }));
        pb.gnutls_initstage = STAGE_CRED_ALLOC;
    }

    if type_id == t_x509 {
        // System trust first (GNU's HAVE_GNUTLS_X509_SYSTEM_TRUST).
        if let Some(f) = g.cert_set_system_trust {
            let ret = unsafe { f(cred) };
            let _ = ret; // GNU only logs the failure
        }
        // Trust files.
        let mut cur = trustfiles;
        while let Value::Cons(c) = cur {
            let (file, next) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            cur = next;
            match &file {
                Value::Str(s) => {
                    let path = crate::editor::expand_file_name_str(i, &s.borrow());
                    let cpath = std::ffi::CString::new(path).unwrap_or_default();
                    let Some(f) = g.cert_set_x509_trust_file else { continue };
                    let ret = unsafe {
                        f(cred, cpath.as_ptr(), GNUTLS_X509_FMT_PEM)
                    };
                    if ret < 0 {
                        return Ok(gnutls_make_error(i, ret));
                    }
                }
                _ => {
                    if let Value::Process(p) = &a[0] {
                        deinit_proc_state(&mut p.borrow_mut());
                    }
                    return Err(boot_error(i, "Invalid trustfile"));
                }
            }
        }
        // CRL files.
        let mut cur = crlfiles;
        while let Value::Cons(c) = cur {
            let (file, next) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            cur = next;
            match &file {
                Value::Str(s) => {
                    let path = crate::editor::expand_file_name_str(i, &s.borrow());
                    let cpath = std::ffi::CString::new(path).unwrap_or_default();
                    let Some(f) = g.cert_set_x509_crl_file else { continue };
                    let ret = unsafe { f(cred, cpath.as_ptr(), GNUTLS_X509_FMT_PEM) };
                    if ret < 0 {
                        return Ok(gnutls_make_error(i, ret));
                    }
                }
                _ => {
                    if let Value::Process(p) = &a[0] {
                        deinit_proc_state(&mut p.borrow_mut());
                    }
                    return Err(boot_error(i, "Invalid CRL file"));
                }
            }
        }
        // Key list: alist of (key-file . cert-file)? GNU takes
        // (car elem)=keyfile, (car (cdr elem))=certfile.
        let mut cur = keylist;
        while let Value::Cons(c) = cur {
            let (elem, next) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            cur = next;
            let keyfile = car_safe(&elem);
            let certfile = car_safe(&cdr_safe(&elem));
            match (&keyfile, &certfile) {
                (Value::Str(k), Value::Str(cf)) => {
                    let kp = std::ffi::CString::new(crate::editor::expand_file_name_str(
                        i,
                        &k.borrow(),
                    ))
                    .unwrap_or_default();
                    let cp = std::ffi::CString::new(crate::editor::expand_file_name_str(
                        i,
                        &cf.borrow(),
                    ))
                    .unwrap_or_default();
                    let Some(f) = g.cert_set_x509_key_file else { continue };
                    let ret = unsafe {
                        f(cred, cp.as_ptr(), kp.as_ptr(), GNUTLS_X509_FMT_PEM)
                    };
                    if ret < 0 {
                        return Ok(gnutls_make_error(i, ret));
                    }
                }
                _ => {
                    if let Value::Process(p) = &a[0] {
                        deinit_proc_state(&mut p.borrow_mut());
                    }
                    return Err(boot_error(
                        i,
                        if matches!(&keyfile, Value::Str(_)) {
                            "Invalid client cert file"
                        } else {
                            "Invalid client key file"
                        },
                    ));
                }
            }
        }
    }

    if let Value::Process(p) = &a[0] {
        let mut pb = p.borrow_mut();
        pb.gnutls_initstage = STAGE_FILES;
        pb.gnutls_initstage = STAGE_CALLBACKS;
    }

    // gnutls_init.
    let mut session: Hd = std::ptr::null_mut();
    let Some(sinit) = g.session_init else {
        return Err(boot_error(i, "GnuTLS session API unavailable"));
    };
    let ret = unsafe { sinit(&mut session, GNUTLS_CLIENT) };
    if ret < 0 {
        return Ok(gnutls_make_error(i, ret));
    }
    if let Value::Process(p) = &a[0] {
        let mut pb = p.borrow_mut();
        if let Some(st) = pb.gnutls.as_mut() {
            st.session = session;
        }
        pb.gnutls_initstage = STAGE_INIT;
    }

    // Priority string.
    let prio = match &priority_string {
        Value::Str(s) => s.borrow().clone(),
        _ => "NORMAL".to_string(),
    };
    let cprio = std::ffi::CString::new(prio).unwrap_or_default();
    let Some(pset) = g.priority_set_direct else {
        return Err(boot_error(i, "GnuTLS priority API unavailable"));
    };
    let ret = unsafe { pset(session, cprio.as_ptr(), std::ptr::null_mut()) };
    if ret < 0 {
        return Ok(gnutls_make_error(i, ret));
    }
    if let Value::Process(p) = &a[0] {
        p.borrow_mut().gnutls_initstage = STAGE_PRIORITY;
    }

    if let (Value::Int(bits), Some(f)) = (&prime_bits, g.dh_set_prime_bits) {
        unsafe { f(session, (*bits).max(0) as u32) };
    }

    // Credentials.
    let Some(cset) = g.credentials_set else {
        return Err(boot_error(i, "GnuTLS credentials API unavailable"));
    };
    let ret = unsafe {
        cset(
            session,
            if type_id == t_x509 {
                GNUTLS_CRD_CERTIFICATE
            } else {
                GNUTLS_CRD_ANON
            },
            cred,
        )
    };
    if std::env::var("GNUTLS_DEBUG").is_ok() {
        eprintln!("cset session={session:?} cred={cred:?} ret={ret}");
    }
    if ret < 0 {
        return Ok(gnutls_make_error(i, ret));
    }

    // SNI.
    if !is_ip_literal(&hostname_s) {
        if let Some(sni) = g.server_name_set {
            let ret = unsafe {
                sni(
                    session,
                    GNUTLS_NAME_DNS,
                    hostname_s.as_ptr() as *const core::ffi::c_void,
                    hostname_s.len(),
                )
            };
            if std::env::var("GNUTLS_DEBUG").is_ok() {
                eprintln!("sni ret={ret} host={hostname_s}");
            }
            if ret < 0 {
                return Ok(gnutls_make_error(i, ret));
            }
        }
    }

    if let Value::Process(p) = &a[0] {
        p.borrow_mut().gnutls_initstage = STAGE_CRED_SET;
    }

    // Handshake: bind transport to the process socket and loop.
    let ret = handshake(i, &a[0], session, g);
    if ret < GNUTLS_E_SUCCESS {
        return Ok(gnutls_make_error(i, ret));
    }

    verify_boot(i, &a[0], proplist, g)
}

fn car_safe(v: &Value) -> Value {
    match v {
        Value::Cons(c) => c.borrow().car.clone(),
        _ => Value::Nil,
    }
}

fn cdr_safe(v: &Value) -> Value {
    match v {
        Value::Cons(c) => c.borrow().cdr.clone(),
        _ => Value::Nil,
    }
}

/// GNU `emacs_gnutls_handshake' + `gnutls_try_handshake'.
fn handshake(i: &mut Interp, proc: &Value, session: Hd, g: &GnutlsFns) -> i32 {
    let (infd, outfd) = match proc {
        Value::Process(p) => {
            let pb = p.borrow();
            if pb.gnutls_initstage < STAGE_HANDSHAKE_CANDO {
                return -1;
            }
            match &pb.io {
                crate::lisp::process::ProcIo::Net(s) => {
                    use std::os::unix::io::AsRawFd;
                    let fd = s.as_raw_fd();
                    (fd, fd)
                }
                _ => return GNUTLS_E_INVALID_SESSION,
            }
        }
        _ => return GNUTLS_E_INVALID_SESSION,
    };
    if let Some(t) = g.transport_set_ptr2 {
        unsafe { t(session, infd as isize, outfd as isize) };
    } else {
        return GNUTLS_E_INVALID_SESSION;
    }
    if let Value::Process(p) = proc {
        p.borrow_mut().gnutls_initstage = STAGE_TRANSPORT_PTRS;
    }

    // GNU loops on E_AGAIN/E_INTERRUPTED for blocking sockets; ours
    // are nonblocking, so retry AGAIN too — bounded by a deadline.
    let Some(hs) = g.handshake else {
        return GNUTLS_E_INVALID_SESSION;
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let mut ret;
    loop {
        ret = unsafe { hs(session) };
        if ret >= 0 {
            break;
        }
        if ret != GNUTLS_E_AGAIN && ret != GNUTLS_E_INTERRUPTED {
            break;
        }
        if std::time::Instant::now() > deadline {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    if let Value::Process(p) = proc {
        p.borrow_mut().gnutls_initstage = STAGE_HANDSHAKE_TRIED;
        if ret == GNUTLS_E_SUCCESS {
            p.borrow_mut().gnutls_initstage = STAGE_READY;
        }
    }
    let _ = i;
    ret
}

/// GNU `gnutls_verify_boot'.
fn verify_boot(i: &mut Interp, proc: &Value, proplist: &Value, g: &GnutlsFns) -> EvalResult {
    let verify_error = plist_get(proplist, intern(i, ":verify-error"));
    let hostname = plist_get(proplist, intern(i, ":hostname"));
    let verify_all = matches!(&verify_error, Value::Sym(s) if *s == crate::lisp::obarray::sym::T);
    if !verify_all && !matches!(&verify_error, Value::Nil | Value::Cons(_)) {
        return Err(boot_error(
            i,
            "gnutls-boot: invalid :verify_error parameter (not a list)",
        ));
    }
    let hostname_s = match &hostname {
        Value::Str(s) => s.borrow().clone(),
        _ => {
            return Err(boot_error(
                i,
                "gnutls-boot: invalid :hostname parameter (not a string)",
            ))
        }
    };
    let mut member = |sym: &str| -> bool {
        let id = intern(i, sym);
        let mut cur = verify_error.clone();
        while let Value::Cons(c) = cur {
            let (car, cdr) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            if matches!(car, Value::Sym(s) if s == id) {
                return true;
            }
            cur = cdr;
        }
        false
    };

    let hd = match proc {
        Value::Process(p) => p.borrow().gnutls_session(),
        _ => None,
    };
    let Some(hd) = hd else {
        return Err(boot_error(i, "GnuTLS session is missing"));
    };

    // Peer verification status.
    let mut status: u32 = 0;
    let ret = match g.certificate_verify_peers2 {
        Some(f) => unsafe { f(hd, &mut status) },
        None => GNUTLS_E_SUCCESS,
    };
    if ret < GNUTLS_E_SUCCESS {
        return Ok(gnutls_make_error(i, ret));
    }
    if let Value::Process(p) = proc {
        let mut pb = p.borrow_mut();
        if let Some(st) = pb.gnutls.as_mut() {
            st.verify_status = status;
        }
    }
    if status != 0 && (verify_all || member(":trustfiles")) {
        if let Value::Process(p) = proc {
            deinit_proc_state(&mut p.borrow_mut());
        }
        return Err(boot_error(
            i,
            &format!(
                "Certificate validation failed {}, verification code {:x}",
                hostname_s, status
            ),
        ));
    }

    // Certificate chain.
    let is_x509 = g
        .certificate_type_get
        .map(|f| unsafe { f(hd) == GNUTLS_CRT_X509 })
        .unwrap_or(false);
    if is_x509 {
        if let Some(get_peers) = g.certificate_get_peers {
            let mut n: u32 = 0;
            let list = unsafe { get_peers(hd, &mut n) };
            if std::env::var("GNUTLS_DEBUG").is_ok() {
                eprintln!("get_peers list={list:?} n={n}");
            }
            if list.is_null() || n == 0 {
                if let Value::Process(p) = proc {
                    deinit_proc_state(&mut p.borrow_mut());
                }
                return Err(boot_error(i, "No x509 certificate was found\n"));
            }
            let mut certs = Vec::new();
            unsafe {
                for k in 0..n as usize {
                    let d = *list.add(k);
                    certs.push(
                        std::slice::from_raw_parts(d.data, d.size as usize).to_vec(),
                    );
                }
            }
            if let Value::Process(p) = proc {
                if let Some(st) = p.borrow_mut().gnutls.as_mut() {
                    st.certificates = certs.clone();
                }
            }
            // Hostname check on the first cert.
            if let (Some(init), Some(import), Some(chk), Some(deinit)) = (
                g.x509_crt_init,
                g.x509_crt_import,
                g.x509_crt_check_hostname,
                g.x509_crt_deinit,
            ) {
                let mut crt: Hd = std::ptr::null_mut();
                unsafe {
                    if init(&mut crt) >= 0 {
                        let d = Datum {
                            data: certs[0].as_ptr() as *mut u8,
                            size: certs[0].len() as u32,
                        };
                        let chost =
                            std::ffi::CString::new(hostname_s.clone()).unwrap_or_default();
                        let mut matched = false;
                        let ir = import(crt, &d, GNUTLS_X509_FMT_DER);
                        if std::env::var("GNUTLS_DEBUG").is_ok() {
                            eprintln!(
                                "hostcheck import={ir} der={} first4={:02x?}",
                                certs[0].len(),
                                &certs[0][..certs[0].len().min(4)]
                            );
                        }
                        if ir >= 0 {
                            matched = chk(crt, chost.as_ptr()) != 0;
                        }
                        if std::env::var("GNUTLS_DEBUG").is_ok() {
                            eprintln!("hostcheck matched={matched}");
                        }
                        deinit(crt);
                        if !matched {
                            if let Value::Process(p) = proc {
                                if let Some(st) = p.borrow_mut().gnutls.as_mut() {
                                    st.host_mismatch = true;
                                }
                            }
                            if verify_all || member(":hostname") {
                                if let Value::Process(p) = proc {
                                    deinit_proc_state(&mut p.borrow_mut());
                                }
                                return Err(boot_error(
                                    i,
                                    &format!(
                                        "The x509 certificate does not match \"{}\"",
                                        hostname_s
                                    ),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(gnutls_make_error(i, GNUTLS_E_SUCCESS))
}

// ---------------------------------------------------------------------------
// Record I/O — called from `lisp::process' read/write paths when the
// process has a booted TLS session.
// ---------------------------------------------------------------------------

/// GNU `emacs_gnutls_read': >0 = bytes read, 0 = peer closed,
/// -1 = would-block (errno=EAGAIN).
pub(crate) fn record_read(pb: &crate::lisp::process::Proc, buf: &mut [u8]) -> isize {
    let would_block = || -1isize;
    if pb.gnutls_initstage != STAGE_READY {
        return would_block();
    }
    let Some(st) = pb.gnutls.as_ref() else {
        return would_block();
    };
    let Some(g) = gnutls() else {
        return would_block();
    };
    let Some(recv) = g.record_recv else {
        return would_block();
    };
    let mut ret;
    loop {
        ret = unsafe { recv(st.session, buf.as_mut_ptr(), buf.len()) };
        if ret != GNUTLS_E_INTERRUPTED as isize {
            break;
        }
    }
    if ret >= 0 {
        return ret;
    }
    if ret == GNUTLS_E_UNEXPECTED_PACKET_LENGTH as isize {
        return 0; // peer closed
    }
    if ret == GNUTLS_E_AGAIN as isize {
        return would_block();
    }
    // Fatal error — GNU surfaces it via handle_error and closes.
    0
}

/// GNU `emacs_gnutls_write'.
pub(crate) fn record_write(pb: &crate::lisp::process::Proc, buf: &[u8]) -> Result<usize, ()> {
    if pb.gnutls_initstage != STAGE_READY {
        return Err(());
    }
    let Some(st) = pb.gnutls.as_ref() else {
        return Err(());
    };
    let Some(g) = gnutls() else {
        return Err(());
    };
    let Some(send) = g.record_send else {
        return Err(());
    };
    let mut written = 0usize;
    let mut rest = buf;
    while !rest.is_empty() {
        let mut ret;
        loop {
            ret = unsafe { send(st.session, rest.as_ptr(), rest.len()) };
            if ret != GNUTLS_E_INTERRUPTED as isize {
                break;
            }
        }
        if ret < 0 {
            break;
        }
        rest = &rest[ret as usize..];
        written += ret as usize;
    }
    Ok(written)
}

/// Whether the process has a live TLS session (READY).
pub(crate) fn tls_ready(pb: &crate::lisp::process::Proc) -> bool {
    pb.gnutls.is_some() && pb.gnutls_initstage == STAGE_READY
}

// ---------------------------------------------------------------------------

pub(crate) static SUBRS: &[crate::lisp::value::Subr] = &[
    S!("gnutls-ciphers", 0, 0, f_ciphers, "Return alist of GnuTLS symmetric cipher descriptions as plists."),
    S!("gnutls-macs", 0, 0, f_macs, "Return alist of GnuTLS mac-algorithm method descriptions as plists."),
    S!("gnutls-digests", 0, 0, f_digests, "Return alist of GnuTLS digest-algorithm method descriptions as plists."),
    S!("gnutls-hash-mac", 3, 3, f_hash_mac, "Hash INPUT with HASH-METHOD and KEY into a unibyte string."),
    S!("gnutls-hash-digest", 2, 2, f_hash_digest, "Digest INPUT with DIGEST-METHOD into a unibyte string."),
    S!("gnutls-symmetric-encrypt", 4, 5, f_symmetric_encrypt, "Encrypt INPUT with symmetric CIPHER, KEY+AEAD_AUTH, and IV to a unibyte string."),
    S!("gnutls-symmetric-decrypt", 4, 5, f_symmetric_decrypt, "Decrypt INPUT with symmetric CIPHER, KEY+AEAD_AUTH, and IV to a unibyte string."),
    S!("gnutls-available-p", 0, 0, f_available_p, "Return list of capabilities if GnuTLS is available in this instance of Emacs."),
    S!("gnutls-errorp", 1, 1, f_errorp, "Return t if ERROR indicates a GnuTLS problem."),
    S!("gnutls-error-fatalp", 1, 1, f_error_fatalp, "Return non-nil if ERROR is fatal."),
    S!("gnutls-error-string", 1, 1, f_error_string, "Return a description of ERROR."),
    S!("gnutls-get-initstage", 1, 1, f_get_initstage, "Return the GnuTLS init stage of PROCESS."),
    S!("gnutls-asynchronous-parameters", 3, 3, f_async_params, "Mark the result of the asynchronous GnuTLS negotiation."),
    S!("gnutls-deinit", 1, 1, f_deinit, "Deallocate GnuTLS resources associated with PROCESS."),
    S!("gnutls-bye", 2, 2, f_bye, "Terminate current GnuTLS connection for process PROC."),
    S!("gnutls-peer-status", 1, 1, f_peer_status, "Return the peer certificate status for process PROC."),
    S!("gnutls-peer-status-warning-describe", 1, 1, f_peer_status_warn_describe, "Return a description of the peer-status warning WARN."),
    S!("gnutls-format-certificate", 1, 1, f_format_certificate, "Format a X.509 certificate, DER or PEM format, to a string."),
    S!("gnutls-boot", 3, 3, f_boot, "Initialize client-side GnuTLS connection for process PROC."),
];

fn f_peer_status_warn_describe(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_peer_status_warn_desc(i, a)
}

/// `(provide 'gnutls)'-adjacent setup: GNU registers the four
/// error symbols' `gnutls-code' properties at startup.
pub(crate) fn install(i: &mut Interp) {
    let code = intern(i, "gnutls-code");
    for (name, n) in [
        ("gnutls-e-interrupted", GNUTLS_E_INTERRUPTED),
        ("gnutls-e-again", GNUTLS_E_AGAIN),
        ("gnutls-e-invalid-session", GNUTLS_E_INVALID_SESSION),
        ("gnutls-e-not-ready-for-handshake", GNUTLS_E_APPLICATION_ERROR_MIN),
    ] {
        let s = intern(i, name);
        i.put_prop(s, code, Value::Int(n as i128));
    }
    // GNU registers no `gnutls' feature at startup (gnutls.c has no
    // Fprovide) — `gnutls-available-p' reports `gnutls' as a capability
    // entry only.  Providing the feature here would make `(require
    // 'gnutls)' short-circuit, skipping gnutls.el's function defs.
}
