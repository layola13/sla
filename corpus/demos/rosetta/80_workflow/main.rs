struct WorkflowState {
    lint_ok: bool,
    tests_ok: bool,
    package_ok: bool,
}

fn completed_steps(state: WorkflowState) -> i32 {
    i32::from(state.lint_ok) + i32::from(state.tests_ok) + i32::from(state.package_ok)
}

fn main() {
    let state = WorkflowState { lint_ok: true, tests_ok: true, package_ok: false };
    println!("{}", completed_steps(state));
}
