//! Stub implementations of libc functions that pdfium's statically-linked
//! wasi-libc expects at runtime under the "env" module namespace.
//!
//! WASI preview1 syscalls (wasi_snapshot_preview1::*) cannot be stubbed from
//! Rust because they live in a different WASM import module namespace. Those
//! are provided in JavaScript — see packages/wasm/scripts/patch-wasi-imports.js.

// ---------------------------------------------------------------------------
// env:: stubs (libc / pthreads)
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn getpid() -> i32 {
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn pthread_mutex_init(_mutex: *mut u8, _attr: *const u8) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn pthread_mutex_lock(_mutex: *mut u8) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn pthread_mutex_unlock(_mutex: *mut u8) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn pthread_mutex_destroy(_mutex: *mut u8) -> i32 {
    0
}

// ---------------------------------------------------------------------------
// setjmp / longjmp support (new WASM exception handling SjLj model)
//
// Chromium's clang uses the new WASM SjLj lowering where:
//   - __c_longjmp is a WebAssembly.Tag with 1 param (env: i32)
//   - __wasm_setjmp(env, label, table) — 3 params
//   - __wasm_setjmp_test(table, env) -> label — 2 params
//
// The table is a flat array of (env, label) pairs on the stack, zero-terminated.
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn __wasm_setjmp(env: u32, label: u32, table: *mut u32) {
    unsafe {
        let mut i = 0;
        loop {
            if *table.add(i) == 0 {
                *table.add(i) = env;
                *table.add(i + 1) = label;
                *table.add(i + 2) = 0; // sentinel
                return;
            }
            i += 2;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __wasm_setjmp_test(table: *const u32, env: u32) -> u32 {
    unsafe {
        let mut i = 0;
        loop {
            let stored_env = *table.add(i);
            if stored_env == 0 {
                return 0;
            }
            if stored_env == env {
                return *table.add(i + 1);
            }
            i += 2;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __wasm_longjmp(_env: *mut u8, _val: i32) {
    // longjmp throws the __c_longjmp WASM exception tag, which we can't do
    // from Rust. Trap instead — in practice, FreeType's setjmp error recovery
    // rarely triggers during normal PDF parsing.
    core::arch::wasm32::unreachable();
}
