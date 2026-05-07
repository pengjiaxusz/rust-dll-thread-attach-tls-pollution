# `std::thread::spawn` aborts after `thread::current()` is called during `DLL_THREAD_ATTACH` on Windows — is this intentional?

<!--
  Search keywords:
  DLL_THREAD_ATTACH, DllMain, thread::spawn, thread::current, rtabort,
  STATUS_STACK_BUFFER_OVERRUN, 0xC0000409, CURRENT thread handle,
  TLS pollution, cdylib, LoadLibrary, Windows, Vulkan layer,
  init_current, set_current, ReentrantLock, stdout, println crash
-->

I'm not sure if this is considered a bug or intentional behavior. The crash is
`rtabort!` in `set_current()`, which means the Rust runtime explicitly chose to
abort in this case. However, the trigger is extremely easy to hit accidentally
(even a `println!` during `DLL_THREAD_ATTACH`), and there's no documentation
warning about this, so I wanted to bring it to your attention.

## I tried this code

```rust
// cdylib loaded via LoadLibrary on Windows
use std::sync::OnceLock;
use windows_sys::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_THREAD_ATTACH};

static INIT: OnceLock<()> = OnceLock::new();

#[unsafe(no_mangle)]
extern "system" fn DllMain(
    _hinst: *mut core::ffi::c_void,
    reason: u32,
    _reserved: *mut core::ffi::c_void,
) -> i32 {
    match reason {
        DLL_PROCESS_ATTACH => {
            INIT.set(()).ok();
            std::thread::spawn(|| {
                println!("worker thread running");
            });
        }
        DLL_THREAD_ATTACH => {
            // even println! here calls thread::current() and pollutes TLS
            println!(
                "DLL_THREAD_ATTACH on thread {:?}",
                std::thread::current().id()
            );
        }
        _ => {}
    }
    1
}
```

## I expected to see this happen

The worker thread spawned during `DLL_PROCESS_ATTACH` should run successfully
regardless of what happens during `DLL_THREAD_ATTACH`.

## Instead, this happened

The process aborts with:

```
fatal runtime error: current thread handle already set during thread spawn, aborting
```

Exit code: `0xC0000409` (`STATUS_STACK_BUFFER_OVERRUN`)

## Root cause analysis

The call chain is:

```
DLL_THREAD_ATTACH:
  println!("...")
    → std::io::stdout()
      → ReentrantLock::try_lock()
        → std::thread::current()
          → init_current()              // src/std/thread/current.rs
            → CURRENT = Some(handle)    // ← TLS POLLUTED

... later, on the main thread ...

std::thread::spawn(|| { ... })
  → CreateThread → thread_start
    → init.init()
      → set_current(self.handle)
        → if CURRENT.get().is_some() {  // ← DETECTS POLLUTION
            rtabort!(
                "current thread handle already set during thread spawn, aborting"
            )
          }
```

The issue is in `std::sys::thread::current()` (specifically `init_current()`) —
it unconditionally sets `CURRENT` on the *calling* thread's TLS, regardless of
whether that thread is owned by the Rust runtime. When `DLL_THREAD_ATTACH` fires
on a *foreign* thread (e.g., Vulkan Loader's internal thread), Rust's std
pollutes `CURRENT` on that thread's TLS. Later, when `std::thread::spawn`
creates a new thread, `set_current()` in the *new* thread (not the foreign
thread) detects `CURRENT` is already `Some` and aborts.

Key observation: `println!`, `format!`, `eprintln!`, `dbg!`, and `tracing` all
eventually call `std::io::stdout()` / `stderr()` → `ReentrantLock::try_lock()`
→ `std::thread::current()`. So even seemingly harmless debug output triggers
this crash.

## Real-world trigger

This reliably occurs when a Rust `cdylib` is loaded as a **Vulkan Layer**,
because the Vulkan Loader creates internal threads and sends
`DLL_THREAD_ATTACH` notifications to all loaded layers.

## Minimal reproduction repository

Complete MRE with 7 test scenarios:

https://github.com/pengjiaxusz/rust-dll-thread-attach-tls-pollution

```powershell
git clone https://github.com/pengjiaxusz/rust-dll-thread-attach-tls-pollution.git
cd rust-dll-thread-attach-tls-pollution
.\run-all-tests.ps1
```

The repo includes isolated child-process testing (crashes don't affect the
runner) and covers these scenarios:

| # | Scenario | Result |
|---|----------|--------|
| 1 | `println!` in `DLL_THREAD_ATTACH` | crash |
| 2 | `format!` + `println!` | crash |
| 3 | `eprintln!` + `dbg!` | crash |
| 4 | `OutputDebugStringW` (no Rust std) | safe |
| 5 | `CreateThread` bypasses check | safe |
| 6 | Early return guard (zero Rust code) | safe |
| 7 | `tracing::non_blocking` internal spawn | crash |

## Possible approaches

1. Make `init_current()` detect foreign threads — check if the current thread is
   already known to the Rust runtime before setting `CURRENT`. If it's a foreign
   thread, don't pollute TLS.

2. Document the hazard prominently — add a warning to Windows-specific platform
   docs and the `DllMain` documentation that any Rust std code during
   `DLL_THREAD_ATTACH` causes undefined behavior.

## Workaround

Add an early return guard at the very top of `DllMain`:

```rust
if reason == DLL_THREAD_ATTACH {
    return 1; // no Rust code executes on this path
}
```

## Meta

`rustc --version --verbose`:

```
rustc 1.96.0-nightly (fda6d37bb 2026-03-27)
binary: rustc
commit-hash: fda6d37bbbf72d5b99e3b43d1a8fd2dd6b0ff6bc
commit-date: 2026-03-27
host: x86_64-pc-windows-msvc
release: 1.96.0-nightly
LLVM version: 20.1.8
```

Tested on Windows 11 only. This is a Windows-specific issue since
`DllMain` / `DLL_THREAD_ATTACH` is a Windows concept.
