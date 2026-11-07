fn main() {
    let pkg = include_str!("sa.pkg");
    let root = include_str!("src/index.sa");
    let metadata = include_str!("src/metadata/index.sa");
    let helper = include_str!("src/metadata/helpers/index.sa");
    let metadata_defs = include_str!("src/metadata/helpers/metadata.sal");

    let package_named = pkg.contains("name = \"demo-218\"");
    let custom_metadata = pkg.contains("custom-key = \"metadata tree\"");
    let metadata_tree = root.contains("src/metadata/index.sa") && metadata.contains("helpers/index.sa");
    let metadata_helper = helper.contains("@metadata_helper_value()") && metadata_defs.contains("METADATA_OFFSET = 18");
    let metadata_layout = package_named && custom_metadata && metadata_tree && metadata_helper;

    println!("{}", metadata_layout as i32);
}
