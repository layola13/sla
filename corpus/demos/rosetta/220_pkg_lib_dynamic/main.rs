fn main() {
    let pkg = include_str!("sa.pkg");
    let host = include_str!("host/index.sa");
    let host_defs = include_str!("host/helpers/host.sal");
    let iface = include_str!("lib/iface.sai");
    let lib = include_str!("lib/index.sa");
    let lib_impl = include_str!("lib/impl/index.sa");
    let lib_defs = include_str!("lib/impl/helpers/lib.sal");

    let package_split = pkg.contains("library = \"lib\"") && pkg.contains("host = \"host\"");
    let host_consumes_iface = host.contains("lib/iface.sai") && iface.contains("@extern sa_library_dynamic_value()");
    let lib_exports_iface = lib.contains("@export sa_library_dynamic_value()") && lib_impl.contains("helpers/index.sa");
    let values_defined = host_defs.contains("HOST_WRAPPER_BONUS = 40") && lib_defs.contains("LIB_DYNAMIC_VALUE = 180");
    let dynamic_layout = package_split && host_consumes_iface && lib_exports_iface && values_defined;

    println!("{}", dynamic_layout as i32);
}
