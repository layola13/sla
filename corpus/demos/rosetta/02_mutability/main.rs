fn main() {
    let mut x = 10;

    {
        let r1 = &x;
        let _before = *r1;
    }

    {
        let r2 = &mut x;
        *r2 = 20;
    }

    println!("{x}");
}
