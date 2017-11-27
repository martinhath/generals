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

fn cell_color(cell: &Cell) -> Color {
    use generals::Cell::*;
    use generals::Team;
    match *cell {
        Mountain => Color::new(0.2, 0.2, 0.2, 1.0),
        Open => Color::new(1.0, 1.0, 1.0, 1.0),
        Fortress(None, _) => Color::new(0.4, 0.4, 0.4, 1.0),
        Captured(Team::Red, _) |
        King(Team::Red, _) |
        Fortress(Some(Team::Red), _) => red(),
        Captured(Team::Blue, _) |
        King(Team::Blue, _) |
        Fortress(Some(Team::Blue), _) => blue(),
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

    current_team: usize,
    focus: Option<Position>
}

impl MainState {
    fn new(_ctx: &mut Context) -> GameResult<MainState> {
        let mut board = Board::empty(32);
        board.randomize();
        Ok(MainState {
            font: graphics::Font::default_font().unwrap(),
            time: Duration::new(0, 0),
            last_tick: Duration::new(0, 0),
            tick_interval: Duration::new(0, 500_000_000),
            current_team: 0,
            focus: None,
            game: GameState {
                board,
                tick_number: 0,
                num_players: 2,
                player_states: vec![PlayerState::new(), PlayerState::new()],
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

        let player_state = self.game.player_states[self.current_team];
        // Draw queued line
        let mut current_pos = player_state.movement;
        graphics::set_color(ctx, black()).unwrap();
        for m in player_state.moves.iter() {
            match *m {
                Move::Focus(coords) => current_pos = Some(coords),
                Move::Movement(dir) => {
                    let (x, y) = current_pos.unwrap();
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
                    current_pos = Some((x + dx, y + dy));
                }
            }
        }

        // Draw focus shade stuff
        if let Some((x, y)) = player_state.focus {
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
        self.game.focus((ix, iy), self.current_team);
    }

    fn key_down_event(&mut self, keycode: Keycode, _keymod: Mod, _repeat: bool) {
        match keycode {
            Keycode::Q => {
                self.game.reset_player_focus(self.current_team);
            }
            Keycode::Up | Keycode::Down | Keycode::Left | Keycode::Right | Keycode::W |
            Keycode::A | Keycode::S | Keycode::D => {
                let dir = direction_from_keycode(keycode);
                let (w, h) = self.dimens();
                if let Some((ref mut x, ref mut y)) = self.game.player_states[self.current_team].focus {
                    let (dx, dy) = dir.to_xy();
                    let nx = *x + dx;
                    let ny = *y + dy;
                    if nx >= 0 && nx < w && ny >= 0 && ny < h {
                        self.game.player_states[self.current_team].moves.push_back(Move::Movement(dir));
                        *x = nx;
                        *y = ny;
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
