fn main() {
    let manifest = include_str!("harness/manifest.toml");
    let basic = include_str!("harness/cases/basic.toml");
    let ffi = include_str!("harness/cases/ffi.toml");
    let generated = include_str!("generated/harness/index.sa");

    let manifest_lists_cases = manifest.contains("cases = [\"basic\", \"ffi\"]");
    let cases_have_commands = basic.contains("name = \"basic\"")
        && basic.contains("command = \"run\"")
        && ffi.contains("name = \"ffi\"")
        && ffi.contains("command = \"run-ffi\"");
    let generated_indexes_cases = generated.contains("harness/manifest.toml")
        && generated.contains("harness/cases/*.toml")
        && generated.contains("#def HARNESS_CASE_COUNT = 2");
    let harness_contract = manifest_lists_cases && cases_have_commands && generated_indexes_cases;

    println!("{}", harness_contract as i32);
}
