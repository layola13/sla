fn main() {
    let parse_job = include_str!("build/parallel/jobs/parse.toml");
    let codegen_job = include_str!("build/parallel/jobs/codegen.toml");
    let generated = include_str!("generated/parallel/schedule.sa");

    let jobs_are_parallel_inputs = parse_job.contains("name = \"parse\"")
        && parse_job.contains("lane = 0")
        && codegen_job.contains("name = \"codegen\"")
        && codegen_job.contains("lane = 1");
    let generated_mentions_jobs = generated.contains("build/parallel/jobs/*.toml")
        && generated.contains("#def PARALLEL_JOB_COUNT = 2")
        && generated.contains("@parallel_schedule_count() -> i32");
    let parallel_contract = jobs_are_parallel_inputs && generated_mentions_jobs;

    println!("{}", parallel_contract as i32);
}
