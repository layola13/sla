fn main() {
    let root = include_str!("sa.pkg");
    let workspace = include_str!("workspace/index.sa");
    let alpha_pkg = include_str!("workspace/members/alpha/sa.pkg");
    let beta_pkg = include_str!("workspace/members/beta/sa.pkg");

    let root_lists_alpha = root.contains("workspace/members/alpha");
    let root_lists_beta = root.contains("workspace/members/beta");
    let workspace_imports_members = workspace.contains("alpha/index.sa") && workspace.contains("beta/index.sa");
    let named_members = alpha_pkg.contains("demo-210-alpha") && beta_pkg.contains("demo-210-beta");
    let result = root_lists_alpha && root_lists_beta && workspace_imports_members && named_members;

    println!("{}", result as i32);
}
