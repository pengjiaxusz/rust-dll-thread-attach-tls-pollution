// Scenario 6: Early return fix — DLL_THREAD_ATTACH returns TRUE immediately, zero Rust code
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
    // KEY: THREAD_ATTACH check MUST be at the very top, before any other Rust code!
    if reason == DLL_THREAD_ATTACH {
        return 1; // immediate return, zero Rust code execution
    }

    match reason {
        DLL_PROCESS_ATTACH => {
            // TLS is clean because THREAD_ATTACH executed zero Rust code
            safe_print("[early-return-fix] DLL_PROCESS_ATTACH: TLS clean, thread::spawn OK");
            std::thread::spawn(|| {
                safe_print("[early-return-fix] spawned thread running...");
                std::thread::sleep(std::time::Duration::from_millis(100));
                safe_print("[early-return-fix] spawned thread done");
            });
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        _ => {}
    }
    1
}
