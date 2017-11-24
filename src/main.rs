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

#[derive(Debug, Clone, Copy)]
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
    pub fn black() -> Color {
        Color::new(0.0, 0.0, 0.0, 1.0)
    }
    pub fn black_overlay() -> Color {
        Color::new(0.0, 0.0, 0.0, 0.3)
    }
    pub fn red_overlay() -> Color {
        Color::new(1.0, 0.0, 0.0, 0.5)
    }
}

impl Cell {
    fn color(&self) -> Color {
        use Cell::*;
        match *self {
            Mountain => Color::new(0.2, 0.2, 0.2, 1.0),
            Open => Color::new(1.0, 1.0, 1.0, 1.0),
            Fortress(None, _) => Color::new(0.4, 0.4, 0.4, 1.0),
            Captured(Team::Red, _) |
            King(Team::Red, _) |
            Fortress(Some(Team::Red), _) => colors::red(),
            Captured(Team::Blue, _) |
            King(Team::Blue, _) |
            Fortress(Some(Team::Blue), _) => colors::blue(),
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

    fn get(&self, x: i32, y: i32) -> &Cell {
        &self.cells[y as usize][x as usize]
    }

    fn get_mut(&mut self, x: i32, y: i32) -> &mut Cell {
        &mut self.cells[y as usize][x as usize]
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

    /// Returns the direction you would get to if you are at the given position, and go in `self`
    /// direction. Clip at `0`, `w`, `h`.
    fn from(&self, (x, y): (i32, i32), w: i32, h: i32) -> Option<(i32, i32)> {
        use Direction::*;
        match *self {
            Up => if y == 0 { None } else { Some((x, y - 1)) },
            Down => if y >= h - 1 { None } else { Some((x, y + 1)) },
            Left => if x == 0 { None } else { Some((x - 1, y)) },
            Right => if x >= w - 1 { None } else { Some((x + 1, y)) },
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
        let update_tick = self.tick_number % 2 == 0;
        let update_all = self.tick_number % ALL_UPDATE_INTERVAL == 0;
        for row in self.board.cells.iter_mut() {
            for cell in row.iter_mut() {
                match *cell {
                    Cell::Fortress(Some(_), ref mut n) |
                    Cell::King(_, ref mut n) => {
                        if update_tick {
                            *n += 1;
                        }
                    }
                    Cell::Captured(_, ref mut n) if update_all => {
                        if update_tick {
                            *n += 1;
                        }
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
                        let (new_x, new_y) = (x + dx, y + dy);
                        let mut units = self.board.get_mut(x, y).take_units();
                        if units == 0 {
                            self.red_state.focus = None;
                            self.red_state.movement = None;
                            self.red_state.moves.clear();
                            break;
                        }

                        let mut did_capture = false;
                        let mut return_units_and_break = false;
                        match self.board.get_mut(new_x, new_y) {
                            &mut Cell::Mountain => {
                                self.red_state.focus = None;
                                self.red_state.movement = None;
                                self.red_state.moves.clear();
                                return_units_and_break = true;
                            }
                            c @ &mut Cell::Open => {
                                *c = Cell::Captured(Team::Red, units);
                            }
                            &mut Cell::Captured(Team::Red, ref mut n) |
                            &mut Cell::King(Team::Red, ref mut n) |
                            &mut Cell::Fortress(Some(Team::Red), ref mut n) => {
                                *n += units;
                            }
                            &mut Cell::Captured(ref mut team, ref mut n) |
                            &mut Cell::Fortress(Some(ref mut team), ref mut n) => {
                                if *n >= units {
                                    *n -= units;
                                } else {
                                    *team = Team::Red;
                                    *n = units - *n;
                                }
                            }
                            &mut Cell::King(_team, ref mut n) => {
                                if *n >= units {
                                    *n -= units;
                                } else {
                                    units -= *n - 1;
                                    did_capture = true;
                                }
                            }
                            &mut Cell::Fortress(ref mut team @ None, ref mut n) => {
                                if *n >= units {
                                    *n -= units;
                                } else {
                                    *team = Some(Team::Red);
                                    *n = units - *n;
                                }
                            }
                        }
                        if return_units_and_break {
                            self.board.get_mut(x, y).give_units(units);
                            break;
                        }
                        if did_capture {
                            *self.board.get_mut(new_x, new_y) =
                                Cell::Fortress(Some(Team::Red), units);
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

    fn dimens(&self) -> (i32, i32) {
        (self.board.cells.len() as i32, self.board.cells.len() as i32)
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
        let board_size = self.board.cells.len();
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
        }
        if let Some((x, y)) = self.red_state.focus {
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
            graphics::set_color(ctx, colors::red_overlay()).unwrap();
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
                    graphics::set_color(ctx, colors::black_overlay()).unwrap();
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
        let clicked_cell = &self.board.get(ix, iy);

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
                let (w, h) = self.dimens();
                if let Some((ref mut x, ref mut y)) = self.red_state.focus {
                    let (dx, dy) = dir.to_xy();
                    let nx = *x + dx;
                    let ny = *y + dy;
                    if nx >= 0 && nx < w && ny >= 0 && ny < h {
                        self.red_state.moves.push_back(Move::Movement(dir));
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
