fn main() {
    let group = include_str!("ffi/group/index.sa");
    let iface = include_str!("ffi/group/index.sai");
    let layout = include_str!("ffi/group/layout.sal");
    let bridge = include_str!("ffi/group/bridge.sa");
    let seed = include_str!("ffi/group/core/seed.sa");

    let group_imports_layers = group.contains("index.sai") && group.contains("layout.sal") && group.contains("bridge.sa");
    let grouped_contracts = iface.contains("ffi_probe_contract") && iface.contains("ffi_reset_contract");
    let shared_layout = layout.contains("Group_SIZE = 4") && layout.contains("Group_value = +0");
    let bridge_exports_both = bridge.contains("@export ffi_probe") && bridge.contains("@export ffi_reset") && bridge.contains("core/seed.sa");
    let seed_exported = seed.contains("@export ffi_seed()");
    let extern_group = group_imports_layers && grouped_contracts && shared_layout && bridge_exports_both && seed_exported;

    println!("{}", extern_group as i32);
}
