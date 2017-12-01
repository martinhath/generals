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

pub type Team = usize;

#[derive(Debug, Clone, Copy)]
pub enum Cell {
    Mountain,
    // TODO: make `Open(usize)`, and have it always be zero?
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

/// A movement, from a position in a direction.
pub type Move = (Position, Direction);

pub struct PlayerState {
    /// The Move queue.
    pub moves: VecDeque<Move>,
    pub dead: bool,
    pub team: Team,
}


#[derive(Clone, Copy)]
pub struct Position(pub i32, pub i32);

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

    pub fn randomize(&mut self, num_players: usize) {
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
        for team in 0..num_players {
            let (x, y) = (rng.gen_range(0, n), rng.gen_range(0, n));
            self.cells[x][y] = Cell::King(team, 1);
        }
    }

    pub fn cells(&self) -> &Vec<Vec<Cell>> {
        &self.cells
    }

    pub fn get(&self, x: i32, y: i32) -> &Cell {
        &self.cells[y as usize][x as usize]
    }

    pub fn try_get(&self, x: i32, y: i32) -> Option<&Cell> {
        self.cells.get(y as usize).and_then(|r| r.get(x as usize))
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
    pub fn new(team: Team) -> Self {
        Self {
            moves: VecDeque::new(),
            dead: false,
            team,
        }
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
        for player_state in self.player_states.iter_mut() {
            let team = player_state.team;
            if let Some((from, dir)) = player_state.moves.pop_front() {
                let Position(x, y) = from;
                let (dx, dy) = dir.to_xy();
                let (new_x, new_y) = (x + dx, y + dy);
                let mut units = self.board.get_mut(x, y).take_units();
                if units == 0 {
                    player_state.moves.clear();
                    continue;
                }

                let mut did_capture = false;
                let mut return_units_and_break = false;
                // Possible scenarios:
                //  We move units from our cell to another of our cells:
                //      - Simply move over the units.
                //  We move units from our cell to a neutral cell:
                //      - If the neutral cell is Open, replace it with `Captured(n - 1)`.
                //      - If the neutral cell is Fortress, eat from it.

                {
                    let target_cell = self.board.get_mut(new_x, new_y);
                    if target_cell.is_controlled_by(team) {
                        target_cell.give_units(units);
                    } else {
                        match target_cell {
                            &mut Cell::Mountain => {
                                player_state.moves.clear();
                                return_units_and_break = true;
                            }
                            cell @ &mut Cell::Open => {
                                *cell = Cell::Captured(team, units);
                            }
                            &mut Cell::Captured(ref mut team, ref mut n) |
                            &mut Cell::Fortress(Some(ref mut team), ref mut n) => {
                                if *n >= units {
                                    *n -= units;
                                } else {
                                    *team = player_state.team;
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
                                    *team = Some(player_state.team);
                                    *n = units - *n;
                                }
                            }
                        }
                    }
                }
                if return_units_and_break {
                    self.board.get_mut(x, y).give_units(units);
                    break;
                }
                if did_capture {
                    *self.board.get_mut(new_x, new_y) =
                        Cell::Fortress(Some(player_state.team), units);
                }
            }
        }
    }
}

impl ::std::ops::Add<Direction> for Position {
    type Output = Position;
    fn add(self, dir: Direction) -> Self {
        let (x, y) = dir.to_xy();
        Position(self.0 + x, self.1 + y)
    }
}
