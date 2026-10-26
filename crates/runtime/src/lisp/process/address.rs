//! Resolve the exact host bytes; Rust's string resolver covers ordinary UTF-8.
use crate::lisp::text::EmacsStr;
use std::io;
use std::net::{SocketAddr, ToSocketAddrs};

pub(crate) fn resolve(host: &EmacsStr, port: u16) -> io::Result<std::vec::IntoIter<SocketAddr>> {
    let bytes = host.to_external_bytes();
    if let Ok(host) = std::str::from_utf8(&bytes) {
        return (host, port)
            .to_socket_addrs()
            .map(|addresses| addresses.collect::<Vec<_>>().into_iter());
    }
    native_resolve(&bytes, port).map(Vec::into_iter)
}

pub(crate) fn numeric(host: &EmacsStr, port: u16) -> io::Result<std::vec::IntoIter<SocketAddr>> {
    native_resolve_flags(&host.to_external_bytes(), port, true).map(Vec::into_iter)
}

fn native_resolve(host: &[u8], port: u16) -> io::Result<Vec<SocketAddr>> {
    native_resolve_flags(host, port, false)
}

#[cfg(unix)]
fn native_resolve_flags(host: &[u8], port: u16, numeric: bool) -> io::Result<Vec<SocketAddr>> {
    use std::ffi::{CStr, CString};
    let host = CString::new(host)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "hostname contains NUL"))?;
    let service = CString::new(port.to_string()).unwrap();
    let mut hints: libc::addrinfo = unsafe { std::mem::zeroed() };
    hints.ai_family = libc::AF_UNSPEC;
    hints.ai_socktype = libc::SOCK_STREAM;
    hints.ai_flags = libc::AI_NUMERICSERV | if numeric { libc::AI_NUMERICHOST } else { 0 };
    let mut answer = std::ptr::null_mut();
    let result = unsafe { libc::getaddrinfo(host.as_ptr(), service.as_ptr(), &hints, &mut answer) };
    if result != 0 {
        if result == libc::EAI_SYSTEM {
            return Err(io::Error::last_os_error());
        }
        let reason = unsafe {
            let message = libc::gai_strerror(result);
            if message.is_null() {
                "address lookup failed".to_owned()
            } else {
                CStr::from_ptr(message).to_string_lossy().into_owned()
            }
        };
        return Err(io::Error::other(reason));
    }
    struct Addresses(*mut libc::addrinfo);
    impl Drop for Addresses {
        fn drop(&mut self) {
            unsafe { libc::freeaddrinfo(self.0) };
        }
    }
    let _answer = Addresses(answer);
    let mut addresses = Vec::new();
    let mut next = answer;
    while !next.is_null() {
        let entry = unsafe { &*next };
        if !entry.ai_addr.is_null() {
            if entry.ai_family == libc::AF_INET
                && entry.ai_addrlen as usize >= std::mem::size_of::<libc::sockaddr_in>()
            {
                let address = unsafe { &*entry.ai_addr.cast::<libc::sockaddr_in>() };
                addresses.push(SocketAddr::from((
                    address.sin_addr.s_addr.to_ne_bytes(),
                    u16::from_be(address.sin_port),
                )));
            } else if entry.ai_family == libc::AF_INET6
                && entry.ai_addrlen as usize >= std::mem::size_of::<libc::sockaddr_in6>()
            {
                let address = unsafe { &*entry.ai_addr.cast::<libc::sockaddr_in6>() };
                addresses.push(SocketAddr::V6(std::net::SocketAddrV6::new(
                    address.sin6_addr.s6_addr.into(),
                    u16::from_be(address.sin6_port),
                    address.sin6_flowinfo,
                    address.sin6_scope_id,
                )));
            }
        }
        next = entry.ai_next;
    }
    Ok(addresses)
}

#[cfg(not(unix))]
fn native_resolve_flags(_host: &[u8], _port: u16, _numeric: bool) -> io::Result<Vec<SocketAddr>> {
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "hostname is not Unicode",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_numeric_host_preserves_port() {
        let host = crate::lisp::text::EmacsText::from("127.0.0.1");
        assert_eq!(
            resolve(&host, 4321).unwrap().collect::<Vec<_>>(),
            ["127.0.0.1:4321".parse::<SocketAddr>().unwrap()]
        );
    }
    #[cfg(unix)]
    #[test]
    fn numeric_hint_never_resolves_named_hosts() {
        let host = crate::lisp::text::EmacsText::from("localhost");
        assert!(numeric(&host, 0).is_err());
        let numeric_host = crate::lisp::text::EmacsText::from("127.1");
        assert!(
            numeric(&numeric_host, 0)
                .unwrap()
                .any(|address| address.ip().is_loopback())
        );
    }
    #[cfg(unix)]
    #[test]
    fn native_byte_resolver_handles_numeric_addresses_without_dns() {
        assert!(
            native_resolve(b"127.0.0.1", 1234)
                .unwrap()
                .iter()
                .any(|address| *address == "127.0.0.1:1234".parse::<SocketAddr>().unwrap())
        );
        assert!(
            native_resolve(b"::1", 5678)
                .unwrap()
                .iter()
                .any(|address| *address == "[::1]:5678".parse::<SocketAddr>().unwrap())
        );
        assert_eq!(
            native_resolve(b"bad\0host", 1).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }
}
