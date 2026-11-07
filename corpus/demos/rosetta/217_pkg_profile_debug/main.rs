fn main() {
    let pkg = include_str!("sa.pkg");
    let root = include_str!("src/index.sa");
    let profile = include_str!("src/profiles/debug/index.sa");
    let helper = include_str!("src/profiles/debug/helpers/index.sa");
    let profile_defs = include_str!("src/profiles/debug/helpers/profile.sal");

    let package_named = pkg.contains("name = \"demo-217\"");
    let debug_tree = root.contains("src/profiles/debug/index.sa") && profile.contains("helpers/index.sa");
    let debug_helper = helper.contains("@debug_helper_value()") && helper.contains("PROFILE_OFFSET");
    let debug_constant = profile_defs.contains("PROFILE_OFFSET = 17");
    let debug_profile = package_named && debug_tree && debug_helper && debug_constant;

    println!("{}", debug_profile as i32);
}
