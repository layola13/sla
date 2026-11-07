struct SceneNode {
    visible: bool,
    weight: i32,
}

fn visible_weight(nodes: [SceneNode; 3]) -> i32 {
    let mut total = 0;
    for node in nodes {
        if node.visible {
            total += node.weight;
        }
    }
    total
}

fn main() {
    let nodes = [
        SceneNode { visible: true, weight: 4 },
        SceneNode { visible: false, weight: 5 },
        SceneNode { visible: true, weight: 6 },
    ];
    println!("{}", visible_weight(nodes));
}
