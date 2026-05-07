// Scenario 2: format! + println! in DLL_THREAD_ATTACH triggers TLS pollution → crash
// Expected result: exit code 0xc0000409 (STATUS_STACK_BUFFER_OVERRUN)

use std::ffi::c_void;

const DLL_PROCESS_ATTACH: u32 = 1;
const DLL_THREAD_ATTACH: u32 = 2;

#[unsafe(no_mangle)]
extern "system" fn DllMain(_hinst: *mut c_void, reason: u32, _reserved: *mut c_void) -> i32 {
    match reason {
        DLL_THREAD_ATTACH => {
            // format! allocates String; some allocator paths may call thread::current()
            // followed by println! which definitely calls thread::current()
            let msg = format!(
                "[format-crash] DLL_THREAD_ATTACH: format! may pollute TLS, reason={}",
                reason
            );
            println!("{}", msg);
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
