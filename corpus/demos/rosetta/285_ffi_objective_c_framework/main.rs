fn main() {
    let bridge = include_str!("bridge/objc_gate.sa");
    let layout = include_str!("bridge/objc_gate.sal");
    let ffi = include_str!("ffi/objective_c.sai");
    let header = include_str!("host/framework/DemoKit.framework/Headers/DemoKit.h");
    let modulemap = include_str!("host/framework/DemoKit.framework/Modules/module.modulemap");
    let binary_note = include_str!("host/framework/DemoKit.framework/DemoKit.tbd.note");

    let bridge_uses_framework_layout = bridge.contains("@ffi_wrapper objc_framework_gate(*frame: ptr) -> i32")
        && bridge.contains("load view+ObjCFrame_framework as i32")
        && layout.contains("#def ObjCFrame_framework = +4");
    let ffi_and_header_match = ffi.contains("@extern objc_framework_probe(tag: i32) -> i32")
        && header.contains("int objc_framework_probe(int tag);");
    let framework_bundle_metadata = modulemap.contains("framework module DemoKit")
        && modulemap.contains("header \"DemoKit.h\"")
        && binary_note.contains(".framework bundle");
    let objc_contract = bridge_uses_framework_layout && ffi_and_header_match && framework_bundle_metadata;

    println!("{}", objc_contract as i32);
}
