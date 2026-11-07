struct Task {
    deps_done: bool,
    priority: i32,
    retries: i32,
    cooldown: bool,
}

fn ready_score(task: Task) -> i32 {
    if task.deps_done {
        if task.cooldown { 0 } else { task.priority - task.retries }
    } else {
        0
    }
}

fn main() {
    let task = Task { deps_done: true, priority: 7, retries: 2, cooldown: false };
    println!("{}", ready_score(task));
}
