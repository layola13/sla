fn main() {
    let module_root = include_str!("module/index.sa");
    let module_leaf = include_str!("module/leaf.sa");
    let tree_root = include_str!("module/tree/index.sa");
    let tree_leaf = include_str!("module/tree/leaf.sa");

    let root_imports_tree = module_root.contains("module/tree/index.sa") && module_root.contains("@export directory_value()");
    let tree_imports_leaf = tree_root.contains("module/tree/leaf.sa") && tree_root.contains("@export module_tree_value()");
    let tree_leaf_exports = tree_leaf.contains("@export module_leaf_value()");
    let sibling_leaf_is_separate = module_leaf.contains("@leaf_value()") && !module_root.contains("module/leaf.sa");
    let directory_module = root_imports_tree && tree_imports_leaf && tree_leaf_exports && sibling_leaf_is_separate;

    println!("{}", directory_module as i32);
}
