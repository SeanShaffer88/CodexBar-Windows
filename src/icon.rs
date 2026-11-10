//! The application icon (resource id 1, embedded by build.rs from
//! assets/app.ico), shared by the visible windows.

use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{LoadIconW, HICON};

/// Load the embedded application icon. Returns null when the resource
/// is missing (a build without windres/rc); the windows then fall
/// back to the system default icon.
pub fn app_icon() -> HICON {
    // Resource id 1 passed as a MAKEINTRESOURCE-style integer pointer.
    let id = std::ptr::without_provenance::<u16>(1);
    unsafe { LoadIconW(GetModuleHandleW(std::ptr::null()), id) }
}
