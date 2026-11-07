fn token_score(tokens: [&str; 4]) -> i32 {
    let mut score = 0;
    for token in tokens {
        score += match token {
            "let" => 10,
            "=" => 3,
            value => value.len() as i32,
        };
    }
    score
}

fn main() {
    let tokens = ["let", "x", "=", "1"];
    println!("{}", token_score(tokens));
}
