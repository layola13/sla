fn main() {
    let index = include_str!("cache/remote/index.json");
    let state = include_str!("cache/remote/state.txt");
    let generated = include_str!("generated/remote/cache.sa");

    let remote_index_declares_backend = index.contains("\"backend\": \"sccache\"")
        && index.contains("\"bucket\": \"sa-asm-builds\"");
    let remote_state_matches_index = state.contains("backend=sccache")
        && state.contains("bucket=sa-asm-builds");
    let generated_mentions_remote_cache = generated.contains("cache/remote/index.json")
        && generated.contains("cache/remote/state.txt")
        && generated.contains("#def REMOTE_CACHE_COUNT = 2");
    let remote_cache_contract = remote_index_declares_backend && remote_state_matches_index && generated_mentions_remote_cache;

    println!("{}", remote_cache_contract as i32);
}
