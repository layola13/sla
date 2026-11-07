fn main() {
    let dep = include_str!("dep/index.sa");
    let mid = include_str!("dep/mid.sa");
    let leaf = include_str!("dep/leaf.sa");
    let legacy_mid = include_str!("mid.sa");
    let legacy_leaf = include_str!("leaf.sa");

    let dep_imports_mid = dep.contains("dep/mid.sa") && dep.contains("@export transitive_value()");
    let mid_imports_leaf = mid.contains("dep/leaf.sa") && mid.contains("@export dep_mid_value()");
    let leaf_exports_value = leaf.contains("@export dep_leaf_value()");
    let older_flat_path_kept = legacy_mid.contains("@import \"leaf.sa\"") && legacy_leaf.contains("@leaf_value()");
    let transitive_dependency = dep_imports_mid && mid_imports_leaf && leaf_exports_value && older_flat_path_kept;

    println!("{}", transitive_dependency as i32);
}
