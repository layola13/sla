fn main() {
    let config = include_str!("config/pkg-config.toml");
    let bridge = include_str!("bridge/pkg_config_gate.sa");
    let layout = include_str!("bridge/pkg_config_gate.sal");
    let ffi = include_str!("ffi/pkg_config.sai");
    let header = include_str!("host/include/pkg_config.h");
    let pc = include_str!("host/pkgconfig/demo284.pc");

    let config_points_to_pkgconfig = config.contains("name = \"demo-284-pkg-config\"")
        && config.contains("libdir = \"host/pkgconfig\"");
    let bridge_uses_pkg_config_ffi = bridge.contains("ffi/pkg_config.sai")
        && bridge.contains("@ffi_wrapper pkg_config_probe_gate(*probe: ptr) -> i32")
        && layout.contains("#def PkgConfigProbe_tag = +0");
    let host_declares_pkg_config_probe = ffi.contains("@extern pkg_config_probe(tag: i32) -> i32")
        && header.contains("int pkg_config_probe(int tag);");
    let pc_exports_paths = pc.contains("Name: demo-284-pkg-config")
        && pc.contains("Libs: -ldemo284")
        && pc.contains("Cflags: -I${includedir}");
    let pkg_config_contract = config_points_to_pkgconfig && bridge_uses_pkg_config_ffi && host_declares_pkg_config_probe && pc_exports_paths;

    println!("{}", pkg_config_contract as i32);
}
