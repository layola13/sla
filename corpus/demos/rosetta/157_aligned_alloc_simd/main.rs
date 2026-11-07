fn aligned_simd_lane_count_surrogate() -> usize {
    let alignment_bytes = 16usize;
    let lane_bytes = std::mem::size_of::<i32>();
    alignment_bytes / lane_bytes
}

fn main() {
    println!("{}", aligned_simd_lane_count_surrogate());
}
