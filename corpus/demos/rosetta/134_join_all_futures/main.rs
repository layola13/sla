async fn job_a() -> i32 {
    1
}

async fn job_b() -> i32 {
    2
}

async fn job_c() -> i32 {
    3
}

#[tokio::main]
async fn main() {
    let a = job_a().await;
    let b = job_b().await;
    let c = job_c().await;
    println!("{}", a + b + c);
}
