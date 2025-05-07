//! This file contains the implementation of the `seL4_MessageInfo_t` struct and its associated methods.
//!
//! The `seL4_MessageInfo_t` struct represents a message info in the seL4 microkernel. It provides methods for creating, converting, and accessing the fields of a message info.
//!
//! # Examples
//!
//! Creating a new `seL4_MessageInfo_t` from a word:
//!
//! ```
//! let word = 0x12345678;
//! let message_info = seL4_MessageInfo_t::from_word(word);
//! ```
//!
//! Getting the label of the message:
//!
//! ```
//! let label = message_info.get_label();
//! ```

use super::sel4_config::SEL4_MSG_MAX_LENGTH;

use crate::{
    arch::MessageLabel, sel4_bitfield_types::Bitfield, shared_types_bf_gen::seL4_MessageInfo,
};

pub trait seL4_MessageInfo_func {
    fn from_word(w: usize) -> Self;
    fn from_word_security(w: usize) -> Self;
    fn to_word(&self) -> usize;
    fn get_message_label(&self) -> MessageLabel;
}

impl seL4_MessageInfo_func for seL4_MessageInfo {
    /// Creates a new `seL4_MessageInfo_t` from a word.
    #[inline]
    fn from_word(w: usize) -> Self {
        Self {
            0: Bitfield { arr: [w as u64] },
        }
    }

    /// Creates a new `seL4_MessageInfo_t` from a word with security checks.
    #[inline]
    fn from_word_security(w: usize) -> Self {
        let mut mi = Self::from_word(w);
        if mi.get_length() > SEL4_MSG_MAX_LENGTH as u64 {
            mi.set_length(SEL4_MSG_MAX_LENGTH as u64);
        }
        mi
    }

    /// Converts the `seL4_MessageInfo_t` to a word.
    #[inline]
    fn to_word(&self) -> usize {
        self.0.arr[0] as usize
    }

    /// Gets the label of the message.
    #[inline]
    fn get_message_label(&self) -> MessageLabel {
        unsafe { core::mem::transmute::<u32, MessageLabel>(self.get_label() as u32) }
    }
}
