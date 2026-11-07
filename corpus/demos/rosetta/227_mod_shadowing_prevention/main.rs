fn main() {
    let registry = include_str!("shadow/registry/index.sa");
    let left = include_str!("shadow/left/index.sa");
    let right = include_str!("shadow/right/index.sa");
    let left_layout = include_str!("shadow/left/layout.sal");
    let right_layout = include_str!("shadow/right/layout.sal");

    let registry_imports_both = registry.contains("../left/index.sa") && registry.contains("../right/index.sa");
    let both_use_layouts = left.contains("shadow/left/layout.sal") && right.contains("shadow/right/layout.sal");
    let duplicate_defs = left_layout.contains("#def SHADOW_SIZE") && right_layout.contains("#def SHADOW_SIZE");
    let conflicting_values = left_layout.contains("SHADOW_SIZE = 227") && right_layout.contains("SHADOW_SIZE = 900");
    let shadow_fixture = registry_imports_both && both_use_layouts && duplicate_defs && conflicting_values;

    println!("{}", shadow_fixture as i32);
}
