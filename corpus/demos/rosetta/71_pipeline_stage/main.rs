struct StageInput {
    raw: i32,
    scale: i32,
    offset: i32,
}

fn pipeline_stage_value(input: StageInput) -> i32 {
    let stage1 = input.raw * input.scale;
    let stage2 = stage1 + input.offset;
    stage2
}

fn main() {
    let input = StageInput { raw: 2, scale: 3, offset: 1 };
    println!("{}", pipeline_stage_value(input));
}
