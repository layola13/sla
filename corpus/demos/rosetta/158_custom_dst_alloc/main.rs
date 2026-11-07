fn custom_dst_alloc_len_surrogate() -> usize {
    let owned = String::from("hey");
    owned.len()
}

fn main() {
    println!("{}", custom_dst_alloc_len_surrogate());
}
