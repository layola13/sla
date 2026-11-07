fn main() {
    let pkg_a = include_str!("pkg_a/main.sa");
    let pkg_b = include_str!("pkg_b/main.sa");
    let a_imports_b = pkg_a.contains("@import \"pkg_a/../pkg_b/main.sa\"");
    let b_imports_a = pkg_b.contains("@import \"pkg_b/../pkg_a/main.sa\"");
    let result = if a_imports_b && b_imports_a { 1 } else { 0 };
    println!("{}", result);
}
