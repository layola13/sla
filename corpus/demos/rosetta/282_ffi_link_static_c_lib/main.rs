fn main() {
    let bridge = include_str!("bridge/static_gate.sa");
    let layout = include_str!("bridge/static_gate.sal");
    let ffi = include_str!("ffi/static_lib.sai");
    let header = include_str!("host/include/staticlib.h");
    let archive = include_str!("host/libstatic.a.note");
    let linker = include_str!("host/linker/staticlib.ld");

    let bridge_uses_static_ffi = bridge.contains("ffi/static_lib.sai")
        && bridge.contains("@ffi_wrapper static_probe_gate(*slot: ptr) -> i32")
        && bridge.contains("load view+StaticProbe_tag as i32");
    let layout_matches_static_probe = layout.contains("#def StaticProbe_SIZE = 4")
        && layout.contains("#def StaticProbe_tag = +0");
    let host_declares_static_probe = ffi.contains("@extern staticlib_probe(tag: i32) -> i32")
        && header.contains("int staticlib_probe(int tag);");
    let archive_linked = archive.contains("static archive")
        && linker.contains("INPUT(libstatic.a)")
        && linker.contains("GROUP(-lc)");
    let static_contract = bridge_uses_static_ffi && layout_matches_static_probe && host_declares_static_probe && archive_linked;

    println!("{}", static_contract as i32);
}
