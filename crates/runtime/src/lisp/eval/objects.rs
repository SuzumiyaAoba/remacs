//! Identity metadata for overlay handles, string representations, and string properties.

use super::Interp;
use crate::lisp::value::Value;
use std::rc::Rc;

impl Interp {
    pub(crate) fn register_overlay(&mut self, value: &Value) {
        if let Value::Vec(handle) = value {
            if self.overlay_handles.len() >= self.overlay_cleanup_at {
                self.overlay_handles
                    .retain(|_, weak| weak.strong_count() > 0);
                self.overlay_cleanup_at = self.overlay_handles.len().saturating_mul(2).max(256);
            }
            self.overlay_handles
                .insert(Rc::as_ptr(handle) as usize, Rc::downgrade(handle));
        }
    }

    pub(crate) fn is_overlay(&self, value: &Value) -> bool {
        let Value::Vec(handle) = value else {
            return false;
        };
        self.overlay_handles
            .get(&(Rc::as_ptr(handle) as usize))
            .and_then(|weak| weak.upgrade())
            .is_some_and(|registered| Rc::ptr_eq(&registered, handle))
    }

    /// Snapshot the editor data behind a genuine overlay handle. Never
    /// interpret an ordinary vector with overlay-looking slots as an overlay.
    pub(crate) fn overlay_state(&self, value: &Value) -> Option<crate::buffer::Overlay> {
        if !self.is_overlay(value) {
            return None;
        }
        // A restored image can reuse the old owner's numeric buffer id.
        // Inline detached state therefore takes precedence over live slots.
        if let Some(state) = crate::buffer::Overlay::detached_state(value) {
            return Some(state);
        }
        let Value::Vec(handle) = value else {
            return None;
        };
        let (buffer, slot) = {
            let handle = handle.borrow();
            (
                usize::try_from(handle.get(1)?.int()?).ok()?,
                usize::try_from(handle.get(2)?.int()?).ok()?,
            )
        };
        let buffer = self.buffers.get(buffer)?;
        let state = buffer.borrow().overlays.get(slot).cloned();
        state
    }

    /// GNU make_string: canonical internal UTF-8 selects multibyte;
    /// an invalid octet makes the complete byte sequence unibyte.
    pub(crate) fn auto_string_value(&mut self, bytes: &[u8]) -> Value {
        let (text, multibyte) = crate::lisp::value::auto_string_bytes(bytes);
        let value = Value::string_with_kind(text, multibyte);
        if let Value::Str(text) = &value {
            if multibyte {
                self.mark_multibyte(text);
            } else {
                self.mark_unibyte(text);
            }
        }
        value
    }

    /// Mark a string as unibyte (encoder output); `prin1' escapes
    /// its ≥0x80 byte-chars as `\NNN' octal like GNU.
    pub fn mark_unibyte(&mut self, s: &crate::lisp::value::StrRef) {
        if Value::empty_string_kind(s) == Some(true) {
            return;
        }
        // Unibyte high bytes have distinct byte8 character codes; genuine
        // Unicode private-use characters never serve as storage proxies.
        {
            let mut text = s.borrow_mut();
            if text.chars().any(|c| (0x80..=0xff).contains(&c.code())) {
                *text = text
                    .chars()
                    .map(|c| {
                        if (0x80..=0xff).contains(&c.code()) {
                            crate::lisp::text::EmacsChar::new(0x3fff00 + c.code()).unwrap()
                        } else {
                            c
                        }
                    })
                    .collect();
            }
        }
        self.unibyte_strings
            .insert(std::rc::Rc::as_ptr(s) as usize, std::rc::Rc::downgrade(s));
    }

    /// Is this string a marked unibyte string?  A stale map entry
    /// (dead Weak, address since reused) never reports true.
    pub fn is_unibyte_str(&self, s: &crate::lisp::value::StrRef) -> bool {
        if let Some(multibyte) = Value::empty_string_kind(s) {
            return !multibyte;
        }
        self.unibyte_strings
            .get(&(std::rc::Rc::as_ptr(s) as usize))
            .and_then(|w| w.upgrade())
            .is_some()
    }

    /// Mark a string as multibyte (decoder output).
    pub fn mark_multibyte(&mut self, s: &crate::lisp::value::StrRef) {
        if Value::empty_string_kind(s) == Some(false) {
            return;
        }
        self.multibyte_strings
            .insert(std::rc::Rc::as_ptr(s) as usize, std::rc::Rc::downgrade(s));
    }

    pub(crate) fn empty_string(&self, multibyte: bool) -> Value {
        if multibyte {
            self.zero_multibyte_string.clone()
        } else {
            self.zero_unibyte_string.clone()
        }
    }

    /// Is this string a marked multibyte string?
    pub fn is_multibyte_str(&self, s: &crate::lisp::value::StrRef) -> bool {
        if let Some(multibyte) = Value::empty_string_kind(s) {
            return multibyte;
        }
        self.multibyte_strings
            .get(&(std::rc::Rc::as_ptr(s) as usize))
            .and_then(|w| w.upgrade())
            .is_some()
    }

    pub(crate) fn note_string_data_change(&mut self) {
        self.string_data_tick = self.string_data_tick.wrapping_add(1);
    }

    /// Text-prop interval list for a string object (empty when none).
    pub fn str_props(&self, s: &crate::lisp::value::StrRef) -> &[(usize, usize, Vec<Value>)] {
        match self.string_props.get(&(std::rc::Rc::as_ptr(s) as usize)) {
            Some((w, v)) if w.upgrade().is_some() => v.as_slice(),
            _ => &[],
        }
    }

    /// Mutable text-prop interval list for a string (creates entry).
    pub fn str_props_mut(
        &mut self,
        s: &crate::lisp::value::StrRef,
    ) -> crate::lisp::memory::IntervalMut<'_> {
        self.string_props
            .entry(std::rc::Rc::as_ptr(s) as usize)
            .or_insert_with(|| (std::rc::Rc::downgrade(s), Default::default()))
            .1
            .edit()
    }

    /// Does a prop interval tree exist for this string?  GNU's
    /// `object-intervals' distinguishes a missing tree from an existing
    /// tree containing only nil plists, so presence must be tracked
    /// separately from the interval list.
    pub fn has_str_props(&self, s: &crate::lisp::value::StrRef) -> bool {
        self.string_props
            .get(&(std::rc::Rc::as_ptr(s) as usize))
            .map(|(w, _)| w.upgrade().is_some())
            .unwrap_or(false)
    }

    /// Drop the tree for a whole-string `set-text-properties' to nil.
    pub fn clear_str_props(&mut self, s: &crate::lisp::value::StrRef) {
        self.string_props.remove(&(std::rc::Rc::as_ptr(s) as usize));
    }

    /// Replace a string's prop intervals wholesale (copy ops).  An
    /// empty list still materializes the interval tree (GNU keeps a
    /// single nil-plist interval covering the string).
    pub fn set_str_props(
        &mut self,
        s: &crate::lisp::value::StrRef,
        v: Vec<(usize, usize, Vec<Value>)>,
    ) {
        self.string_props.insert(
            std::rc::Rc::as_ptr(s) as usize,
            (std::rc::Rc::downgrade(s), v.into()),
        );
    }
}
