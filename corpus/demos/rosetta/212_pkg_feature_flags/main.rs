fn main() {
    let pkg = include_str!("sa.pkg");
    let root = include_str!("src/index.sa");
    let flags = include_str!("src/flags/index.sa");
    let flag_defs = include_str!("src/flags/helpers/flags.sal");
    let helper = include_str!("src/flags/helpers/index.sa");

    let package_named = pkg.contains("name = \"demo-212\"");
    let nested_feature_module = root.contains("src/flags/index.sa") && flags.contains("src/flags/helpers/index.sa");
    let feature_constants = flag_defs.contains("FLAG_ALPHA") && flag_defs.contains("FLAG_BETA") && flag_defs.contains("FLAG_GAMMA");
    let helper_combines_flags = helper.contains("FLAG_ALPHA") && helper.contains("FLAG_BETA") && helper.contains("FLAG_GAMMA");
    let feature_layout = package_named && nested_feature_module && feature_constants && helper_combines_flags;

    println!("{}", feature_layout as i32);
}
