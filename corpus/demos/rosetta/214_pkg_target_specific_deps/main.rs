fn main() {
    let pkg = include_str!("sa.pkg");
    let root = include_str!("src/index.sa");
    let target_index = include_str!("src/targets/index.sa");
    let target_defs = include_str!("src/targets/helpers/target.sal");
    let helper = include_str!("src/targets/helpers/index.sa");
    let native = include_str!("src/targets/helpers/native.sa");
    let portable = include_str!("src/targets/helpers/portable.sa");

    let package_named = pkg.contains("name = \"demo-214\"");
    let target_module = root.contains("src/targets/index.sa") && target_index.contains("target.sal");
    let target_switch = target_defs.contains("TARGET_NATIVE = 1") && helper.contains("L_NATIVE") && helper.contains("L_PORTABLE");
    let both_branches_present = native.contains("@native_target_value()") && portable.contains("@portable_target_value()");
    let target_specific = package_named && target_module && target_switch && both_branches_present;

    println!("{}", target_specific as i32);
}
