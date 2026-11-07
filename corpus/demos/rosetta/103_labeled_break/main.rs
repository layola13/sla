fn main() {
    let mut outer = 0;
    let mut total = 0;

    'outer: loop {
        outer += 1;

        let mut inner = 0;
        loop {
            inner += 1;

            if outer == 2 && inner == 2 {
                break 'outer;
            }

            total += outer + inner;

            if inner >= 3 {
                break;
            }
        }

        if outer >= 3 {
            break;
        }
    }

    println!("{total}");
}
