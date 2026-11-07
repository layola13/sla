use std::cell::Cell;

struct Guard<'a> {
    tally: &'a Cell<i32>,
    value: i32,
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        self.tally.set(self.tally.get() + self.value);
    }
}

fn compute() -> i32 {
    let tally = Cell::new(0);
    let outer = Guard { tally: &tally, value: 1 };

    let result = 'outer: loop {
        let left = Some(2);
        let right = Some(3);

        if let Some(x) = left && let Some(y) = right {
            let _inner = Guard { tally: &tally, value: 10 };
            let Some(sum) = Some(x + y) else {
                break 'outer 0;
            };
            break 'outer sum;
        } else {
            break 'outer 0;
        }
    };

    drop(outer);
    tally.get() + result
}

fn main() {
    println!("{}", compute());
}
