//! Links the wasm libc that `dioxus-code`'s tree-sitter grammars compile against.
//!
//! `arborium-sysroot` ships that libc as a static archive, but nothing in Rust references the
//! crate, so rustc drops it from the final link and `stderr` comes out undefined. Its search path
//! survives, so naming the library here is enough. `-bundle` keeps archive semantics: only the
//! members the grammars actually need are pulled, where linking the crate whole would duplicate
//! every allocator symbol Rust's own wasm runtime already defines.

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        println!("cargo::rustc-link-lib=static:-bundle=arborium_sysroot");
    }
}
