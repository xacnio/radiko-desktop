//! Platform-specific integrations.
//!
//! Groups OS-specific modules (splash screen, taskbar thumbnails, shortcuts)
//! that would otherwise clutter the crate root.

pub mod splash;
pub mod shortcut;

#[cfg(target_os = "windows")]
pub mod thumbbar;
