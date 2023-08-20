mod app;

fn main() {
    pollster::block_on(app::run());
}