fn main() {
    let bridge = include_str!("bridge/rust_static_gate.sa");
    let layout = include_str!("bridge/rust_static_gate.sal");
    let ffi = include_str!("ffi/rust_staticlib.sai");
    let cargo = include_str!("host/rust/cargo-bridge.toml");
    let header = include_str!("host/rust/include/demo286.h");
    let archive = include_str!("host/rust/libdemo286.a.note");

    let bridge_uses_rust_staticlib = bridge.contains("ffi/rust_staticlib.sai")
        && bridge.contains("@ffi_wrapper rust_staticlib_gate(*slot: ptr) -> i32")
        && bridge.contains("load view+RustStaticProbe_minor as i32");
    let layout_has_two_fields = layout.contains("#def RustStaticProbe_tag = +0")
        && layout.contains("#def RustStaticProbe_minor = +4");
    let rust_host_metadata = ffi.contains("@extern rust_staticlib_probe(tag: i32) -> i32")
        && cargo.contains("name = \"demo286-bridge\"")
        && header.contains("int rust_staticlib_probe(int tag);")
        && archive.contains("Rust build would hand to the linker");
    let rust_staticlib_contract = bridge_uses_rust_staticlib && layout_has_two_fields && rust_host_metadata;

    println!("{}", rust_staticlib_contract as i32);
}
