struct Resource {
    id: i32,
    busy: bool,
}

fn available_id(pool: [Resource; 3]) -> i32 {
    if !pool[0].busy {
        pool[0].id
    } else if !pool[1].busy {
        pool[1].id
    } else {
        pool[2].id
    }
}

fn main() {
    let pool = [
        Resource { id: 10, busy: true },
        Resource { id: 20, busy: false },
        Resource { id: 30, busy: true },
    ];
    println!("{}", available_id(pool));
}
