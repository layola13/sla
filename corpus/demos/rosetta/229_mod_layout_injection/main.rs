fn main() {
    let api = include_str!("api/record.sai");
    let layout = include_str!("layout/record.sal");
    let bridge = include_str!("bridge/record.sa");

    let iface_declared = api.contains("@extern record_mix_contract") && api.contains("ptr");
    let layout_declared = layout.contains("Record_SIZE = 8") && layout.contains("Record_lo = +0") && layout.contains("Record_hi = +4");
    let wrapper_uses_layout = bridge.contains("@ffi_wrapper record_mix") && bridge.contains("Record_lo") && bridge.contains("Record_hi");
    let layout_injected = iface_declared && layout_declared && wrapper_uses_layout;

    println!("{}", layout_injected as i32);
}
