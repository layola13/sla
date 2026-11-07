fn main() {
    let bridge = include_str!("bridge/dynamic_gate.sa");
    let layout = include_str!("bridge/dynamic_gate.sal");
    let ffi = include_str!("ffi/dynamic_lib.sai");
    let header = include_str!("host/include/dynamiclib.h");
    let rpath = include_str!("host/loader/rpath.txt");
    let pkg = include_str!("host/pkgconfig/dynamiclib.pc");

    let bridge_uses_dynamic_ffi = bridge.contains("ffi/dynamic_lib.sai")
        && bridge.contains("@ffi_wrapper dynamic_probe_gate(*slot: ptr) -> i32")
        && bridge.contains("load view+DynamicProbe_tag as i32");
    let layout_matches_dynamic_probe = layout.contains("#def DynamicProbe_SIZE = 4")
        && layout.contains("#def DynamicProbe_tag = +0");
    let host_declares_dynamic_probe = ffi.contains("@extern dynamiclib_probe(tag: i32) -> i32")
        && header.contains("int dynamiclib_probe(int tag);");
    let dynamic_loader_metadata = rpath.contains("loader=dlopen")
        && rpath.contains("rpath=$ORIGIN")
        && pkg.contains("Libs: -ldynamic");
    let dynamic_contract = bridge_uses_dynamic_ffi && layout_matches_dynamic_probe && host_declares_dynamic_probe && dynamic_loader_metadata;

    println!("{}", dynamic_contract as i32);
}
