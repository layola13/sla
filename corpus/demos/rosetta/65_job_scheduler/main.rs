struct Job {
    ready: bool,
    cost: i32,
}

fn scheduled_cost(jobs: [Job; 4]) -> i32 {
    let mut done = 0;
    for job in jobs {
        if job.ready {
            done += job.cost;
        }
    }
    done
}

fn main() {
    let jobs = [
        Job { ready: true, cost: 1 },
        Job { ready: false, cost: 2 },
        Job { ready: true, cost: 3 },
        Job { ready: true, cost: 4 },
    ];
    println!("{}", scheduled_cost(jobs));
}
