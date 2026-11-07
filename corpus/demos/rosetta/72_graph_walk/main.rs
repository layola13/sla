struct Edge {
    from: i32,
    to: i32,
    weight: i32,
}

fn walk_cost(graph: [Edge; 2]) -> i32 {
    graph[0].from + graph[0].to + graph[0].weight + graph[1].to + graph[1].weight
}

fn main() {
    let graph = [
        Edge { from: 0, to: 1, weight: 3 },
        Edge { from: 1, to: 2, weight: 5 },
    ];
    println!("{}", walk_cost(graph));
}
