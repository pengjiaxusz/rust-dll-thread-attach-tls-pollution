// Scenario 5: CreateThread instead of std::thread::spawn — safe even with polluted TLS
// CreateThread creates a raw Windows thread that bypasses Rust std set_current() check
// Expected result: exit code 0

use std::ffi::c_void;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateThread(
        lpThreadAttributes: *mut c_void,
        dwStackSize: usize,
        lpStartAddress: Option<unsafe extern "system" fn(*mut c_void) -> u32>,
        lpParameter: *mut c_void,
        dwCreationFlags: u32,
        lpThreadId: *mut u32,
    ) -> *mut c_void;
    fn OutputDebugStringW(lpOutputString: *const u16);
}

fn safe_print(msg: &str) {
    let wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { OutputDebugStringW(wide.as_ptr()) };
}

unsafe extern "system" fn worker_thread(_param: *mut c_void) -> u32 {
    safe_print("[create-thread-safe] CreateThread worker running...");
    std::thread::sleep(std::time::Duration::from_millis(100));
    safe_print("[create-thread-safe] CreateThread worker done");
    0
}

const DLL_PROCESS_ATTACH: u32 = 1;
const DLL_THREAD_ATTACH: u32 = 2;

#[unsafe(no_mangle)]
extern "system" fn DllMain(_hinst: *mut c_void, reason: u32, _reserved: *mut c_void) -> i32 {
    match reason {
        DLL_THREAD_ATTACH => {
            // Deliberately pollute TLS via println!
            println!("[create-thread-safe] DLL_THREAD_ATTACH: polluting TLS via println!");
        }
        DLL_PROCESS_ATTACH => {
            // Use CreateThread instead of std::thread::spawn — bypasses set_current() check
            safe_print("[create-thread-safe] DLL_PROCESS_ATTACH: using CreateThread");
            unsafe {
                CreateThread(
                    std::ptr::null_mut(),
                    0,
                    Some(worker_thread),
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                );
            }
            // Give CreateThread time to run
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        _ => {}
    }
    1
}
