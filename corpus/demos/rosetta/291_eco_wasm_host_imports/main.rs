fn main() {
    let guest = include_str!("guest/guest_entry.sa");
    let layout = include_str!("guest/guest_entry.sal");
    let imports = include_str!("host/host_imports.sai");
    let wit = include_str!("host/imports.wit.md");
    let docs = include_str!("docs/runtime.md");

    let guest_imports_host_surface = guest.contains("host/host_imports.sai")
        && guest.contains("@ffi_wrapper wasm_guest_gate(*frame: ptr) -> i32")
        && guest.contains("@export wasm_guest_entry() -> i32");
    let layout_defines_frame = layout.contains("#def WasmHostFrame_tag = +0")
        && layout.contains("#def WasmHostFrame_status = +4");
    let host_imports_are_explicit = imports.contains("@extern host_log(tag: i32) -> i32")
        && imports.contains("@extern host_clock_ms() -> i32")
        && wit.contains("host-side imports")
        && docs.contains("host import table separate");
    let wasm_import_contract = guest_imports_host_surface && layout_defines_frame && host_imports_are_explicit;

    println!("{}", wasm_import_contract as i32);
}
