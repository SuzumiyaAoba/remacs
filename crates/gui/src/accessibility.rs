//! Native macOS accessibility text surface for the canvas-rendered editor.

use cocoa::{
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
    target_window: Option<usize>,
    text: &str,
    selection: Range<usize>,
    label_text: &str,
    help_text: &str,
) -> Option<usize> {
    let window = target_window? as id;
    unsafe {
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
                let _: () = msg_send![view, autorelease];
                view
            }
        };

        let existing_window: id = msg_send![view, accessibilityWindow];
        if existing_window != window {
            if existing_window != nil {
                let existing_content: id = msg_send![existing_window, contentView];
                if existing_content != nil {
                    let existing_children: id =
                        msg_send![existing_content, accessibilityChildren];
                    if existing_children != nil {
                        let children: id = msg_send![class!(NSMutableArray), array];
                        let count: usize = msg_send![existing_children, count];
                        for index in 0..count {
                            let child: id = msg_send![existing_children, objectAtIndex: index];
                            if child != view {
                                let _: () = msg_send![children, addObject: child];
                            }
                        }
                        let _: () = msg_send![existing_content, setAccessibilityChildren: children];
                    }
                    NSAccessibilityPostNotification(
                        existing_content,
                        NSAccessibilityLayoutChangedNotification,
                    );
                }
            }

            let _: () = msg_send![view, setAccessibilityParent: content];
            let _: () = msg_send![view, setAccessibilityWindow: window];
            let _: () = msg_send![view, setAccessibilityTopLevelUIElement: window];
            let bounds: NSRect = msg_send![content, bounds];
            let screen_frame: NSRect = msg_send![content, convertRectToScreen: bounds];
            let _: () = msg_send![view, setAccessibilityFrame: screen_frame];

            let old_children: id = msg_send![content, accessibilityChildren];
            let children: id = msg_send![class!(NSMutableArray), array];
            if old_children != nil {
                let _: () = msg_send![children, addObjectsFromArray: old_children];
            }
            let _: () = msg_send![children, addObject: view];
            let _: () = msg_send![content, setAccessibilityChildren: children];
            NSAccessibilityPostNotification(content, NSAccessibilityLayoutChangedNotification);
        }

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

/// Refresh the screen-space frame after the native window moves or resizes.
pub(super) fn update_text_view_frame(existing: Option<usize>, target_window: Option<usize>) {
    let Some(pointer) = existing else {
        return;
    };
    let Some(window) = target_window.map(|window| window as id) else {
        return;
    };
    unsafe {
        let view = pointer as id;
        if window == nil {
            return;
        }
        let content: id = msg_send![window, contentView];
        if content == nil {
            return;
        }
        let bounds: NSRect = msg_send![content, bounds];
        let _: () = msg_send![view, setFrame: bounds];
        let screen_frame: NSRect = msg_send![content, convertRectToScreen: bounds];
        let _: () = msg_send![view, setAccessibilityFrame: screen_frame];
        NSAccessibilityPostNotification(view, NSAccessibilityLayoutChangedNotification);
    }
}

