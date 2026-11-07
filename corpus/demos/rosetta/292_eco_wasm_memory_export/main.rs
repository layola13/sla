fn main() {
    let guest = include_str!("guest/memory_export.sa");
    let layout = include_str!("guest/memory_export.sal");
    let asset = include_str!("assets/memory-layout.json");
    let note = include_str!("host/memory_export.note");
    let map = include_str!("host/memory_map.txt");

    let guest_exports_memory_entry = guest.contains("@ffi_wrapper memory_gate(*view: ptr) -> i32")
        && guest.contains("@export wasm_memory_export() -> i32");
    let layout_defines_view = layout.contains("#def MemoryView_value = +0")
        && layout.contains("#def MemoryView_marker = +4");
    let host_documents_memory = asset.contains("\"memory\": \"exported\"")
        && asset.contains("\"pages\": 1")
        && note.contains("map this guest memory")
        && map.contains("linear-memory = guest-owned");
    let memory_contract = guest_exports_memory_entry && layout_defines_view && host_documents_memory;

    println!("{}", memory_contract as i32);
}
