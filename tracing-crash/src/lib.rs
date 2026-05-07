// Scenario 7: tracing_appender::non_blocking internally calls std::thread::spawn
// When TLS is already polluted from DLL_THREAD_ATTACH, the internal spawn crashes
// Expected result: exit code 0xc0000409 (STATUS_STACK_BUFFER_OVERRUN)

use std::ffi::c_void;
use std::sync::OnceLock;

static TRACING_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

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
            // Pollute TLS via println! first
            println!("[tracing-crash] DLL_THREAD_ATTACH: polluting TLS via println!");
        }
        DLL_PROCESS_ATTACH => {
            safe_print("[tracing-crash] DLL_PROCESS_ATTACH: initializing tracing non_blocking");

            // non_blocking internally calls std::thread::spawn to create a writer thread
            // Since TLS was polluted during THREAD_ATTACH, this will crash!
            let (non_blocking_writer, guard) = tracing_appender::non_blocking(std::io::stdout());
            let _ = TRACING_GUARD.set(guard);

            let _subscriber = tracing_subscriber::fmt()
                .with_writer(non_blocking_writer)
                .with_ansi(false)
                .with_thread_ids(true)
                .finish();

            safe_print("[tracing-crash] if you see this line, no crash happened (unlikely)");
        }
        _ => {}
    }
    1
}
