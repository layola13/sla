fn main() {
    let pkg = include_str!("sa.pkg");
    let vendor = include_str!("vendor/git_dep.sa");
    let has_git_url = pkg.contains("git = \"https://example.invalid/demo-203.git\"");
    let has_rev = pkg.contains("rev = \"d17c0de\"");
    let has_vendor_checkout = vendor.contains("@git_dep_value()");
    let result = if has_git_url && has_rev && has_vendor_checkout { 1 } else { 0 };
    println!("{}", result);
}
