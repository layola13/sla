fn main() {
    let manifest = include_str!("bench/manifest.toml");
    let throughput = include_str!("bench/cases/throughput.toml");
    let latency = include_str!("bench/cases/latency.toml");
    let generated = include_str!("generated/bench/runner.sa");

    let manifest_lists_bench_cases = manifest.contains("throughput")
        && manifest.contains("latency");
    let cases_have_sample_counts = throughput.contains("name = \"throughput\"")
        && throughput.contains("samples = 128")
        && latency.contains("name = \"latency\"")
        && latency.contains("samples = 32");
    let generated_counts_bench_cases = generated.contains("bench/manifest.toml")
        && generated.contains("bench/cases/*.toml")
        && generated.contains("#def BENCHMARK_CASE_COUNT = 2");
    let bench_contract = manifest_lists_bench_cases && cases_have_sample_counts && generated_counts_bench_cases;

    println!("{}", bench_contract as i32);
}
