struct SubmissionQueue {
    depth: usize,
    submitted: usize,
}

impl SubmissionQueue {
    fn submit(&mut self) {
        if self.submitted < self.depth {
            self.submitted += 1;
        }
    }
}

fn main() {
    let mut queue = SubmissionQueue {
        depth: 2,
        submitted: 0,
    };
    queue.submit();
    queue.submit();
    queue.submit();
    println!("{}", queue.submitted);
}
