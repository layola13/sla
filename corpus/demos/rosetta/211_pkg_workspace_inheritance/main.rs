fn main() {
    let root = include_str!("sa.pkg");
    let workspace = include_str!("workspace/index.sa");
    let shared_config = include_str!("workspace/shared/config.sal");
    let shared_pkg = include_str!("workspace/shared/sa.pkg");
    let app_pkg = include_str!("workspace/members/app/sa.pkg");
    let tool_pkg = include_str!("workspace/members/tool/sa.pkg");
    let app_helper = include_str!("workspace/members/app/helpers/index.sa");
    let tool_helper = include_str!("workspace/members/tool/helpers/index.sa");

    let members_declared = root.contains("workspace/members/app") && root.contains("workspace/members/tool");
    let shared_imported = workspace.contains("workspace/shared/index.sa") && shared_pkg.contains("workspace-shared");
    let shared_constants = shared_config.contains("WORKSPACE_BASE = 100") && shared_config.contains("WORKSPACE_SHARED = 11");
    let members_named = app_pkg.contains("demo-211-app") && tool_pkg.contains("demo-211-tool");
    let helpers_take_shared_values = app_helper.contains("shared: i32") && tool_helper.contains("base: i32");
    let inherited = members_declared && shared_imported && shared_constants && members_named && helpers_take_shared_values;

    println!("{}", inherited as i32);
}
