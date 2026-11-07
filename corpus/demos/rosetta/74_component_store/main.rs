struct Position {
    entity: i32,
    x: i32,
    y: i32,
}

fn component_sum(store: [Position; 2]) -> i32 {
    store[1].entity + store[1].x + store[1].y
}

fn main() {
    let positions = [
        Position { entity: 1, x: 1, y: 2 },
        Position { entity: 2, x: 3, y: 4 },
    ];
    println!("{}", component_sum(positions));
}
