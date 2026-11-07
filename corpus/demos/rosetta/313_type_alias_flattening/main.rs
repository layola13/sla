#[derive(Clone, Copy)]
struct Transform {
    x: i32,
    y: i32,
}

#[derive(Clone, Copy)]
struct Velocity {
    vx: i32,
    vy: i32,
}

#[derive(Clone, Copy)]
struct BulletData {
    x: i32,
    y: i32,
    vx: i32,
    vy: i32,
    damage: i32,
}

fn spawn_bullet() -> i32 {
    let bullet = BulletData {
        x: Transform { x: 10, y: 20 }.x,
        y: Transform { x: 10, y: 20 }.y,
        vx: Velocity { vx: 30, vy: 40 }.vx,
        vy: Velocity { vx: 30, vy: 40 }.vy,
        damage: 50,
    };
    bullet.x + bullet.y + bullet.vx + bullet.vy + bullet.damage
}

fn main() {
    println!("{}", spawn_bullet());
}
