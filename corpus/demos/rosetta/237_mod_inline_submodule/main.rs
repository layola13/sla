fn main() {
    let submodule = include_str!("submodule/index.sa");
    let inline = include_str!("submodule/inline/index.sa");
    let seed = include_str!("submodule/inline/deep/seed.sa");

    let outer_imports_inline = submodule.contains("submodule/inline/index.sa") && submodule.contains("@export submodule_value()");
    let inline_imports_seed = inline.contains("submodule/inline/deep/seed.sa") && inline.contains("@export inline_value()");
    let seed_exported = seed.contains("@export inline_seed()");
    let inline_submodule = outer_imports_inline && inline_imports_seed && seed_exported;

    println!("{}", inline_submodule as i32);
}
