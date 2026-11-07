enum Event {
    Tick(i32),
    Reset,
    TickAfterReset(i32),
}

fn run_loop(events: [Event; 4]) -> i32 {
    let mut acc = 0;
    for event in events {
        match event {
            Event::Tick(value) => acc += value,
            Event::Reset => acc = 0,
            Event::TickAfterReset(value) => acc += value,
        }
    }
    acc
}

fn main() {
    let events = [Event::Tick(2), Event::Tick(3), Event::Reset, Event::TickAfterReset(4)];
    println!("{}", run_loop(events));
}
