extern crate ggez;
extern crate rand;

mod generals;
use generals::*;

use std::time::Duration;

use ggez::conf;
use ggez::event::{self, MouseButton, Keycode, Mod};
use ggez::{GameResult, Context};
use ggez::graphics::{self, Color, DrawMode, Rect, Point, Drawable};

const CELL_SIZE: f32 = 48.0;

pub fn red() -> Color {
    Color::new(1.0, 0.1, 0.1, 1.0)
}
pub fn blue() -> Color {
    Color::new(0.1, 0.1, 1.0, 1.0)
}
pub fn black() -> Color {
    Color::new(0.0, 0.0, 0.0, 1.0)
}
pub fn black_overlay() -> Color {
    Color::new(0.0, 0.0, 0.0, 0.3)
}
pub fn red_overlay() -> Color {
    Color::new(1.0, 0.0, 0.0, 0.5)
}

fn team_color(team: Team) -> Color {
    match team {
        0 => red(),
        1 => blue(),
        _ => panic!("Missing team color for team {}", team),
    }
}

fn cell_color(cell: &Cell) -> Color {
    use generals::Cell::*;
    match *cell {
        Mountain => Color::new(0.2, 0.2, 0.2, 1.0),
        Open => Color::new(1.0, 1.0, 1.0, 1.0),
        Fortress(None, _) => Color::new(0.4, 0.4, 0.4, 1.0),

        Captured(team, _) |
        King(team, _) |
        Fortress(Some(team), _) => team_color(team),
    }
}

fn direction_from_keycode(keycode: Keycode) -> Direction {
    match keycode {
        Keycode::Up | Keycode::W => Direction::Up,
        Keycode::Down | Keycode::S => Direction::Down,
        Keycode::Left | Keycode::A => Direction::Left,
        Keycode::Right | Keycode::D => Direction::Right,
        _ => panic!("Not a valid direction: {:?}", keycode),
    }
}

struct MainState {
    font: graphics::Font,
    game: GameState,
    time: Duration,
    last_tick: Duration,
    tick_interval: Duration,

    team: usize,
    focus: Option<Position>
}

impl MainState {
    fn new(_ctx: &mut Context) -> GameResult<MainState> {
        let num_players = 2;
        let mut board = Board::empty(32);
        board.randomize(num_players);
        Ok(MainState {
            font: graphics::Font::default_font().unwrap(),
            time: Duration::new(0, 0),
            last_tick: Duration::new(0, 0),
            tick_interval: Duration::new(0, 500_000_000),
            team: 0,
            focus: None,
            game: GameState {
                board,
                tick_number: 0,
                num_players: 2,
                player_states: (0..num_players).map(PlayerState::new).collect(),
                dimens: (32, 32),
            },
        })
    }

    fn dimens(&self) -> (i32, i32) {
        self.game.dimens
    }
}

impl event::EventHandler for MainState {
    fn update(&mut self, _ctx: &mut Context, _dt: Duration) -> GameResult<()> {
        self.time += _dt;
        while self.time - self.last_tick > self.tick_interval {
            self.last_tick += self.tick_interval;
            self.game.tick();
        }

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        let board_size = self.game.dimens.0;
        graphics::clear(ctx);
        for (y, row) in self.game.board.cells().iter().enumerate() {
            for (x, cell) in row.iter().enumerate() {
                let (x, y) = (
                    x as f32 * (CELL_SIZE + 1.0) + CELL_SIZE / 2.0,
                    y as f32 * (CELL_SIZE + 1.0) + CELL_SIZE / 2.0,
                );
                let rect = Rect {
                    x,
                    y,
                    w: CELL_SIZE,
                    h: CELL_SIZE,
                };
                graphics::set_color(ctx, cell_color(&cell)).unwrap();
                graphics::rectangle(ctx, DrawMode::Fill, rect).unwrap();
                match *cell {
                    Cell::Fortress(_, n) |
                    Cell::King(_, n) |
                    Cell::Captured(_, n) => {
                        let t = graphics::Text::new(ctx, &format!("{}", n), &self.font).unwrap();
                        graphics::set_color(ctx, Color::new(0.0, 0.0, 0.0, 1.0)).unwrap();
                        t.draw(ctx, Point::new(x, y), 0.0).unwrap();
                    }
                    _ => {}
                }
            }
        }

        let player_state = &self.game.player_states[self.team];
        // Draw queued line
        graphics::set_color(ctx, black()).unwrap();
        for &(from_pos, dir) in player_state.moves.iter() {
            let Position(x, y) = from_pos;
            let (dx, dy) = dir.to_xy();
            let points = [
                Point {
                    x: x as f32 * (CELL_SIZE + 1.0) + CELL_SIZE / 2.0,
                    y: y as f32 * (CELL_SIZE + 1.0) + CELL_SIZE / 2.0,
                },
                Point {
                    x: (x + dx) as f32 * (CELL_SIZE + 1.0) + CELL_SIZE / 2.0,
                    y: (y + dy) as f32 * (CELL_SIZE + 1.0) + CELL_SIZE / 2.0,
                },
            ];
            graphics::line(ctx, &points).unwrap();
        }

        // Draw focus shade stuff
        if let Some(Position(x, y)) = self.focus {
            let (fx, fy) = (
                x as f32 * (CELL_SIZE + 1.0) + CELL_SIZE / 2.0,
                y as f32 * (CELL_SIZE + 1.0) + CELL_SIZE / 2.0,
            );
            let rect = Rect {
                x: fx,
                y: fy,
                w: CELL_SIZE,
                h: CELL_SIZE,
            };
            graphics::set_color(ctx, red_overlay()).unwrap();
            graphics::rectangle(ctx, DrawMode::Fill, rect).unwrap();

            let w = board_size as i32;
            let h = board_size as i32;
            for d in &[
                Direction::Up,
                Direction::Left,
                Direction::Right,
                Direction::Down,
            ]
            {
                if let Some((x, y)) = d.from((x, y), w, h) {
                    let (fx, fy) = (
                        x as f32 * (CELL_SIZE + 1.0) + CELL_SIZE / 2.0,
                        y as f32 * (CELL_SIZE + 1.0) + CELL_SIZE / 2.0,
                    );
                    let rect = Rect {
                        x: fx,
                        y: fy,
                        w: CELL_SIZE,
                        h: CELL_SIZE,
                    };
                    graphics::set_color(ctx, black_overlay()).unwrap();
                    graphics::rectangle(ctx, DrawMode::Fill, rect).unwrap();
                }
            }
        }

        graphics::present(ctx);
        Ok(())
    }

    fn mouse_button_down_event(&mut self, button: MouseButton, x: i32, y: i32) {
        if button != MouseButton::Left {
            return;
        }
        let ix = x / (CELL_SIZE + 1.0) as i32;
        let iy = y / (CELL_SIZE + 1.0) as i32;
        if let Some(cell) = self.game.board.try_get(ix, iy) {
            if cell.is_controlled_by(self.team) {
                self.focus = Some(Position(ix, iy));
            }
        }
    }

    fn key_down_event(&mut self, keycode: Keycode, _keymod: Mod, _repeat: bool) {
        match keycode {
            Keycode::Q => {
                self.game.player_mut(self.team).moves.clear();
            }
            Keycode::Up | Keycode::Down | Keycode::Left | Keycode::Right | Keycode::W |
            Keycode::A | Keycode::S | Keycode::D => {
                let dir = direction_from_keycode(keycode);
                let (w, h) = self.dimens();
                if let Some(ref mut pos) = self.focus {
                    let Position(x, y) = *pos;
                    let (dx, dy) = dir.to_xy();
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx >= 0 && nx < w && ny >= 0 && ny < h {
                        self.game.player_mut(self.team).moves.push_back((*pos, dir));
                        *pos = Position(nx, ny);
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn main() {
    let mut c = conf::Conf::new();
    c.window_height = 1600;
    c.window_width = 1600;
    let ctx = &mut Context::load_from_conf("GeNeRaLs", "martin", c).unwrap();
    let state = &mut MainState::new(ctx).unwrap();
    event::run(ctx, state).unwrap();
}
