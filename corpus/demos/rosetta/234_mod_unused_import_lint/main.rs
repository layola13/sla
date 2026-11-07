fn main() {
    let lint = include_str!("lint/index.sa");
    let used = include_str!("lint/used/index.sa");
    let used_seed = include_str!("lint/used/detail/seed.sa");
    let unused = include_str!("lint/unused/index.sa");
    let unused_seed = include_str!("lint/unused/detail/seed.sa");

    let imports_both = lint.contains("lint/used/index.sa") && lint.contains("lint/unused/index.sa");
    let only_used_called = lint.contains("call @lint_used_value()") && !lint.contains("call @lint_unused_value()");
    let used_branch_complete = used.contains("lint/used/detail/seed.sa") && used_seed.contains("@export lint_used_seed()");
    let unused_branch_complete = unused.contains("lint/unused/detail/seed.sa") && unused_seed.contains("@export lint_unused_seed()");
    let unused_lint_fixture = imports_both && only_used_called && used_branch_complete && unused_branch_complete;

    println!("{}", unused_lint_fixture as i32);
}
