fn main() {
    let seed = include_str!("build/repro/seed.toml");
    let fingerprint = include_str!("cache/repro/fingerprint.txt");
    let generated = include_str!("generated/repro/build.sa");

    let seed_is_fixed = seed.contains("seed = 424242")
        && seed.contains("epoch = 0");
    let fingerprint_matches_seed = fingerprint.contains("seed=424242")
        && fingerprint.contains("epoch=0");
    let generated_mentions_repro_sources = generated.contains("build/repro/seed.toml")
        && generated.contains("cache/repro/fingerprint.txt")
        && generated.contains("#def REPRO_ARTIFACT_COUNT = 2");
    let repro_contract = seed_is_fixed && fingerprint_matches_seed && generated_mentions_repro_sources;

    println!("{}", repro_contract as i32);
}
