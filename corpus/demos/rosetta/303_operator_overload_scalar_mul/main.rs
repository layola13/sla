use std::ops::Mul;

#[derive(Copy, Clone, Debug)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Mul<f32> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f32) -> Vec3 {
        Vec3 {
            x: self.x * s,
            y: self.y * s,
            z: self.z * s,
        }
    }
}

fn main() {
    let a = Vec3 { x: 1.0, y: 2.0, z: 3.0 };
    let b = a * 4.0;
    println!("({}, {}, {})", b.x, b.y, b.z);
    // 期望输出 (4, 8, 12)
    assert!(b.x == 4.0 && b.y == 8.0 && b.z == 12.0);
}
