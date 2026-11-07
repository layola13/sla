fn main() {
    let pkg = include_str!("sa.pkg");
    let has_name = pkg.contains("name = \"demo-201\"");
    let has_version = pkg.contains("version = \"0.1.0\"");
    let has_entry = pkg.contains("entry = \"main.sa\"");
    let result = (has_name as i32) + (has_version as i32) + (has_entry as i32);
    println!("{}", result);
}
