use crate::WindowAppearance;
use cocoa::{
    appkit::{NSAppearanceNameVibrantDark, NSAppearanceNameVibrantLight},
    base::{id, nil},
    foundation::NSString,
};
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::CStr;

impl WindowAppearance {
    pub(crate) unsafe fn from_native(appearance: id) -> Self {
        let name: id = msg_send![appearance, name];
        unsafe {
            if name == NSAppearanceNameVibrantLight {
                Self::VibrantLight
            } else if name == NSAppearanceNameVibrantDark {
                Self::VibrantDark
            } else if name == NSAppearanceNameAqua {
                Self::Light
            } else if name == NSAppearanceNameDarkAqua {
                Self::Dark
            } else {
                println!(
                    "unknown appearance: {:?}",
                    CStr::from_ptr(name.UTF8String())
                );
                Self::Light
            }
        }
    }
}

pub(crate) unsafe fn native_appearance(appearance: Option<WindowAppearance>) -> id {
    let Some(appearance) = appearance else {
        return nil;
    };
    let name = unsafe {
        match appearance {
            WindowAppearance::Light => NSAppearanceNameAqua,
            WindowAppearance::VibrantLight => NSAppearanceNameVibrantLight,
            WindowAppearance::Dark => NSAppearanceNameDarkAqua,
            WindowAppearance::VibrantDark => NSAppearanceNameVibrantDark,
        }
    };
    unsafe { msg_send![class!(NSAppearance), appearanceNamed: name] }
}

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    pub static NSAppearanceNameAqua: id;
    pub static NSAppearanceNameDarkAqua: id;
}
