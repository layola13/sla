async fn branch_a() -> i32 {
    11
}

async fn branch_b() -> i32 {
    22
}

async fn branch_c() -> i32 {
    33
}

#[tokio::main]
async fn main() {
    let winner = tokio::select! {
        biased;
        value = branch_a() => value,
        value = branch_b() => value,
        value = branch_c() => value,
    };

    println!("{}", winner);
}
