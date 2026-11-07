fn main() {
    let config = include_str!("build/ci.toml");
    let release = include_str!("ci/workflows/release.yml");
    let verify = include_str!("ci/workflows/verify.yml");
    let generated = include_str!("generated/ci/pipeline.sa");

    let config_points_to_pipeline = config.contains("pipeline = \"release\"")
        && config.contains("artifact = \"generated/ci/pipeline.sa\"");
    let workflows_cover_push_and_pr = release.contains("name: release")
        && release.contains("push:")
        && release.contains("branches:")
        && verify.contains("name: verify")
        && verify.contains("pull_request: {}");
    let generated_counts_workflows = generated.contains("build/ci.toml")
        && generated.contains("ci/workflows/*.yml")
        && generated.contains("#def CI_WORKFLOW_COUNT = 2");
    let ci_contract = config_points_to_pipeline && workflows_cover_push_and_pr && generated_counts_workflows;

    println!("{}", ci_contract as i32);
}
