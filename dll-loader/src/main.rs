// DLL Loader — loads a test DLL in a child process to isolate crashes
// Usage: dll-loader.exe <dll-path>
// Exit codes: 0 = success, 0xc0000409 = TLS pollution crash, other = error

use std::ffi::c_void;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryW(lpLibFileName: *const u16) -> *mut c_void;
    fn FreeLibrary(hLibModule: *mut c_void) -> i32;
}

fn to_utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn main() {
    let dll_path = std::env::args()
        .nth(1)
        .expect("Usage: dll-loader.exe <dll-path>");

    let abs_path = std::path::PathBuf::from(&dll_path)
        .canonicalize()
        .unwrap_or_else(|_| std::path::PathBuf::from(&dll_path));

    let wide_path = to_utf16(&abs_path.to_string_lossy());

    unsafe {
        let handle = LoadLibraryW(wide_path.as_ptr());
        if handle.is_null() {
            let err = std::io::Error::last_os_error();
            eprintln!("LoadLibraryW failed! Error: {err}");
            std::process::exit(1);
        }

        // Give spawned threads time to execute (if the DLL hasn't already crashed)
        std::thread::sleep(std::time::Duration::from_millis(300));

        let _ = FreeLibrary(handle);
    }

    // If we reach here, everything is fine — no TLS crash
    std::process::exit(0);
}
