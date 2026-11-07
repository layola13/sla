use std::cell::RefCell;

fn main() {
    let value = RefCell::new(7);
    {
        let borrowed = value.borrow();
        println!("{}", *borrowed);
    }
    *value.borrow_mut() = 9;
    println!("{}", value.borrow());
}
