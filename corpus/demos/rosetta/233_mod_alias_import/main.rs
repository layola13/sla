fn main() {
    let alias = include_str!("alias/index.sa");
    let deep = include_str!("alias/deep/index.sa");
    let seed = include_str!("alias/deep/seed.sa");

    let alias_wraps_deep = alias.contains("alias/deep/index.sa") && alias.contains("@export alias_value()");
    let deep_imports_seed = deep.contains("alias/deep/seed.sa") && deep.contains("@export alias_deep_value()");
    let seed_exported = seed.contains("@export alias_seed()");
    let short_surface_hides_seed = !alias.contains("alias_seed");
    let alias_import = alias_wraps_deep && deep_imports_seed && seed_exported && short_surface_hides_seed;

    println!("{}", alias_import as i32);
}
