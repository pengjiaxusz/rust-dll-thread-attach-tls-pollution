// Scenario 1: println! in DLL_THREAD_ATTACH triggers TLS pollution → crash
// Expected result: exit code 0xc0000409 (STATUS_STACK_BUFFER_OVERRUN)

use std::ffi::c_void;

const DLL_PROCESS_ATTACH: u32 = 1;
const DLL_THREAD_ATTACH: u32 = 2;

#[unsafe(no_mangle)]
extern "system" fn DllMain(_hinst: *mut c_void, reason: u32, _reserved: *mut c_void) -> i32 {
    match reason {
        DLL_THREAD_ATTACH => {
            // println! internally goes through stdout() → ReentrantLock → thread::current()
            // → init_current() → sets CURRENT TLS on the foreign thread ← TLS polluted!
            println!("[println-crash] DLL_THREAD_ATTACH: polluting TLS via println!");
        }
        DLL_PROCESS_ATTACH => {
            // TLS was polluted above. std::thread::spawn now detects CURRENT is already set
            // → rtabort!("current thread handle already set during thread spawn")
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_millis(100));
            });
        }
        _ => {}
    }
    1
}
