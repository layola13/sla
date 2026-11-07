struct GateState {
    arrived: i32,
    required: i32,
    drained: bool,
}

fn gate_code(state: GateState) -> i32 {
    if state.arrived < state.required {
        0
    } else if state.drained {
        2
    } else {
        1
    }
}

fn main() {
    println!("{}", gate_code(GateState { arrived: 3, required: 3, drained: false }));
}
