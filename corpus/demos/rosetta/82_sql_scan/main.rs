#[derive(Copy, Clone)]
struct Row {
    active: bool,
    age: i32,
}

fn qualifies(row: Row) -> bool {
    row.active && row.age >= 18
}

fn main() {
    let rows = [Row { active: true, age: 21 }, Row { active: false, age: 40 }, Row { active: true, age: 17 }];
    let score: i32 = rows
        .into_iter()
        .filter(|row| qualifies(*row))
        .map(|row| row.age)
        .sum();
    println!("{score}");
}
