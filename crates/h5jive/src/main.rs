mod app;
mod core;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    app::run()
}
