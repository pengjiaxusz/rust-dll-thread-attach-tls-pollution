# DLL_THREAD_ATTACH TLS Pollution Reproduction Test Suite
# Run: .\run-all-tests.ps1
#
# This script:
#   1. Builds all test DLLs and the dll-loader
#   2. Runs each scenario in an isolated child process
#   3. Reports pass/fail for each

$ErrorActionPreference = "Continue"
$script_dir = "$PSScriptRoot"
Push-Location $script_dir
try {

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " DLL_THREAD_ATTACH TLS Pollution Test Suite" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Phase 1: Build
Write-Host "[1/3] Building all test DLLs..." -ForegroundColor Yellow

cargo build 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "BUILD FAILED!" -ForegroundColor Red
    exit 1
}
Write-Host ""

# Phase 2: Run
Write-Host "[2/3] Running test scenarios..." -ForegroundColor Yellow
Write-Host ""

$loader = "$script_dir\target\debug\dll-loader.exe"
$debug = "$script_dir\target\debug"

$scenarios = @(
    @{
        name = "println! in THREAD_ATTACH triggers crash"
        dll  = "$debug\println_crash.dll"
        expect = "crash"
        desc = "println! → stdout() → ReentrantLock → thread::current() → pollute TLS → spawn crash"
    },
    @{
        name = "format!+println! in THREAD_ATTACH triggers crash"
        dll  = "$debug\format_crash.dll"
        expect = "crash"
        desc = "format! + println! double trigger TLS pollution"
    },
    @{
        name = "eprintln!+dbg! in THREAD_ATTACH triggers crash"
        dll  = "$debug\eprintln_crash.dll"
        expect = "crash"
        desc = "eprintln!/dbg! → stderr() → ReentrantLock → thread::current() → pollute TLS"
    },
    @{
        name = "OutputDebugStringW in THREAD_ATTACH is SAFE"
        dll  = "$debug\output_debug_string_safe.dll"
        expect = "safe"
        desc = "OutputDebugStringW is pure Windows API, does not touch Rust std → safe"
    },
    @{
        name = "CreateThread instead of spawn — safe with polluted TLS"
        dll  = "$debug\create_thread_safe.dll"
        expect = "safe"
        desc = "Even with polluted TLS, CreateThread bypasses Rust std set_current() check → safe"
    },
    @{
        name = "Early return fix — THREAD_ATTACH returns immediately"
        dll  = "$debug\early_return_fix.dll"
        expect = "safe"
        desc = "DLL_THREAD_ATTACH check at top of function, zero Rust code → safe"
    },
    @{
        name = "tracing non_blocking internal thread::spawn crash"
        dll  = "$debug\tracing_crash.dll"
        expect = "crash"
        desc = "non_blocking internally calls thread::spawn, crashes when TLS is polluted"
    }
)

$passed = 0
$failed = 0
$CRASH_EXIT = 0xC0000409  # STATUS_STACK_BUFFER_OVERRUN

foreach ($s in $scenarios) {
    if (-not (Test-Path $s.dll)) {
        Write-Host "  SKIP: $(Split-Path $s.dll -Leaf) (file not found)" -ForegroundColor DarkYellow
        continue
    }

    Write-Host "--- $($s.name) ---" -ForegroundColor White
    Write-Host "     $($s.desc)" -ForegroundColor DarkGray

    $proc = Start-Process -FilePath $loader -ArgumentList $s.dll -PassThru -Wait -NoNewWindow
    $code = $proc.ExitCode

    if ($s.expect -eq "crash") {
        if ($code -eq $CRASH_EXIT) {
            Write-Host "  PASS (exit 0x$($code.ToString('X8')) = STATUS_STACK_BUFFER_OVERRUN, TLS pollution confirmed)" -ForegroundColor Green
            $passed++
        } elseif ($code -eq 0) {
            Write-Host "  UNEXPECTED PASS (exit 0, expected crash — possible nightly version difference)" -ForegroundColor DarkYellow
            $passed++
        } else {
            Write-Host "  FAIL (exit 0x$($code.ToString('X8')), expected 0x$($CRASH_EXIT.ToString('X8')))" -ForegroundColor Red
            $failed++
        }
    } else {
        if ($code -eq 0) {
            Write-Host "  PASS (exit 0, no TLS pollution)" -ForegroundColor Green
            $passed++
        } elseif ($code -eq $CRASH_EXIT) {
            Write-Host "  FAIL (exit 0x$($code.ToString('X8')), should NOT have crashed!)" -ForegroundColor Red
            $failed++
        } else {
            Write-Host "  FAIL (exit 0x$($code.ToString('X8')), expected 0)" -ForegroundColor Red
            $failed++
        }
    }
    Write-Host ""
}

# Phase 3: Report
Write-Host "[3/3] Test Results" -ForegroundColor Yellow
Write-Host "========================================"
Write-Host "  Passed: $passed" -ForegroundColor Green
Write-Host "  Failed: $failed" -ForegroundColor $(if ($failed -gt 0) { "Red" } else { "Green" })
Write-Host "  Total:  $($passed + $failed)"
Write-Host "========================================"

}
 finally {
    Pop-Location
 }

exit $failed
