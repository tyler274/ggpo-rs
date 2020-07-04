use crate::game_state;
use crate::game_state::{GameState, Ship};
use crate::non_game_state::{ChecksumInfo, NonGameState, PlayerConnectState, PlayerConnectionInfo};
use ggpo::player::PlayerType;
use std::f64::consts::PI;
// use crate::resource_manager::{FontDetails, FontManager, ResourceManager, TextureManager};
use log::error;
use sdl2::{
    pixels::Color,
    rect::{Point, Rect},
    render::{Canvas, RendererContext, Texture, TextureCreator},
    ttf::Font,
    video::{Window, WindowContext},
    Sdl, VideoSubsystem,
};
// use typed_arena::Arena;

// handle the annoying Rect i32
macro_rules! rect(
    ($x:expr, $y:expr, $w:expr, $h:expr) => (
        Rect::new($x as i32, $y as i32, $w as u32, $h as u32)
    )
);

const PROGRESS_BAR_WIDTH: i32 = 100;
const PROGRESS_BAR_TOP_OFFSET: i32 = 22;
const PROGRESS_BAR_HEIGHT: i32 = 8;
const PROGRESS_TEXT_OFFSET: i32 = PROGRESS_BAR_TOP_OFFSET + PROGRESS_BAR_HEIGHT + 4;

pub struct SDLRenderer {
    _rc: Rect,
    _status: String,
    ship_colors: [Color; 4],
    white: Color,
    // used for bullets
    yellow: Color,
}

impl<'a> SDLRenderer {
    pub fn new(canvas: &Canvas<Window>) -> Self {
        let rectangle = Rect::new(0, 0, canvas.window().size().0, canvas.window().size().1);
        Self {
            // texture_arena,
            // texture_manager: texture_manager.clone(),
            // font_manager,
            // font: font_texture,
            yellow: Color::RGB(255, 255, 0),
            white: Color::RGB(255, 255, 255),
            ship_colors: [
                Color::RGB(255, 0, 0),
                Color::RGB(0, 255, 0),
                Color::RGB(0, 0, 255),
                Color::RGB(128, 128, 128),
            ],
            _status: String::from('\0'),
            _rc: Rect::new(0, 0, canvas.window().size().0, canvas.window().size().1),
        }
    }
    pub fn create_font() {}
    pub fn draw(
        &mut self,
        canvas: &mut Canvas<Window>,
        font: &Font,
        texture_creator: &TextureCreator<Window>,
        game_state: &GameState,
        non_game_state: &NonGameState,
    ) -> Result<(), String> {
        canvas.set_draw_color(Color::RGBA(0, 0, 0, 255));
        canvas.clear();

        for i in 0..game_state.num_ships as usize {
            let color = self.ship_colors[i];
            canvas.set_draw_color(color);

            self.draw_ship(canvas, font, texture_creator, i, game_state);
            self.draw_connect_state(
                canvas,
                font,
                texture_creator,
                &game_state.ships[i],
                &non_game_state.players[i],
                &self.ship_colors[i],
            );
        }

        let (width, height) = self._rc.size();
        // more or less centered
        let mut dst: Point = Point::new(((width / 2) - 80) as i32, (height - 32) as i32);
        self.draw_text(
            canvas,
            font,
            texture_creator,
            &self._status,
            &mut dst,
            &self.white,
        )
        .map_err(|e| e.to_string())?;

        let mut col: Color = Color::RGB(192, 192, 192);
        self.render_checksum(
            canvas,
            font,
            texture_creator,
            40,
            &non_game_state.periodic,
            &col,
        )?;

        col = Color::RGB(128, 128, 128);
        self.render_checksum(canvas, font, texture_creator, 56, &non_game_state.now, &col)?;

        canvas.present();

        Ok(())
    }

    pub fn draw_text(
        &self,
        canvas: &mut Canvas<Window>,
        font: &Font,
        texture_creator: &TextureCreator<Window>,
        text: &str,
        dst: &mut Point,
        color: &Color,
    ) -> Result<(), String> {
        let render = font
            .render(text)
            .blended_wrapped(*color, 48)
            .map_err(|e| e.to_string())?;
        let dst_rect: Rect = Rect::new(
            dst.x(),
            dst.y(),
            render.rect().width(),
            render.rect().height(),
        );
        canvas
            .copy(
                &render
                    .as_texture(texture_creator)
                    .map_err(|e| e.to_string())?,
                render.rect(),
                dst_rect,
            )
            .map_err(|e| e.to_string())?;

        dst.offset(dst.x() + render.rect().width() as i32, 0);
        Ok(())
    }

    pub fn draw_ship(
        &mut self,
        canvas: &mut Canvas<Window>,
        font: &Font,
        texture_creator: &TextureCreator<Window>,
        which: usize,
        game_state: &GameState,
    ) -> Result<(), String> {
        let ship: &Ship = &game_state.ships[which];
        let mut bullet = Rect::new(0, 0, 2, 2);

        let mut shape: [Point; 5] = [
            Point::new(game_state::SHIP_RADIUS as i32, 0),
            Point::new(
                -game_state::SHIP_RADIUS as i32,
                game_state::SHIP_WIDTH as i32,
            ),
            Point::new((game_state::SHIP_TUCK - game_state::SHIP_RADIUS) as i32, 0),
            Point::new(
                -game_state::SHIP_RADIUS as i32,
                -game_state::SHIP_WIDTH as i32,
            ),
            Point::new(game_state::SHIP_RADIUS as i32, 0),
        ];

        // TODO: Magic numbers
        let alignment_adjustment: [i32; 4] = [-5, 65, -5, 65];

        let (x, y, w, h) = (
            game_state.bounds.x(),
            game_state.bounds.y(),
            game_state.bounds.width() as i32,
            game_state.bounds.height() as i32,
        );
        let text_offsets: [Point; 4] = [
            Point::new(x + 2, y + 2),
            Point::new(x + w - 2, y + 2),
            Point::new(x + 2, y + h - 20),
            Point::new(x + w - 2, y + h - 20),
        ];

        for i in 0..shape.len() {
            let (newx, newy, theta): (i32, i32, f64);
            theta = ship.heading as f64 * PI / 180.;
            let (sint, cost) = theta.sin_cos();

            newx = shape[i].x() * cost as i32 - shape[i].y() * sint as i32;
            newy = shape[i].x() * sint as i32 + shape[i].y() * cost as i32;

            shape[i].offset(newx, newy);
        }
        canvas.draw_lines(shape.as_ref())?;

        canvas.set_draw_color(self.yellow);

        for i in 0..game_state::MAX_BULLETS {
            if ship.bullets[i].active {
                bullet.set_x((ship.bullets[i].position.x - 1.) as i32);
                bullet.set_y((ship.bullets[i].position.y - 1.) as i32);
                canvas.fill_rect(bullet)?;
            }
        }

        let buf = format!("Hits: {}", ship.score);

        let mut dst = Point::new(
            text_offsets[which].x - alignment_adjustment[which],
            text_offsets[which].y,
        );
        self.draw_text(
            canvas,
            font,
            texture_creator,
            &buf,
            &mut dst,
            &self.ship_colors[which],
        )?;

        Ok(())
    }
    pub fn draw_connect_state(
        &self,
        canvas: &mut Canvas<Window>,
        font: &Font,
        texture_creator: &TextureCreator<Window>,
        ship: &Ship,
        info: &PlayerConnectionInfo,
        color: &Color,
    ) -> Result<(), String> {
        let status: &str;
        let mut progress: i32 = -1;
        let status = match info.state {
            PlayerConnectState::Connecting => match info._type {
                PlayerType::Local => "Local Player",
                _ => "Connecting...",
            },
            PlayerConnectState::Synchronizing => {
                progress = info.connect_progress;
                match info._type {
                    PlayerType::Local => "Local Player",
                    _ => "Synchronizing...",
                }
            }
            PlayerConnectState::Disconnected => "Disconnected",
            PlayerConnectState::Disconnecting => {
                progress = ((info.disconnect_start.elapsed().as_millis()) * 100
                    / info.disconnect_timeout.as_millis()) as i32;
                "Waiting for player..."
            }
            PlayerConnectState::Running => "\0",
        };
        self.draw_text(
            canvas,
            font,
            texture_creator,
            status,
            &mut Point::new(
                ship.position.x as i32 - 40,
                ship.position.y as i32 + PROGRESS_TEXT_OFFSET,
            ),
            color,
        )?;

        if progress >= 0 {
            let mut rc = Rect::new(
                ship.position.x as i32 - (PROGRESS_BAR_WIDTH / 2),
                ship.position.y as i32 + PROGRESS_BAR_TOP_OFFSET,
                PROGRESS_BAR_WIDTH as u32 / 2,
                (PROGRESS_BAR_TOP_OFFSET + PROGRESS_BAR_HEIGHT) as u32,
            );
            canvas.draw_rect(rc)?;
            rc.set_width((rc.x() + std::cmp::min(100, progress) * PROGRESS_BAR_WIDTH / 100) as u32);
            canvas.fill_rect(rc)?;
        }

        Ok(())
    }
    pub fn render_checksum(
        &self,
        canvas: &mut Canvas<Window>,
        font: &Font,
        texture_creator: &TextureCreator<Window>,
        y: i32,
        info: &ChecksumInfo,
        color: &Color,
    ) -> Result<(), String> {
        let checksum_str = format!(
            "Frame: {:04}  Checksum: {:#08x}",
            info.frame_number.unwrap(),
            info.checksum
        );

        self.draw_text(
            canvas,
            font,
            texture_creator,
            &checksum_str,
            &mut Point::new(((self._rc.width() / 2) - 120) as i32, y),
            color,
        )?;
        Ok(())
    }
}

fn create_main_window() -> (Sdl, VideoSubsystem, Window) {
    let sdl_context = match sdl2::init() {
        Ok(context) => context,
        Err(error) => {
            error!("Error (SDL): could not initialise SDL: {}\n", error);
            std::process::exit(1);
        }
    };
    let video_subsystem = match sdl_context.video() {
        Ok(subsystem) => subsystem,
        Err(error) => {
            error!(
                "Error (SDL): could not initialise SDL's Video Subsystem: {}\n",
                error
            );
            std::process::exit(1);
        }
    };

    let window = match video_subsystem
        .window(
            &format!("(pid: {}) ggpo sdk sample: vector war", std::process::id()),
            640,
            480,
        )
        .position_centered()
        .build()
    {
        Ok(window) => window,
        Err(error) => {
            error!("Error (SDL): could not init Window: {}\n", error);
            std::process::exit(1);
        }
    };
    (sdl_context, video_subsystem, window)
}

pub fn print_sdl_error(location: &str) {
    error!("SDL Error ({}): {}\n", location, sdl2::get_error());
}
