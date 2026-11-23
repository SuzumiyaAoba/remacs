mod dispatcher;
mod headless;
mod keyboard;
mod platform;
#[cfg(any(feature = "wayland", feature = "x11"))]
mod text_system;
#[cfg(feature = "wayland")]
mod wayland;
#[cfg(feature = "x11")]
mod x11;

#[cfg(any(feature = "wayland", feature = "x11"))]
mod xdg_desktop_portal;

use http_client::Url;
use smallvec::SmallVec;

pub(crate) use dispatcher::*;
pub(crate) use headless::*;
pub(crate) use keyboard::*;
pub(crate) use platform::*;
#[cfg(any(feature = "wayland", feature = "x11"))]
pub(crate) use text_system::*;
#[cfg(feature = "wayland")]
pub(crate) use wayland::*;
#[cfg(feature = "x11")]
pub(crate) use x11::*;

pub(crate) fn external_paths_from_uri_list(content: &str) -> Option<crate::ExternalPaths> {
    let mut items = SmallVec::<[crate::ExternalPath; 2]>::new();
    for line in content.lines() {
        let Ok(url) = Url::parse(line) else {
            continue;
        };
        if url.scheme() != "file" {
            continue;
        }
        let file_url = url.to_string();
        match url.to_file_path() {
            Ok(path) => items.push(crate::ExternalPath::Path(path)),
            Err(())
                if url.host_str().is_some_and(|host| {
                    !host.is_empty() && !host.eq_ignore_ascii_case("localhost")
                }) =>
            {
                items.push(crate::ExternalPath::FileUrl(file_url));
            }
            Err(()) => {}
        }
    }
    (!items.is_empty()).then(|| crate::ExternalPaths::from_items(items))
}

#[cfg(all(feature = "screen-capture", any(feature = "wayland", feature = "x11")))]
pub(crate) type PlatformScreenCaptureFrame = scap::frame::Frame;
#[cfg(not(all(feature = "screen-capture", any(feature = "wayland", feature = "x11"))))]
pub(crate) type PlatformScreenCaptureFrame = ();
