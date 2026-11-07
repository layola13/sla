fn main() {
    let pkg = include_str!("sa.pkg");
    let root = include_str!("src/index.sa");
    let profile = include_str!("src/profiles/release/index.sa");
    let helper = include_str!("src/profiles/release/helpers/index.sa");
    let profile_defs = include_str!("src/profiles/release/helpers/profile.sal");

    let package_named = pkg.contains("name = \"demo-216\"");
    let release_tree = root.contains("src/profiles/release/index.sa") && profile.contains("helpers/index.sa");
    let release_helper = helper.contains("@release_helper_value()") && helper.contains("PROFILE_OFFSET");
    let release_constant = profile_defs.contains("PROFILE_OFFSET = 16");
    let release_profile = package_named && release_tree && release_helper && release_constant;

    println!("{}", release_profile as i32);
}
