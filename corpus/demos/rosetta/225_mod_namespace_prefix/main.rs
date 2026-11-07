fn main() {
    let index = include_str!("ns/prefix/index.sa");
    let alpha = include_str!("ns/prefix/alpha.sa");
    let beta = include_str!("ns/prefix/beta.sa");

    let namespace_folder = index.contains("ns/prefix/alpha.sa") && index.contains("ns/prefix/beta.sa");
    let namespace_exports = index.contains("@export ns_prefix_value()");
    let prefixed_symbols = alpha.contains("@export ns_prefix_alpha()") && beta.contains("@export ns_prefix_beta()");
    let namespace_layout = namespace_folder && namespace_exports && prefixed_symbols;

    println!("{}", namespace_layout as i32);
}
