mod math {
    pub fn double(value: i32) -> i32 {
        return value + value;
    }

    pub fn scale(value: i32, factor: i32) -> i32 {
        return value * factor;
    }
}

use math;

struct Meter {
    value: i32,
}

impl Meter {
    fn double(self) -> i32 {
        return self.value.double();
    }

    fn scale(self, factor: i32) -> i32 {
        return self.value.scale(factor);
    }
}

fn main() {
    let m = Meter { value: 7 };
    assert!(m.double() == 14);
    assert!(m.scale(3) == 21);
}
