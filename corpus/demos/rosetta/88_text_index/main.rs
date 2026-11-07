fn postings_for(term: &str) -> i32 {
    match term {
        "rust" => 2,
        "sla" => 3,
        _ => 0,
    }
}

fn main() {
    println!("{}", postings_for("sla"));
}
