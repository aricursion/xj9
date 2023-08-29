mod app;
mod texture;


fn main() {
    pollster::block_on(app::run());
}