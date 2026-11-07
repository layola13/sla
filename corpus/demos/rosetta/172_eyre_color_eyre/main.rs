fn render_context_line(message: &str) -> String {
    format!("context: {}", message)
}

fn main() {
    let context = render_context_line("boom");
    println!("{}", context.len());
}
