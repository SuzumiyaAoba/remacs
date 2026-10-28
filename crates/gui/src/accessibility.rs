//! Native macOS accessibility text surface for the canvas-rendered editor.

use cocoa::{
    appkit::NSApp,
    base::{NO, YES, id, nil},
    foundation::{NSPoint, NSRange, NSRect, NSSize, NSString},
};
use objc::{class, msg_send, sel, sel_impl};
use std::ops::Range;

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    fn NSAccessibilityPostNotification(element: id, notification: id);
    static NSAccessibilityLayoutChangedNotification: id;
    static NSAccessibilityValueChangedNotification: id;
    static NSAccessibilitySelectedTextChangedNotification: id;
}

/// Add a non-rendering NSTextView to the native window's accessibility
/// children, then keep its text and caret synchronized with the visible grid.
pub(super) fn update_text_view(
    existing: Option<usize>,
    text: &str,
    selection: Range<usize>,
    label_text: &str,
    help_text: &str,
) -> Option<usize> {
    unsafe {
        let app = NSApp();
        if app == nil {
            return existing;
        }
        let key_window: id = msg_send![app, keyWindow];
        let window: id = if key_window == nil {
            msg_send![app, mainWindow]
        } else {
            key_window
        };
        if window == nil {
            return existing;
        }
        let content: id = msg_send![window, contentView];
        if content == nil {
            return existing;
        }

        let view = match existing {
            Some(pointer) => pointer as id,
            None => {
                let bounds: NSRect = msg_send![content, bounds];
                let view_frame = NSRect::new(
                    NSPoint::new(0.0, 0.0),
                    NSSize::new(bounds.size.width, bounds.size.height),
                );
                let allocated: id = msg_send![class!(NSTextView), alloc];
                let view: id = msg_send![allocated, initWithFrame: view_frame];
                if view == nil {
                    return None;
                }
                let _: () = msg_send![view, setEditable: NO];
                let _: () = msg_send![view, setSelectable: YES];
                let _: () = msg_send![view, setDrawsBackground: NO];
                let _: () = msg_send![view, setAllowsUndo: NO];
                let _: () = msg_send![view, setAccessibilityElement: YES];

                let label = NSString::alloc(nil).init_str(label_text);
                let _: () = msg_send![view, setAccessibilityLabel: label];
                let _: () = msg_send![label, release];

                let help = NSString::alloc(nil).init_str(help_text);
                let _: () = msg_send![view, setAccessibilityHelp: help];
                let _: () = msg_send![help, release];

                let _: () = msg_send![view, setAccessibilityParent: content];
                let _: () = msg_send![view, setAccessibilityWindow: window];
                let _: () = msg_send![view, setAccessibilityTopLevelUIElement: window];
                let screen_frame: NSRect = msg_send![content, convertRectToScreen: bounds];
                let _: () = msg_send![view, setAccessibilityFrame: screen_frame];

                let old_children: id = msg_send![content, accessibilityChildren];
                let children: id = msg_send![class!(NSMutableArray), array];
                if old_children != nil {
                    let _: () = msg_send![children, addObjectsFromArray: old_children];
                }
                let _: () = msg_send![children, addObject: view];
                let _: () = msg_send![content, setAccessibilityChildren: children];
                let _: () = msg_send![view, autorelease];
                NSAccessibilityPostNotification(content, NSAccessibilityLayoutChangedNotification);
                view
            }
        };

        let value = NSString::alloc(nil).init_str(text);
        let _: () = msg_send![view, setString: value];
        let _: () = msg_send![value, release];

        let text_length = text.encode_utf16().count();
        let start = selection.start.min(text_length);
        let end = selection.end.min(text_length).max(start);
        let _: () =
            msg_send![view, setSelectedRange: NSRange::new(start as u64, (end - start) as u64)];
        NSAccessibilityPostNotification(view, NSAccessibilityValueChangedNotification);
        NSAccessibilityPostNotification(view, NSAccessibilitySelectedTextChangedNotification);
        Some(view as usize)
    }
}
