fn main() {
    let pkg = include_str!("sa.pkg");
    let codec = include_str!("registry/codec.sa");
    let has_registry = pkg.contains("registry = \"sa-lang\"");
    let has_version = pkg.contains("version = \"1.2.3\"");
    let has_cached_codec = codec.contains("@codec_value()");
    let result = if has_registry && has_version && has_cached_codec { 1 } else { 0 };
    println!("{}", result);
}
