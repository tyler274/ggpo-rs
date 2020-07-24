extern crate ggpo;

mod constants;
mod game_state;
mod non_game_state;
// mod resource_manager;
// mod sdl_renderer;
mod vectorwar;

use std::path::Path;
use std::time::Duration;

static SCREEN_WIDTH: u32 = 640;
static SCREEN_HEIGHT: u32 = 480;

pub fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();

    Ok(())
}
