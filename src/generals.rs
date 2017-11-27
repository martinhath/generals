use std::collections::VecDeque;
use rand::{self, Rng};
use rand::distributions::{Weighted, WeightedChoice, IndependentSample};

pub struct GameState {
    pub board: Board,
    pub tick_number: usize,
    pub num_players: usize,
    pub player_states: Vec<PlayerState>,
    pub dimens: (i32, i32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Team {
    Blue,
    Red,
}

#[derive(Debug, Clone, Copy)]
pub enum Cell {
    Mountain,
    Open,
    Fortress(Option<Team>, usize),
    King(Team, usize),
    Captured(Team, usize),
}

#[derive(Debug, Clone, Copy)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

pub type Move = (Position, Direction);

pub struct PlayerState {
    /// The Move queue.
    pub moves: VecDeque<Move>,
    pub dead: bool,
}

pub type Position = (i32, i32);

#[derive(Debug)]
pub struct Board {
    cells: Vec<Vec<Cell>>,
}

impl Cell {
    pub fn is_controlled_by(&self, team: Team) -> bool {
        use Cell::*;
        match *self {
            Mountain | Open => false,
            Fortress(Some(t), _) |
            King(t, _) |
            Captured(t, _) => team == t,
            _ => false,
        }
    }

    pub fn take_units(&mut self) -> usize {
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

    pub fn give_units(&mut self, num: usize) {
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


impl Board {
    pub fn empty(n: usize) -> Self {
        let cells = (0..n).map(|_| vec![Cell::Open; n]).collect::<Vec<_>>();
        Board { cells }
    }

    pub fn randomize(&mut self) {
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
                weight: 3,
                item: Cell::Fortress(None, 0),
            },
        ];
        let wc = WeightedChoice::new(&mut items);

        for row in self.cells.iter_mut() {
            for cell in row.iter_mut() {
                *cell = wc.ind_sample(&mut rng);
                match *cell {
                    Cell::Fortress(_, ref mut n) => {
                        *n = rng.gen_range(40, 50);
                    }
                    _ => {}
                }
            }
        }
        let n = self.cells.len();
        let (r_x, r_y) = (rng.gen_range(0, n), rng.gen_range(0, n));
        let (b_x, b_y) = (rng.gen_range(0, n), rng.gen_range(0, n));
        self.cells[r_x][r_y] = Cell::King(Team::Red, 0);
        self.cells[b_x][b_y] = Cell::King(Team::Blue, 0);
    }

    pub fn cells(&self) -> &Vec<Vec<Cell>> {
        &self.cells
    }

    pub fn get(&self, x: i32, y: i32) -> &Cell {
        &self.cells[y as usize][x as usize]
    }

    pub fn get_mut(&mut self, x: i32, y: i32) -> &mut Cell {
        &mut self.cells[y as usize][x as usize]
    }
}

impl Direction {
    pub fn to_xy(&self) -> (i32, i32) {
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
    pub fn from(&self, (x, y): (i32, i32), w: i32, h: i32) -> Option<(i32, i32)> {
        use Direction::*;
        match *self {
            Up => if y == 0 { None } else { Some((x, y - 1)) },
            Down => if y >= h - 1 { None } else { Some((x, y + 1)) },
            Left => if x == 0 { None } else { Some((x - 1, y)) },
            Right => if x >= w - 1 { None } else { Some((x + 1, y)) },
        }
    }
}

impl PlayerState {
    pub fn new() -> Self {
        Self {
            moves: VecDeque::new(),
            dead: false,
        }
    }

    pub fn clear_movement_queue(&mut self) {
        self.moves.clear();
    }
}

impl GameState {
    pub fn player_mut(&mut self, player: usize) -> &mut PlayerState {
        &mut self.player_states[player]
    }

    pub fn tick(&mut self) {
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
        'asd: for player_state in self.player_states.iter_mut() {
            if let Some((from, dir)) = player_state.moves.pop_front() {
                let (x, y) = from;

                let (dx, dy) = dir.to_xy();
                let (new_x, new_y) = (x + dx, y + dy);
                let mut units = self.board.get_mut(x, y).take_units();
                if units == 0 {
                    player_state.moves.clear();
                    continue 'asd;
                }

                let mut did_capture = false;
                let mut return_units_and_break = false;
                match self.board.get_mut(new_x, new_y) {
                    &mut Cell::Mountain => {
                        player_state.moves.clear();
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
                    *self.board.get_mut(new_x, new_y) = Cell::Fortress(Some(Team::Red), units);
                }
            }
        }
    }
}
