fn main() {
    let startup = include_str!("guest/startup.sa");
    let layout = include_str!("guest/startup.sal");
    let docs = include_str!("docs/board.md");
    let linker = include_str!("host/board/linker.ld");
    let map = include_str!("host/board/memory-map.txt");

    let startup_exports_entry = startup.contains("@ffi_wrapper embedded_gate(*state: ptr) -> i32")
        && startup.contains("@export embedded_start() -> i32");
    let layout_defines_state = layout.contains("#def EmbeddedState_tag = +0")
        && layout.contains("#def EmbeddedState_status = +4");
    let board_metadata = docs.contains("startup, memory map, and linker notes")
        && linker.contains("ENTRY(_start)")
        && linker.contains("FLASH (rx)")
        && map.contains("startup = guest/startup.sa");
    let embedded_contract = startup_exports_entry && layout_defines_state && board_metadata;

    println!("{}", embedded_contract as i32);
}
