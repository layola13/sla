fn main() {
    let pkg = include_str!("sa.pkg");
    let dep_src = include_str!("pkg/local_dep.sa");
    let has_local_path = pkg.contains("path = \"pkg/local_dep.sa\"");
    let has_local_symbol = dep_src.contains("@local_dep_value()");
    let result = if has_local_path && has_local_symbol { 1 } else { 0 };
    println!("{}", result);
}
