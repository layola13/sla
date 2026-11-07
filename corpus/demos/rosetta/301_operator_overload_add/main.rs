use std::ops::Add;

#[derive(Copy, Clone, Debug)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, other: Vec3) -> Vec3 {
        Vec3 {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }
}

fn main() {
    let a = Vec3 { x: 1.0, y: 2.0, z: 3.0 };
    let b = Vec3 { x: 4.0, y: 5.0, z: 6.0 };
    let c = a + b;
    println!("({}, {}, {})", c.x, c.y, c.z);
    // 期望输出 (5, 7, 9)
    assert!(c.x == 5.0 && c.y == 7.0 && c.z == 9.0);
}
