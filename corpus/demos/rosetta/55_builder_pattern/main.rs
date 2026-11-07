struct Request {
    method: &'static str,
    path: &'static str,
}

struct Builder {
    method: &'static str,
    path: &'static str,
}

impl Builder {
    fn new() -> Self {
        Self { method: "GET", path: "/" }
    }

    fn method(mut self, method: &'static str) -> Self {
        self.method = method;
        self
    }

    fn path(mut self, path: &'static str) -> Self {
        self.path = path;
        self
    }

    fn build(self) -> Request {
        Request { method: self.method, path: self.path }
    }
}

fn main() {
    let req = Builder::new().method("POST").path("/api").build();
    println!("{} {}", req.method, req.path);
}
