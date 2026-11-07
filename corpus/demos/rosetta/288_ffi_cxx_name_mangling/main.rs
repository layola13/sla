fn main() {
    let bridge = include_str!("bridge/cxx_gate.sa");
    let layout = include_str!("bridge/cxx_gate.sal");
    let ffi = include_str!("ffi/cxx_name.sai");
    let header = include_str!("host/cxx/include/demo.hpp");
    let map = include_str!("host/cxx/linker.map");
    let nm = include_str!("host/cxx/nm.txt");

    let bridge_uses_cxx_probe = bridge.contains("ffi/cxx_name.sai")
        && bridge.contains("@ffi_wrapper cxx_name_gate(*probe: ptr) -> i32")
        && layout.contains("#def CxxProbe_suffix = +4");
    let extern_c_surface = ffi.contains("@extern cxx_name_probe(tag: i32) -> i32")
        && header.contains("extern \"C\" int cxx_name_probe(int tag);");
    let symbol_map_is_explicit = map.contains("cxx_name_probe;")
        && map.contains("local:")
        && nm.contains("T cxx_name_probe")
        && nm.contains("T cxx_name_probe_wrapper");
    let cxx_contract = bridge_uses_cxx_probe && extern_c_surface && symbol_map_is_explicit;

    println!("{}", cxx_contract as i32);
}
