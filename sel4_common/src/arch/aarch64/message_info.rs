include!(concat!(env!("OUT_DIR"), "/message_label.rs"));
#[cfg(not(feature = "kernel_mcs"))]
pub const CNODE_LAST_INVOCATION: usize = MessageLabel::CNodeSaveCaller as usize;
#[cfg(feature = "kernel_mcs")]
pub const CNODE_LAST_INVOCATION: usize = MessageLabel::CNodeRotate as usize;
