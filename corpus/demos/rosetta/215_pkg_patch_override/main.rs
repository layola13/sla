fn main() {
    let pkg = include_str!("sa.pkg");
    let root = include_str!("src/index.sa");
    let patches = include_str!("src/patches/index.sa");
    let helper = include_str!("src/patches/helpers/index.sa");
    let upstream = include_str!("src/patches/helpers/upstream.sa");
    let override_src = include_str!("src/patches/helpers/override.sa");
    let patch_defs = include_str!("src/patches/helpers/patch.sal");

    let package_named = pkg.contains("name = \"demo-215\"");
    let patch_module = root.contains("src/patches/index.sa") && patches.contains("src/patches/helpers/index.sa");
    let override_wraps_upstream = helper.contains("override.sa") && override_src.contains("upstream.sa") && upstream.contains("@upstream_patch_value()");
    let patch_bias = patch_defs.contains("PATCH_BIAS = 5") && override_src.contains("PATCH_BIAS");
    let patched = package_named && patch_module && override_wraps_upstream && patch_bias;

    println!("{}", patched as i32);
}
