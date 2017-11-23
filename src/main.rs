extern crate ggez;
extern crate rand;

use std::collections::VecDeque;

use rand::Rng;
use rand::distributions::{Weighted, WeightedChoice, IndependentSample};

use ggez::conf;
use ggez::event::{self, MouseButton, Keycode, Mod};
use ggez::{GameResult, Context};
use ggez::graphics::{self, Color, DrawMode, Rect, Point, Drawable};
use std::time::Duration;

const CELL_SIZE: f32 = 48.0;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Team {
    Blue,
    Red,
}

#[derive(Debug, Clone)]
enum Cell {
    Mountain,
    Open,
    Fortress(Option<Team>, usize),
    King(Team, usize),
    Captured(Team, usize),
}

mod colors {
    use super::*;
    pub fn red() -> Color {
        Color::new(1.0, 0.1, 0.1, 1.0)
    }
    pub fn blue() -> Color {
        Color::new(0.1, 0.1, 1.0, 1.0)
    }
    pub fn red_light() -> Color {
        Color::new(0.8, 0.1, 0.1, 1.0)
    }
    pub fn blue_light() -> Color {
        Color::new(0.1, 0.1, 0.8, 1.0)
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
    pub fn blue_overlay() -> Color {
        Color::new(0.0, 0.0, 1.0, 0.5)
    }
}

impl Cell {
    fn color(&self) -> Color {
        use Cell::*;
        match *self {
            Mountain => Color::new(0.2, 0.2, 0.2, 1.0),
            Open => Color::new(1.0, 1.0, 1.0, 1.0),
            Fortress(_, _) => Color::new(0.5, 1.0, 0.3, 1.0),
            King(Team::Red, _) => colors::red(),
            King(Team::Blue, _) => colors::blue(),
            Captured(Team::Red, _) => colors::red_light(),
            Captured(Team::Blue, _) => colors::blue_light(),
        }
    }

    fn is_controlled_by(&self, team: Team) -> bool {
        use Cell::*;
        match *self {
            Mountain | Open => false,
            Fortress(Some(t), _) |
            King(t, _) |
            Captured(t, _) => team == t,
            _ => false,
        }
    }

    fn take_units(&mut self) -> usize {
        use Cell::*;
        match *self {
            Fortress(_, ref mut n) |
            King(_, ref mut n) |
            Captured(_, ref mut n) => {
                let num = *n;
                *n = 1;
                num - 1
            }
            _ => panic!("Cell {:?} has no units!", self),
        }
    }

    fn give_units(&mut self, num: usize) {
        use Cell::*;
        match *self {
            Fortress(_, ref mut n) |
            King(_, ref mut n) |
            Captured(_, ref mut n) => {
                *n += num;
            }
            _ => panic!("Cell {:?} has no units!", self),
        }
    }
}

#[derive(Debug)]
struct Board {
    cells: Vec<Vec<Cell>>,
}

impl Board {
    pub fn empty(n: usize) -> Self {
        let cells = (0..n).map(|_| vec![Cell::Open; n]).collect::<Vec<_>>();
        Board { cells }
    }

    fn randomize(&mut self) {
        let mut rng = rand::thread_rng();
        let mut items = [
            Weighted {
                weight: 100,
                item: Cell::Open,
            },
            Weighted {
                weight: 10,
                item: Cell::Mountain,
            },
            Weighted {
                weight: 1,
                item: Cell::Fortress(None, 0),
            },
        ];
        let wc = WeightedChoice::new(&mut items);

        for row in self.cells.iter_mut() {
            for cell in row.iter_mut() {
                *cell = wc.ind_sample(&mut rng);
            }
        }
        let n = self.cells.len();
        let (r_x, r_y) = (rng.gen_range(0, n), rng.gen_range(0, n));
        let (b_x, b_y) = (rng.gen_range(0, n), rng.gen_range(0, n));
        self.cells[r_x][r_y] = Cell::King(Team::Red, 0);
        self.cells[b_x][b_y] = Cell::King(Team::Blue, 0);
    }
}

#[derive(Debug, Clone, Copy)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    fn from_keycode(keycode: Keycode) -> Self {
        match keycode {
            Keycode::Up => Direction::Up,
            Keycode::Down => Direction::Down,
            Keycode::Left => Direction::Left,
            Keycode::Right => Direction::Right,
            _ => panic!("Not a valid direction: {:?}", keycode),
        }
    }

    fn to_xy(&self) -> (i32, i32) {
        use Direction::*;
        match *self {
            Up => (0, -1),
            Down => (0, 1),
            Left => (-1, 0),
            Right => (1, 0),
        }
    }
}

enum Move {
    Focus((i32, i32)),
    Movement(Direction),
}

struct PlayerState {
    /// Where is the player currently focusing?
    focus: Option<(i32, i32)>,
    /// Where are we currently moving?
    movement: Option<(i32, i32)>,
    /// The Move queue.
    moves: VecDeque<Move>,
}

impl PlayerState {
    fn new() -> Self {
        Self {
            focus: None,
            movement: None,
            moves: VecDeque::new(),
        }
    }
}

struct MainState {
    board: Board,
    time: Duration,
    tick_number: usize,
    last_tick: Duration,
    tick_interval: Duration,
    font: graphics::Font,

    // blue_state: PlayerState,
    red_state: PlayerState,
}

impl MainState {
    fn new(_ctx: &mut Context) -> GameResult<MainState> {
        let mut board = Board::empty(32);
        board.randomize();
        Ok(MainState {
            board,
            time: Duration::new(0, 0),
            tick_number: 0,
            last_tick: Duration::new(0, 0),
            tick_interval: Duration::new(0, 500_000_000),
            font: graphics::Font::default_font().unwrap(),

            // blue_state: PlayerState::new(),
            red_state: PlayerState::new(),
        })
    }

    fn tick(&mut self) {
        const ALL_UPDATE_INTERVAL: usize = 32;
        self.tick_number += 1;
        let update_all = self.tick_number % ALL_UPDATE_INTERVAL == 0;
        for row in self.board.cells.iter_mut() {
            for cell in row.iter_mut() {
                match *cell {
                    Cell::Fortress(Some(_), ref mut n) |
                    Cell::King(_, ref mut n) => {
                        *n += 1;
                    }
                    Cell::Captured(_, ref mut n) if update_all => {
                        *n += 1;
                    }
                    _ => {}
                }
            }
        }
        while let Some(m) = self.red_state.moves.pop_front() {
            match m {
                Move::Focus((x, y)) => self.red_state.movement = Some((x, y)),
                Move::Movement(dir) => {
                    if let Some((x, y)) = self.red_state.movement {
                        // TODO: clean up types here.
                        let (dx, dy) = dir.to_xy();
                        let (old_x, old_y) = (x as usize, y as usize);
                        let (new_x, new_y) = (x + dx, y + dy);
                        let units = self.board.cells[old_y][old_x].take_units();
                        if units == 0 {
                            self.red_state.focus = None;
                            self.red_state.movement = None;
                            self.red_state.moves.clear();
                            break;
                        }
                        let new_cell = &mut self.board.cells[new_y as usize][new_x as usize];

                        match new_cell {
                            &mut Cell::Mountain => {
                                self.red_state.moves.clear();
                                break;
                            }
                            &mut Cell::Open => {
                                *new_cell = Cell::Captured(Team::Red, units);
                            }
                            _ => unreachable!(),
                        }
                        self.red_state.movement = Some((new_x, new_y));
                    } else {
                        panic!("Got movement, but we were not moving!");
                    }
                    break;
                }
            }
        }
    }
}

impl event::EventHandler for MainState {
    fn update(&mut self, _ctx: &mut Context, _dt: Duration) -> GameResult<()> {
        self.time += _dt;
        while self.time - self.last_tick > self.tick_interval {
            self.last_tick += self.tick_interval;
            self.tick();
        }

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        graphics::clear(ctx);
        for (y, row) in self.board.cells.iter().enumerate() {
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
                graphics::set_color(ctx, cell.color()).unwrap();
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
        let mut current_pos = self.red_state.movement;
        graphics::set_color(ctx, colors::black()).unwrap();
        for m in self.red_state.moves.iter() {
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
            println!("{:?}", current_pos);
        }
        if let Some((x, y)) = self.red_state.focus {
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
            graphics::set_color(ctx, colors::red_overlay()).unwrap();
            graphics::rectangle(ctx, DrawMode::Fill, rect).unwrap();
        }
        graphics::present(ctx);
        Ok(())
    }

    fn mouse_button_down_event(&mut self, button: MouseButton, x: i32, y: i32) {
        if button != MouseButton::Left {
            return;
        }
        let ix = (x / (CELL_SIZE + 1.0) as i32) as usize;
        let iy = (y / (CELL_SIZE + 1.0) as i32) as usize;
        let clicked_cell = &self.board.cells[iy][ix];

        // TODO: handle player state instead of red state.
        if clicked_cell.is_controlled_by(Team::Red) {
            let coord = (ix as i32, iy as i32);
            self.red_state.focus = Some(coord);
            self.red_state.moves.push_back(Move::Focus(coord));
        } else {
            self.red_state.focus = None;
        }
    }

    fn key_down_event(&mut self, keycode: Keycode, _keymod: Mod, _repeat: bool) {
        match keycode {
            Keycode::Q => {
                self.red_state.moves.clear();
                self.red_state.movement = None;
            }
            Keycode::Up | Keycode::Down | Keycode::Left | Keycode::Right => {
                let dir = Direction::from_keycode(keycode);
                if let Some((ref x, ref y)) = self.red_state.focus {
                    self.red_state.moves.push_back(Move::Movement(dir));
                    let (dx, dy) = dir.to_xy();
                    x.checked_add(dx);
                    y.checked_add(dy);
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
