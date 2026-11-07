fn main() {
    let config = include_str!("build/cache.toml");
    let index = include_str!("cache/index.json");
    let hashes = include_str!("cache/hashes.txt");
    let generated = include_str!("generated/cache/state.sa");

    let config_selects_incremental = config.contains("strategy = \"incremental\"")
        && config.contains("index = \"cache/index.json\"");
    let cache_records_hashes = index.contains("\"packages\": [\"core\", \"extras\"]")
        && index.contains("cache/hashes.txt")
        && hashes.contains("core=9f4d")
        && hashes.contains("extras=18ac");
    let generated_counts_cache_entries = generated.contains("build/cache.toml")
        && generated.contains("cache/index.json")
        && generated.contains("#def CACHE_ENTRY_COUNT = 2");
    let cache_contract = config_selects_incremental && cache_records_hashes && generated_counts_cache_entries;

    println!("{}", cache_contract as i32);
}
