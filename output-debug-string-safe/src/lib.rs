// Scenario 4: OutputDebugStringW in DLL_THREAD_ATTACH is SAFE
// OutputDebugStringW is a pure Windows API — never touches Rust std → no TLS pollution
// Expected result: exit code 0

use std::ffi::c_void;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn OutputDebugStringW(lpOutputString: *const u16);
}

fn safe_print(msg: &str) {
    let wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { OutputDebugStringW(wide.as_ptr()) };
}

const DLL_PROCESS_ATTACH: u32 = 1;
const DLL_THREAD_ATTACH: u32 = 2;

#[unsafe(no_mangle)]
extern "system" fn DllMain(_hinst: *mut c_void, reason: u32, _reserved: *mut c_void) -> i32 {
    match reason {
        DLL_THREAD_ATTACH => {
            // OutputDebugStringW does NOT call any Rust std code → TLS stays clean
            safe_print("[output-debug-string-safe] DLL_THREAD_ATTACH: safe via OutputDebugStringW");
        }
        DLL_PROCESS_ATTACH => {
            // TLS is clean → std::thread::spawn works normally
            safe_print("[output-debug-string-safe] DLL_PROCESS_ATTACH: thread::spawn OK");
            std::thread::spawn(|| {
                safe_print("[output-debug-string-safe] spawned thread running...");
                std::thread::sleep(std::time::Duration::from_millis(100));
                safe_print("[output-debug-string-safe] spawned thread done");
            });
        }
        _ => {}
    }
    1
}
