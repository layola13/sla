fn main() {
    let root = include_str!("shared/root/index.sa");
    let codec = include_str!("shared/root/codec/index.sa");
    let leaf = include_str!("shared/root/codec/leaf.sa");

    let root_imports_codec = root.contains("@import \"shared/root/codec/index.sa\"") && root.contains("@export shared_root_value()");
    let codec_imports_leaf = codec.contains("@import \"shared/root/codec/leaf.sa\"") && codec.contains("@export shared_root_codec()");
    let leaf_exports_value = leaf.contains("@export shared_root_leaf()");
    let absolute_tree = root_imports_codec && codec_imports_leaf && leaf_exports_value;

    println!("{}", absolute_tree as i32);
}
