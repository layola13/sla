fn main() {
    let prelude = include_str!("prelude/index.sa");
    let iface = include_str!("prelude/index.sai");
    let layout = include_str!("prelude/index.sal");
    let seed = include_str!("prelude/core/seed.sa");

    let prelude_imports_layers = prelude.contains("prelude/index.sai") && prelude.contains("prelude/index.sal") && prelude.contains("prelude/core/seed.sa");
    let iface_declares_contracts = iface.contains("prelude_seed_contract") && iface.contains("prelude_value_contract");
    let layout_declares_slot = layout.contains("Prelude_SIZE = 4") && layout.contains("Prelude_value = +0");
    let seed_exported = seed.contains("@export prelude_seed()");
    let prelude_layout = prelude_imports_layers && iface_declares_contracts && layout_declares_slot && seed_exported;

    println!("{}", prelude_layout as i32);
}
