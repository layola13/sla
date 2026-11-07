fn main() {
    let logo = include_str!("assets/text/logo.txt");
    let tagline = include_str!("assets/text/tagline.txt");
    let manifest = include_str!("bundle/manifest.toml");
    let generated = include_str!("generated/asset_bundle.sa");

    let assets_are_real = logo.contains("SA-ASM asset bundle")
        && tagline.contains("bundled from assets/text");
    let manifest_lists_assets = manifest.contains("assets/text/logo.txt")
        && manifest.contains("assets/text/tagline.txt")
        && manifest.contains("output = \"generated/asset_bundle.sa\"");
    let generated_mentions_sources = generated.contains("bundle/manifest.toml")
        && generated.contains("assets/text/*.txt")
        && generated.contains("@asset_bundle_value() -> i32");
    let bundle_contract = assets_are_real && manifest_lists_assets && generated_mentions_sources;

    println!("{}", bundle_contract as i32);
}
