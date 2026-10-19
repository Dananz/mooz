//! Platform-independent logic. Nothing here touches the OS, so it is unit
//! tested natively on any machine (`cargo test`).

pub mod applist;
pub mod config;
pub mod gesture;
pub mod modifiers;
pub mod pinch;
pub mod wheel;

pub mod shortcut;
