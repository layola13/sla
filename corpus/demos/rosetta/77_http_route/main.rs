struct Request<'a> {
    method: &'a str,
    path: &'a str,
}

fn route_status(req: &Request<'_>) -> i32 {
    match (req.method, req.path) {
        ("GET", "/health") => 200,
        ("POST", "/jobs") => 202,
        _ => 404,
    }
}

fn main() {
    let request = Request { method: "POST", path: "/jobs" };
    println!("{}", route_status(&request));
}
