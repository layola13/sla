use std::ops::Neg;

#[derive(Copy, Clone, Debug)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        Vec3 {
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }
}

fn main() {
    let a = Vec3 { x: 1.0, y: -2.0, z: 3.0 };
    let b = -a;
    println!("({}, {}, {})", b.x, b.y, b.z);
    // 期望输出 (-1, 2, -3)
    assert!(b.x == -1.0 && b.y == 2.0 && b.z == -3.0);
}
