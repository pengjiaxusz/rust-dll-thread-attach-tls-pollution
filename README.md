This is a minimal reproducible repository for [rust-lang/rust#156277](https://github.com/rust-lang/rust/issues/156277).

# DLL_THREAD_ATTACH TLS Pollution: MRE Repository

Minimal reproducible examples demonstrating a critical crash when Rust code executes
during `DLL_THREAD_ATTACH` on Windows, causing subsequent `std::thread::spawn` to abort.

## The Problem

When a Rust `cdylib` receives `DLL_THREAD_ATTACH` on a *foreign thread* (one not created
by Rust's std), any call to `thread::current()` — even indirectly through `println!`,
`format!`, or `tracing` — pollutes the thread's `CURRENT` TLS slot. Later, when
`std::thread::spawn` creates a new thread, the runtime detects `CURRENT` is already set
and aborts:

```
fatal runtime error: current thread handle already set during thread spawn, aborting
exit code: 0xc0000409 (STATUS_STACK_BUFFER_OVERRUN)
```

### The Call Chain

```
DLL_THREAD_ATTACH:
  println!("...")
    → std::io::stdout()
      → ReentrantLock::try_lock()
        → std::thread::current()
          → init_current()              // Rust std: src/std/thread/current.rs
            → CURRENT.set(thread_ptr)   // ← TLS POLLUTED!

... later ...

std::thread::spawn(|| { ... })
  → CreateThread → thread_start
    → init.init()
      → set_current(self.handle)
        → CURRENT.get() != NONE?        // ← DETECTS POLLUTION
        → rtabort!("current thread handle already set during thread spawn")
```

### Real-World Trigger

This occurs naturally when a Rust `cdylib` is loaded as a **Vulkan Layer**, because the
Vulkan Loader creates internal threads and sends `DLL_THREAD_ATTACH` to all loaded
layers during negotiation.

## Repository Structure

```
├── Cargo.toml                  # Workspace root
├── README.md
├── run-all-tests.ps1           # One-click test runner (Windows PowerShell)
│
├── println-crash/              # Scenario 1: println! → crash
├── format-crash/               # Scenario 2: format!+println! → crash
├── eprintln-crash/             # Scenario 3: eprintln!+dbg! → crash
├── output-debug-string-safe/   # Scenario 4: OutputDebugStringW → SAFE
├── create-thread-safe/         # Scenario 5: CreateThread → SAFE
├── early-return-fix/           # Scenario 6: early return guard → SAFE
├── tracing-crash/              # Scenario 7: tracing non_blocking → crash
│
└── dll-loader/                 # Child-process DLL loader (isolates crashes)
```

## Quick Start

### Prerequisites

- **Windows** only (DLL/DllMain is a Windows concept)
- **Rust nightly** — tested with `nightly-2026-05-06` (rustc 1.97.0); the `rust-toolchain.toml` auto-selects nightly, any recent nightly should work
- Uses `edition = "2024"`

### One-Click Reproduction

```powershell
.\run-all-tests.ps1
```

This builds all test DLLs and runs the full 7-scenario suite. Each test DLL is loaded
in an isolated child process, so crashes do not affect the test runner.

Expected output:

```
========================================
 DLL_THREAD_ATTACH TLS Pollution Test Suite
========================================

[1/3] Building all test DLLs...

[2/3] Running test scenarios...

--- println! in THREAD_ATTACH triggers crash ---
     println! → stdout() → ReentrantLock → thread::current() → pollute TLS → spawn crash
  PASS (exit 0xC0000409 = STATUS_STACK_BUFFER_OVERRUN, TLS pollution confirmed)

--- OutputDebugStringW in THREAD_ATTACH is SAFE ---
     OutputDebugStringW is pure Windows API, does not touch Rust std → safe
  PASS (exit 0, no TLS pollution)

...

[3/3] Test Results
========================================
  Passed: 7
  Failed: 0
  Total:  7
========================================
```

### Manual Build & Test

```powershell
# Build everything
cargo build

# Test a single scenario
.\target\debug\dll-loader.exe .\target\debug\println_crash.dll
# Exit 0xC0000409 = crash confirmed
```

## The Fix

Add an **early return guard** at the **very top** of `DllMain`, before any Rust code:

```rust
#[unsafe(no_mangle)]
extern "system" fn DllMain(_hinst: *mut c_void, reason: u32, _reserved: *mut c_void) -> i32 {
    // MUST be the FIRST executable lines in the function body
    if reason == DLL_THREAD_ATTACH {
        return 1; // zero Rust code execution
    }
    if reason == DLL_PROCESS_DETACH {
        return 1;
    }

    // ... rest of initialization (only runs on DLL_PROCESS_ATTACH)
    1
}
```

See [early-return-fix/src/lib.rs](early-return-fix/src/lib.rs) for a complete example.

## Scenarios Summary

| # | Scenario | Result |
|---|----------|--------|
| 1 | `println!` in THREAD_ATTACH | 💥 crash |
| 2 | `format!` + `println!` in THREAD_ATTACH | 💥 crash |
| 3 | `eprintln!` + `dbg!` in THREAD_ATTACH | 💥 crash |
| 4 | `OutputDebugStringW` in THREAD_ATTACH | ✅ safe |
| 5 | `CreateThread` with polluted TLS | ✅ safe |
| 6 | Early return guard | ✅ safe |
| 7 | `tracing::non_blocking` | 💥 crash |

## License

MIT OR Apache-2.0


---

**Note to reviewers:** This issue was drafted with the help of an AI assistant. While I've done my best to verify the core problem myself (the crash is reproducible and the test cases in the linked repo all produce consistent results), the root cause analysis and technical explanations in this post may contain inaccuracies or oversights that I'm not qualified to catch. Please take the diagnosis with a grain of salt, and I'd greatly appreciate any corrections where my understanding falls short.