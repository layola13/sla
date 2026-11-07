// 306 - Or-Patterns
// Rust: match color { Red | Green | Blue => "primary", _ => "other" }
enum Color { Red, Green, Blue, Yellow, Cyan, Magenta }

fn is_primary(c: &Color) -> bool {
    matches!(c, Color::Red | Color::Green | Color::Blue)
}

fn main() {
    let colors = [Color::Red, Color::Yellow, Color::Blue, Color::Cyan];
    let count = colors.iter().filter(|c| is_primary(c)).count();
    println!("{count}");
}
