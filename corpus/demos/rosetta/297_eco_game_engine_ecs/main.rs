fn main() {
    let world = include_str!("engine/world.sa");
    let layout = include_str!("engine/world.sal");
    let docs = include_str!("docs/ecs.md");
    let assets = include_str!("host/scene/assets.list");
    let scene = include_str!("host/scene/world.scene.txt");

    let world_steps_entity = world.contains("@step_entity(&entity: ptr) -> i32")
        && world.contains("store entity+Entity_X, next_x as i32")
        && world.contains("@export engine_scene_step() -> i32");
    let layout_defines_ecs_components = layout.contains("#def Entity_X = +0")
        && layout.contains("#def Entity_Y = +4")
        && layout.contains("#def Entity_VX = +8")
        && layout.contains("#def Entity_VY = +12");
    let scene_metadata = docs.contains("layout, step logic, and scene assets")
        && assets.contains("entity_sprite.png")
        && assets.contains("level0.json")
        && scene.contains("world = engine/world.sa");
    let ecs_contract = world_steps_entity && layout_defines_ecs_components && scene_metadata;

    println!("{}", ecs_contract as i32);
}
