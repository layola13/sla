fn main() {
    let bridge = include_str!("bridge/handle_gate.sa");
    let layout = include_str!("bridge/handle_gate.sal");
    let ffi = include_str!("ffi/handle.sai");
    let header = include_str!("host/include/handle.h");
    let manifest = include_str!("host/ownership/handle.manifest");
    let notes = include_str!("host/ownership/notes.md");

    let bridge_reads_handle_slot = bridge.contains("@ffi_wrapper handle_gate(*slot: ptr) -> i32")
        && bridge.contains("load view+HandleSlot_refcount as i32")
        && layout.contains("#def HandleSlot_refcount = +4");
    let ffi_declares_open_close = ffi.contains("@extern handle_open(id: i32) -> ptr")
        && ffi.contains("@extern handle_close(handle: ptr) -> i32");
    let host_marks_opaque_external = header.contains("typedef struct DemoHandle DemoHandle;")
        && header.contains("DemoHandle *handle_open(int id);")
        && manifest.contains("opaque-handle = true")
        && manifest.contains("ownership = external")
        && notes.contains("host owns the real handle lifetime");
    let handle_contract = bridge_reads_handle_slot && ffi_declares_open_close && host_marks_opaque_external;

    println!("{}", handle_contract as i32);
}
