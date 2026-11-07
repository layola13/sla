fn main() {
    let pkg = include_str!("sa.pkg");
    let root = include_str!("src/index.sa");
    let defaults = include_str!("src/defaults/index.sa");
    let default_defs = include_str!("src/defaults/helpers/defaults.sal");
    let helper = include_str!("src/defaults/helpers/index.sa");

    let package_named = pkg.contains("name = \"demo-213\"");
    let nested_defaults_module = root.contains("src/defaults/index.sa") && defaults.contains("src/defaults/helpers/index.sa");
    let default_constant = default_defs.contains("DEFAULT_OFFSET = 13");
    let helper_uses_default = helper.contains("DEFAULT_OFFSET");
    let default_layout = package_named && nested_defaults_module && default_constant && helper_uses_default;

    println!("{}", default_layout as i32);
}
