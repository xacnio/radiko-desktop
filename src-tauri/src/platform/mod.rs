//! Platform-specific integrations.
//!
//! Groups OS-specific modules (splash screen, taskbar thumbnails, shortcuts)
//! that would otherwise clutter the crate root.

pub mod shortcut;
pub mod splash;

#[cfg(target_os = "windows")]
pub mod thumbbar;

#[cfg(target_os = "windows")]
pub mod mouse_hook;

#[cfg(target_os = "macos")]
pub mod macos;
