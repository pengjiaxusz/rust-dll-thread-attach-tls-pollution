// Scenario 3: eprintln! + dbg! in DLL_THREAD_ATTACH trigger TLS pollution → crash
// Expected result: exit code 0xc0000409 (STATUS_STACK_BUFFER_OVERRUN)

use std::ffi::c_void;

const DLL_PROCESS_ATTACH: u32 = 1;
const DLL_THREAD_ATTACH: u32 = 2;

#[unsafe(no_mangle)]
extern "system" fn DllMain(_hinst: *mut c_void, reason: u32, _reserved: *mut c_void) -> i32 {
    match reason {
        DLL_THREAD_ATTACH => {
            // eprintln! goes through stderr() → ReentrantLock → thread::current()
            // → init_current() → sets CURRENT TLS ← TLS polluted!
            eprintln!("[eprintln-crash] DLL_THREAD_ATTACH: polluting TLS via eprintln!");

            // dbg! also uses stderr → same call chain
            dbg!("[eprintln-crash] DLL_THREAD_ATTACH: dbg! also triggers");
        }
        DLL_PROCESS_ATTACH => {
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_millis(100));
            });
        }
        _ => {}
    }
    1
}
