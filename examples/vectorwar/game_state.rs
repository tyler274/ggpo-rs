/*
* Encapsulates all the game state for the vector war application inside
* a single structure.  This makes it trivial to implement our GGPO
* save and load functions.
*/

use crate::vectorwar::Input;
use enumflags2::BitFlags;
use ggpo::game_input::FrameNum;
use log::info;
use std::f64::consts::PI;

pub const STARTING_HEALTH: i32 = 100;
pub const ROTATE_INCREMENT: f64 = 3.;
pub const SHIP_RADIUS: f64 = 15.;
pub const SHIP_WIDTH: f64 = 8.;
pub const SHIP_TUCK: f64 = 3.;
pub const SHIP_THRUST: f64 = 0.06;
pub const SHIP_MAX_THRUST: f64 = 4.0;
pub const SHIP_BREAK_SPEED: f64 = 0.6;
pub const BULLET_SPEED: f64 = 5.;
pub const MAX_BULLETS: usize = 30;
pub const BULLET_COOLDOWN: f64 = 8.;
pub const BULLET_DAMAGE: f64 = 10.;

pub const MAX_SHIPS: usize = 4;

#[derive(Copy, Clone)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

impl Position {
    pub const fn new() -> Self {
        Self { x: 0., y: 0. }
    }
    pub fn distance(&self, rhs: &Position) -> f64 {
        let x = rhs.x - self.x;
        let y = rhs.y - self.y;
        ((x * x) + (y * y)).sqrt()
    }
}

#[derive(Copy, Clone)]
pub struct Velocity {
    pub dx: f64,
    pub dy: f64,
}

impl Velocity {
    pub const fn new() -> Self {
        Self { dx: 0., dy: 0. }
    }
}

#[derive(Copy, Clone)]
pub struct Bullet {
    pub active: bool,
    pub position: Position,
    pub velocity: Velocity,
}

impl Bullet {
    pub const fn new() -> Self {
        Self {
            active: false,
            position: Position::new(),
            velocity: Velocity::new(),
        }
    }
}

#[derive(Copy, Clone)]
pub struct Ship {
    pub position: Position,
    pub velocity: Velocity,
    pub radius: u32,
    pub heading: i32,
    pub health: i32,
    pub speed: i32,
    pub cooldown: i32,
    pub bullets: [Bullet; MAX_BULLETS],
    pub score: i32,
}

impl Ship {
    pub const fn new() -> Self {
        Self {
            position: Position::new(),
            velocity: Velocity::new(),
            radius: 0,
            heading: 0,
            health: 0,
            speed: 0,
            cooldown: 0,
            bullets: [Bullet::new(); MAX_BULLETS],
            score: 0,
        }
    }
}

pub struct GameState {
    pub frame_number: FrameNum,
    // pub bounds: Rect,
    pub num_ships: u32,
    pub ships: [Ship; MAX_SHIPS],
}

impl GameState {
    pub fn new() -> Self {
        Self {
            frame_number: 0,
            // bounds: Rect::new(0, 0, 0, 0),
            num_ships: 0,
            ships: [Ship::new(); MAX_SHIPS],
        }
    }
    pub fn init(&mut self, num_players: u32, width: u32, height: u32) {
        // self.bounds.set_width(width);
        // self.bounds.set_height(height);

        let r = height / 4;

        self.frame_number = 0;
        self.num_ships = num_players;
        for i in 0..self.num_ships as usize {
            let heading = i as u32 * 360 / num_players;
            let theta = heading as f64 * PI / 180.;

            let (sint, cost) = theta.sin_cos();

            self.ships[i].position.x = (width as f64 / 2.) + r as f64 * cost;
            self.ships[i].position.y = (height as f64 / 2.) + r as f64 * sint;
            self.ships[i].heading = ((heading as f64 + 180.) % 360.) as i32;
            self.ships[i].health = STARTING_HEALTH;
            self.ships[i].radius = SHIP_RADIUS as u32;
        }
    }
    pub fn get_ship_ai(&self, i: usize, heading: &mut f64, thrust: &mut f64, fire: &mut bool) {
        *heading = ((self.ships[i].heading + 5) % 360) as f64;
        *thrust = 0.;
        *fire = false;
    }
    pub fn parse_ship_inputs(
        &self,
        inputs: BitFlags<Input>,
        i: usize,
        heading: &mut f64,
        thrust: &mut f64,
        fire: &mut bool,
    ) {
        let ship = &self.ships[i];
        info!("parsing ship {} input: {:#?}.\n", i, inputs);

        if inputs.contains(Input::RotateRight) {
            *heading = (ship.heading as f64 + ROTATE_INCREMENT) % 360.;
        } else if inputs.contains(Input::RotateLeft) {
            *heading = (ship.heading as f64 - ROTATE_INCREMENT + 360.) % 360.;
        } else {
            *heading = ship.heading as f64;
        }

        if inputs.contains(Input::Thrust) {
            *thrust = SHIP_THRUST;
        } else if inputs.contains(Input::Break) {
            *thrust = -SHIP_THRUST;
        } else {
            *thrust = 0.;
        }

        *fire = inputs.contains(Input::Fire);
    }
    pub fn move_ship(&mut self, i: usize, heading: f64, thrust: f64, fire: bool) {
        let mut ship = self.ships[i];

        info!(
            "calculation of new ship coordinates: (thrust:{:.4} heading:{:.4}).\n",
            thrust, heading
        );

        ship.heading = heading as i32;

        if ship.cooldown == 0 {
            if fire {
                info!("firing bullet.\n");
                for i in 0..MAX_BULLETS {
                    let (dy, dx) = (ship.heading as f64).to_radians().sin_cos();
                    let bullet = &mut ship.bullets[i];
                    if !bullet.active {
                        bullet.active = true;
                        bullet.position.x = ship.position.x + (ship.radius as f64 * dx);
                        bullet.position.y = ship.position.y + (ship.radius as f64 * dy);
                        bullet.velocity.dx = ship.velocity.dx + (BULLET_SPEED * dx);
                        bullet.velocity.dy = ship.velocity.dy + (BULLET_SPEED * dy);
                        ship.cooldown = BULLET_COOLDOWN as i32;
                        break;
                    }
                }
            }
        }

        if thrust > 0. {
            let (dy, dx) = (ship.heading as f64).to_radians().sin_cos();
            let (dx, dy) = (dx * thrust, dy * thrust);

            ship.velocity.dx += dx;
            ship.velocity.dy += dy;

            let mag: f64 =
                (ship.velocity.dx * ship.velocity.dx + ship.velocity.dy * ship.velocity.dy).sqrt();

            if mag > SHIP_MAX_THRUST {
                ship.velocity.dx = (ship.velocity.dx * SHIP_MAX_THRUST) / mag;
                ship.velocity.dy = (ship.velocity.dy * SHIP_MAX_THRUST) / mag;
            }
            info!(
                "new ship velocity: (dx:{:.4} dy:{:.2}).\n",
                ship.velocity.dx, ship.velocity.dy
            );

            ship.position.x += ship.velocity.dx;
            ship.position.y += ship.velocity.dy;
            info!(
                "new ship position: (dx:{:.4} dy:{:.2}).\n",
                ship.position.x, ship.position.y
            );

            // if ship.position.x - (ship.radius as f64) < self.bounds.y() as f64
            //     || ship.position.y + ship.radius as f64
            //         > self.bounds.y() as f64 + self.bounds.height() as f64
            // {
            //     ship.velocity.dx *= -1.;
            //     ship.position.x += ship.velocity.dx * 2.;
            // }

            // if ship.position.y - (ship.radius as f64) < self.bounds.y() as f64
            //     || ship.position.y + (ship.radius as f64)
            //         > self.bounds.y() as f64 + self.bounds.height() as f64
            // {
            //     ship.velocity.dy *= -1.;
            //     ship.position.y += ship.velocity.dy * 2.;
            // }

            // for i in 0..MAX_BULLETS {
            //     let bullet = &mut ship.bullets[i];
            //     if bullet.active {
            //         bullet.position.x += bullet.velocity.dx;
            //         bullet.position.y += bullet.velocity.dy;
            //         if bullet.position.x < self.bounds.x() as f64
            //             || bullet.position.y < self.bounds.x() as f64
            //             || bullet.position.x > self.bounds.x() as f64 + self.bounds.width() as f64
            //             || bullet.position.y > self.bounds.y() as f64 + self.bounds.height() as f64
            //         {
            //             bullet.active = false;
            //         } else {
            //             for j in 0..self.num_ships as usize {
            //                 if bullet.position.distance(&self.ships[j].position)
            //                     < self.ships[j].radius as f64
            //                 {
            //                     ship.score += 1;
            //                     self.ships[j].health -= BULLET_DAMAGE as i32;
            //                     bullet.active = false;
            //                     break;
            //                 }
            //             }
            //         }
            //     }
            // }
        }
        self.ships[i] = ship;
    }
    pub fn update(&mut self, inputs: &[BitFlags<Input>], disconnect_flags: i32) {
        self.frame_number += 1;
        for i in 0..self.num_ships as usize {
            let (mut thrust, mut heading, mut fire): (f64, f64, bool) = (0., 0., false);
            if (disconnect_flags & ((i as i32) << i)) > 0 {
                self.get_ship_ai(i, &mut heading, &mut thrust, &mut fire);
            } else {
                self.parse_ship_inputs(inputs[i], i, &mut heading, &mut thrust, &mut fire);
            }

            self.move_ship(i, heading, thrust, fire);

            if self.ships[i].cooldown > 0 {
                self.ships[i].cooldown -= 1;
            }
        }
    }
}
