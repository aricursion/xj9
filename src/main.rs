mod app;

fn main() {
    println!("Hello, world!");
    pollster::block_on(app::run());
}