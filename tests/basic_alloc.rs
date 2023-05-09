use the_world_allocator::TheWorld;

#[global_allocator]
static ALLOCATOR: TheWorld = TheWorld;

fn main() {
    //let s: String = "hello".to_string();
    //assert_eq!(0, 0)
}
