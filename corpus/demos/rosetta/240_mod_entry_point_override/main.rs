fn main() {
    let entry = include_str!("entry/index.sa");
    let default = include_str!("entry/default/index.sa");
    let default_seed = include_str!("entry/default/detail/seed.sa");
    let override_entry = include_str!("entry/override/index.sa");
    let override_seed = include_str!("entry/override/detail/seed.sa");

    let entry_has_both = entry.contains("entry/default/index.sa") && entry.contains("entry/override/index.sa");
    let override_selected = entry.contains("call @override_entry_value()") && !entry.contains("call @default_entry_value()");
    let default_branch_complete = default.contains("entry/default/detail/seed.sa") && default_seed.contains("@export default_seed()");
    let override_branch_complete = override_entry.contains("entry/override/detail/seed.sa") && override_seed.contains("@export override_seed()");
    let entry_override = entry_has_both && override_selected && default_branch_complete && override_branch_complete;

    println!("{}", entry_override as i32);
}
