fn main() {
    let selector = include_str!("profiles/index.sa");
    let native = include_str!("profiles/native/index.sa");
    let native_seed = include_str!("profiles/native/detail/seed.sa");
    let portable = include_str!("profiles/portable/index.sa");
    let portable_seed = include_str!("profiles/portable/detail/seed.sa");

    let selector_has_both_branches = selector.contains("profiles/native/index.sa") && selector.contains("profiles/portable/index.sa");
    let selector_chooses_native = selector.contains("call @native_value()") && !selector.contains("call @portable_value()");
    let native_branch_complete = native.contains("native/detail/seed.sa") && native_seed.contains("@export native_seed()");
    let portable_branch_complete = portable.contains("portable/detail/seed.sa") && portable_seed.contains("@export portable_seed()");
    let conditional_import = selector_has_both_branches && selector_chooses_native && native_branch_complete && portable_branch_complete;

    println!("{}", conditional_import as i32);
}
