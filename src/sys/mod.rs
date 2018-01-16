#[cfg(all(unix, not(target_os = "fuchsia")))]
mod unix;

#[cfg(all(unix, not(target_os = "fuchsia")))]
pub use self::unix::*;

#[cfg(target_os = "fuchsia")]
mod fuchsia;

#[cfg(target_os = "fuchsia")]
pub use self::fuchsia::*;

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use self::windows::*;
