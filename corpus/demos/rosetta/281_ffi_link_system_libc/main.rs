fn main() {
    let bridge = include_str!("bridge/libc_gate.sa");
    let layout = include_str!("bridge/libc_gate.sal");
    let ffi = include_str!("ffi/libc.sai");
    let header = include_str!("host/libc.h");
    let linker = include_str!("host/linker/system-libc.ld");
    let pkg = include_str!("host/system-libc.pc");

    let bridge_uses_libc_ffi = bridge.contains("ffi/libc.sai")
        && bridge.contains("@ffi_wrapper libc_probe_gate(*probe: ptr) -> i32")
        && bridge.contains("load view+LibcProbe_tag as i32");
    let layout_matches_probe = layout.contains("#def LibcProbe_SIZE = 4")
        && layout.contains("#def LibcProbe_tag = +0");
    let host_declares_system_probe = ffi.contains("@extern system_probe(tag: i32) -> i32")
        && header.contains("int system_probe(int tag);");
    let link_metadata = linker.contains("GROUP(-lc)")
        && pkg.contains("Name: demo-281-system-libc")
        && pkg.contains("Libs: -lc");
    let libc_contract = bridge_uses_libc_ffi && layout_matches_probe && host_declares_system_probe && link_metadata;

    println!("{}", libc_contract as i32);
}
