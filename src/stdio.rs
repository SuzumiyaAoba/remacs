//! Use the process C stdout, as GNU print.c does. In particular, a pipe is
//! block buffered, explicit flushes publish it, and exec discards pending data.
unsafe extern "C" {
    #[cfg_attr(target_vendor = "apple", link_name = "__stdoutp")]
    #[cfg_attr(not(target_vendor = "apple"), link_name = "stdout")]
    static mut C_STDOUT: *mut libc::FILE;
}

pub fn write(bytes: &[u8]) {
    unsafe { libc::fwrite(bytes.as_ptr().cast(), 1, bytes.len(), C_STDOUT); }
}

pub fn flush() {
    unsafe { libc::fflush(C_STDOUT); }
}

pub(crate) fn read_byte() -> Option<u8> {
    let byte = unsafe { libc::getchar() };
    (byte != libc::EOF).then_some(byte as u8)
}
