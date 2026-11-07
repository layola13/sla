fn main() {
    let root = include_str!("sa.pkg");
    let resolver = include_str!("resolver/index.sa");
    let v1_pkg = include_str!("packages/v1_0_0/sa.pkg");
    let v2_pkg = include_str!("packages/v2_0_0/sa.pkg");
    let v1_leaf = include_str!("packages/v1_0_0/leaf.sa");
    let v2_leaf = include_str!("packages/v2_0_0/leaf.sa");

    let same_package = v1_pkg.contains("name = \"demo-207-dep\"") && v2_pkg.contains("name = \"demo-207-dep\"");
    let incompatible_versions = v1_pkg.contains("version = \"1.0.0\"") && v2_pkg.contains("version = \"2.0.0\"");
    let resolver_imports_both = resolver.contains("v1_0_0/index.sa") && resolver.contains("v2_0_0/index.sa");
    let duplicate_symbol = v1_leaf.contains("@dep_value()") && v2_leaf.contains("@dep_value()");
    let declared_conflict = root.contains("same public symbol in two versioned packages");
    let conflict_reported = same_package && incompatible_versions && resolver_imports_both && duplicate_symbol && declared_conflict;

    println!("{}", conflict_reported as i32);
}
