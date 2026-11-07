fn main() {
    let paths = include_str!("paths/index.sa");
    let first = include_str!("paths/first/index.sa");
    let first_seed = include_str!("paths/first/deep/seed.sa");
    let second = include_str!("paths/second/index.sa");
    let second_seed = include_str!("paths/second/deep/seed.sa");

    let explicit_order = paths.find("paths/first/index.sa") < paths.find("paths/second/index.sa");
    let aggregate_imports_both = paths.contains("@export path_value()") && paths.contains("paths/first/index.sa") && paths.contains("paths/second/index.sa");
    let first_branch_complete = first.contains("paths/first/deep/seed.sa") && first_seed.contains("@export first_seed()");
    let second_branch_complete = second.contains("paths/second/deep/seed.sa") && second_seed.contains("@export second_seed()");
    let path_order = explicit_order && aggregate_imports_both && first_branch_complete && second_branch_complete;

    println!("{}", path_order as i32);
}
