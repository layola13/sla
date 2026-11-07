trait Provider {
    type Item<'a>;
    fn get<'a>(&'a self) -> Self::Item<'a>;
}

struct IntProvider { val: i32 }
impl Provider for IntProvider {
    type Item<'a> = &'a i32;
    fn get<'a>(&'a self) -> &'a i32 {
        &self.val
    }
}

fn main() {
    let p = IntProvider { val: 42 };
    let item = p.get();
    println!("{}", item);
}
