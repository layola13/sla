fn main() {
    let hash = include_str!("crypto/hash.sa");
    let layout = include_str!("crypto/hash.sal");
    let docs = include_str!("docs/simd.md");
    let header = include_str!("host/include/crypto.h");
    let bench = include_str!("bench/hash_bench.txt");

    let hash_exports_entry = hash.contains("@ffi_wrapper crypto_hash_gate(*state: ptr) -> i32")
        && hash.contains("@export crypto_hash_entry() -> i32");
    let layout_defines_hash_state = layout.contains("#def HashState_round = +0")
        && layout.contains("#def HashState_lane = +4");
    let host_crypto_metadata = docs.contains("benchmark and public header")
        && header.contains("int crypto_hash_entry(void);")
        && bench.contains("rounds = 298");
    let crypto_contract = hash_exports_entry && layout_defines_hash_state && host_crypto_metadata;

    println!("{}", crypto_contract as i32);
}
