//! Main-thread bridge for macOS AppleScript execution.

use cocoa::{
    base::{id, nil, NO},
    foundation::NSString,
};
use block::ConcreteBlock;
use objc::{class, msg_send, sel, sel_impl};
use remacs_runtime::frontend::AppleScriptResult;
use std::{
    ffi::{CStr, c_char},
    sync::mpsc,
};

const TYPE_NULL: u32 = u32::from_be_bytes(*b"null");
const TYPE_UNICODE_TEXT: u32 = u32::from_be_bytes(*b"utxt");
const TYPE_UTF16_EXTERNAL: u32 = u32::from_be_bytes(*b"ut16");
const TYPE_UTF8_TEXT: u32 = u32::from_be_bytes(*b"utf8");
const TYPE_CSTRING: u32 = u32::from_be_bytes(*b"cstr");

/// Run SCRIPT on the AppKit thread and translate its result to a Lisp value.
pub(super) fn execute(script: &str) -> Result<AppleScriptResult, String> {
    unsafe {
        let (done, result) = mpsc::sync_channel(1);
        let script = script.to_owned();
        let app: id = msg_send![class!(NSApplication), sharedApplication];
        let block = ConcreteBlock::new(move |_: id| {
            let result = execute_on_appkit_thread(&script);
            let _ = done.send(result);
            let _: () = msg_send![app, stop: nil];
        });
        let block = block.copy();
        let timer: id = msg_send![
            class!(NSTimer),
            scheduledTimerWithTimeInterval: 0.0f64
            repeats: NO
            block: block
        ];
        if timer == nil {
            return Err("Could not schedule AppleScript execution".to_owned());
        }
        let _: () = msg_send![app, run];
        result
            .recv()
            .map_err(|_| "AppleScript execution was interrupted".to_owned())?
    }
}

fn execute_on_appkit_thread(script: &str) -> Result<AppleScriptResult, String> {
    unsafe {
        let source = NSString::alloc(nil).init_str(script);
        let source: id = msg_send![source, autorelease];
        let script_object: id = msg_send![class!(NSAppleScript), alloc];
        let script_object: id = msg_send![script_object, initWithSource: source];
        if script_object == nil {
            return Err("Could not create NSAppleScript".to_owned());
        }

        let mut error_info: id = nil;
        let descriptor: id = msg_send![script_object, executeAndReturnError: &mut error_info];
        let _: () = msg_send![script_object, release];
        if descriptor == nil {
            return Err(error_message(error_info));
        }

        let descriptor_type: u32 = msg_send![descriptor, descriptorType];
        if descriptor_type == TYPE_NULL {
            return Ok(AppleScriptResult::Nil);
        }
        if matches!(
            descriptor_type,
            TYPE_UNICODE_TEXT | TYPE_UTF16_EXTERNAL | TYPE_UTF8_TEXT | TYPE_CSTRING
        ) {
            let text: id = msg_send![descriptor, coerceToDescriptorType: TYPE_UTF8_TEXT];
            return Ok(utf8_string(text)
                .map(AppleScriptResult::String)
                .unwrap_or(AppleScriptResult::Other));
        }

        let value: i32 = msg_send![descriptor, int32Value];
        Ok(AppleScriptResult::Integer(i128::from(value)))
    }
}

unsafe fn error_message(error_info: id) -> String {
    if error_info == nil {
        return "AppleScript compilation or execution failed".to_owned();
    }
    for key in ["NSAppleScriptErrorMessage", "NSAppleScriptErrorBriefMessage"] {
        let key = unsafe { NSString::alloc(nil).init_str(key) };
        let key: id = unsafe { msg_send![key, autorelease] };
        let value: id = unsafe { msg_send![error_info, objectForKey: key] };
        if let Some(message) = unsafe { utf8_string(value) } {
            return message;
        }
    }
    "AppleScript compilation or execution failed".to_owned()
}

unsafe fn utf8_string(value: id) -> Option<String> {
    if value == nil {
        return None;
    }
    let bytes: *const c_char = unsafe { msg_send![value, UTF8String] };
    if bytes.is_null() {
        return None;
    }
    Some(
        unsafe { CStr::from_ptr(bytes) }
            .to_string_lossy()
            .into_owned(),
    )
}
