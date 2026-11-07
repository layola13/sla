fn main() {
    let bridge = include_str!("bridge/index.sa");
    let value = include_str!("bridge/deep/value.sa");
    let seed = include_str!("bridge/deep/seed.sa");

    let bridge_reexports_deep = bridge.contains("bridge/deep/value.sa") && bridge.contains("@export bridge_value()");
    let deep_value_imports_seed = value.contains("bridge/deep/seed.sa") && value.contains("@export deep_value()");
    let seed_exports_value = seed.contains("@export deep_seed()");
    let public_surface_hides_seed = !bridge.contains("deep_seed");
    let reexport_layout = bridge_reexports_deep && deep_value_imports_seed && seed_exports_value && public_surface_hides_seed;

    println!("{}", reexport_layout as i32);
}
