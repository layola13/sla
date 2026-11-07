use std::rc::Rc;

fn main() {
    let value = Rc::new(5);
    let clone = value.clone();
    println!("{}", *value + *clone);
}
