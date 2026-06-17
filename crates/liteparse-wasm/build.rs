use std::env;

fn main() {
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    // pdfium's wasm build links libsetjmp.a for __c_longjmp, but that archive's
    // single object also defines the __wasm_setjmp/__wasm_longjmp helpers that
    // Rust's own codegen already emits. The definitions are identical LLVM SjLj
    // helpers, so allow the linker to keep the first and resolve the otherwise
    // missing __c_longjmp from the same archive. Must live here (the final
    // cdylib) because rustc-link-arg is not propagated from dependency crates.
    if target_arch == "wasm32" {
        println!("cargo:rustc-link-arg=--allow-multiple-definition");
    }
}
