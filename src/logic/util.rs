use gxhash::GxBuildHasher;
use indexmap::{IndexMap, IndexSet};
use pyo3::prelude::*;
use rand::Rng;

use super::{car::CarDecision, grid::Grid};

pub type HashMap<K, V> = IndexMap<K, V, GxBuildHasher>;
pub type HashSet<K> = IndexSet<K, GxBuildHasher>;

pub fn hashmap_with_capacity<K, V>(capacity: usize) -> HashMap<K, V> {
    HashMap::with_capacity_and_hasher(capacity, GxBuildHasher::default())
}

pub fn hashset_with_capacity<K>(capacity: usize) -> HashSet<K> {
    HashSet::with_capacity_and_hasher(capacity, GxBuildHasher::default())
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[pyclass]
pub enum Orientation {
    Horizontal,
    Vertical,
}

impl Orientation {
    pub fn other(self) -> Self {
        match self {
            Orientation::Horizontal => Orientation::Vertical,
            Orientation::Vertical => Orientation::Horizontal,
        }
    }

    pub fn direction(self, towards_positive: bool) -> Direction {
        match (self, towards_positive) {
            (Orientation::Horizontal, true) => Direction::Right,
            (Orientation::Horizontal, false) => Direction::Left,
            (Orientation::Vertical, true) => Direction::Down,
            (Orientation::Vertical, false) => Direction::Up,
        }
    }

    pub fn max_road_index(self) -> usize {
        match self {
            Self::Horizontal => Grid::HORIZONTAL_ROADS - 1,
            Self::Vertical => Grid::VERTICAL_ROADS - 1,
        }
    }

    pub fn max_section_index(self) -> usize {
        self.other().max_road_index() - 1
    }

    pub fn max_position_in_section(self) -> usize {
        match self {
            Self::Horizontal => Grid::HORIZONTAL_SECTION_SLOTS - 1,
            Self::Vertical => Grid::VERTICAL_SECTION_SLOTS - 1,
        }
    }
}

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
#[pyclass]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    pub fn orientation(self) -> Orientation {
        match self {
            Direction::Up | Direction::Down => Orientation::Vertical,
            Direction::Left | Direction::Right => Orientation::Horizontal,
        }
    }

    pub fn is_horizontal(self) -> bool {
        self.orientation() == Orientation::Horizontal
    }

    pub fn towards_positive(self) -> bool {
        // 0, 0 is top left
        self == Direction::Down || self == Direction::Right
    }

    pub fn offset(self) -> isize {
        match self.towards_positive() {
            true => 1,
            false => -1,
        }
    }

    pub fn max_road_index(self) -> usize {
        match self.is_horizontal() {
            true => Grid::HORIZONTAL_ROADS - 1,
            false => Grid::VERTICAL_ROADS - 1,
        }
    }

    pub fn max_section_index(self) -> usize {
        self.clockwise().max_road_index() - 1
    }

    pub fn max_position_in_section(self) -> usize {
        match self.is_horizontal() {
            true => Grid::HORIZONTAL_SECTION_SLOTS - 1,
            false => Grid::VERTICAL_SECTION_SLOTS - 1,
        }
    }

    pub fn clockwise(self) -> Self {
        match self {
            Self::Up => Self::Right,
            Self::Right => Self::Down,
            Self::Down => Self::Left,
            Self::Left => Self::Up,
        }
    }

    pub fn counterclockwise(self) -> Self {
        match self {
            Self::Up => Self::Left,
            Self::Right => Self::Up,
            Self::Down => Self::Right,
            Self::Left => Self::Down,
        }
    }

    pub fn inverted(self) -> Self {
        match self {
            Self::Up => Self::Down,
            Self::Down => Self::Up,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    pub fn random(mut rng: impl Rng) -> Self {
        match rng.gen_range(0..4) {
            0 => Self::Up,
            1 => Self::Down,
            2 => Self::Left,
            3 => Self::Right,
            _ => unreachable!(),
        }
    }
}

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
#[pyclass]
pub struct RoadSection {
    // isize (not usize) because it makes rendering traffic lights easier
    #[pyo3(get)]
    pub road_index: isize,
    #[pyo3(get)]
    pub section_index: isize,
    #[pyo3(get)]
    pub direction: Direction,
    // both indexes start from 0
}

impl RoadSection {
    // road() and section() are for when you know it's positive (unsigned)

    pub fn road(self) -> usize {
        if self.road_index < 0 || self.road_index as usize > self.direction.max_road_index() {
            panic!(
                "Invalid road index {} (max {})",
                self.road_index,
                self.direction.max_road_index()
            )
        }

        self.road_index as usize
    }

    pub fn section(self) -> usize {
        if self.section_index < 0
            || self.section_index as usize > self.direction.max_section_index()
        {
            panic!(
                "Invalid section index {} (max {})",
                self.section_index,
                self.direction.max_section_index()
            )
        }

        self.section_index as usize
    }

    pub fn get(direction: Direction, road_index: usize, section_index: usize) -> Self {
        let this = Self::get_raw(direction, road_index as isize, section_index as isize);
        this.valid().unwrap();
        this
    }

    pub fn get_raw(direction: Direction, road_index: isize, section_index: isize) -> Self {
        Self {
            direction,
            road_index,
            section_index,
        }
    }

    pub fn get_invalid() -> Self {
        Self::get_raw(Direction::Up, -1, -1)
    }

    pub fn all() -> Vec<Self> {
        let mut all = vec![];

        // horizontal ones
        for road_index in 0..Grid::HORIZONTAL_ROADS {
            for section_index in 0..Grid::VERTICAL_ROADS - 1 {
                for direction in [Direction::Left, Direction::Right] {
                    let this = Self::get(direction, road_index, section_index);
                    all.push(this);
                }
            }
        }

        // and now vertical
        for road_index in 0..Grid::VERTICAL_ROADS {
            for section_index in 0..Grid::HORIZONTAL_ROADS - 1 {
                for direction in [Direction::Up, Direction::Down] {
                    let this = Self::get(direction, road_index, section_index);
                    all.push(this);
                }
            }
        }

        assert!(all.iter().all(|section| section.valid().is_ok()));

        all
    }

    pub fn random(mut rng: impl Rng) -> Self {
        let direction = Direction::random(&mut rng);

        let road_index = rng.gen_range(0..=direction.max_road_index());
        let section_index = rng.gen_range(0..=direction.max_section_index());
        Self::get(direction, road_index, section_index)
    }

    pub fn random_in_area(mut rng: impl Rng, area: (f32, f32, f32, f32)) -> Self {
        let direction = Direction::random(&mut rng);
        let (x1, y1, x2, y2) = area;

        for _ in 0..1000 {
            let road_index;
            let section_index;

            if direction.is_horizontal() {
                road_index = rng.gen_range(y1 as usize..y2 as usize);
                section_index = rng.gen_range(x1 as usize..x2 as usize);
            } else {
                road_index = rng.gen_range(x1 as usize..x2 as usize);
                section_index = rng.gen_range(y1 as usize..y2 as usize);
            }

            let this = Self::get(direction, road_index, section_index);
            let (x, y) = this.checkerboard_coords();

            if x >= x1 && x <= x2 && y >= y1 && y <= y2 {
                return this;
            }

        }

        panic!("Failed to find random section in area {:?}", area);
    }

    pub fn valid(self) -> Result<(), String> {
        if self.road_index < 0 || self.road_index as usize > self.direction.max_road_index() {
            return Err(format!(
                "Road {} going {:?} doesn't exist! (max {})",
                self.road_index,
                self.direction,
                self.direction.max_road_index()
            ));
        }

        if self.section_index < 0
            || self.section_index as usize > self.direction.max_section_index()
        {
            return Err(format!(
                "Section {} going {:?} doesn't exist! (max {})",
                self.section_index,
                self.direction,
                self.direction.max_section_index()
            ));
        }

        Ok(())
    }

    pub fn go_straight(self) -> Option<Self> {
        let new_section_index = self.section_index + self.direction.offset();
        if new_section_index < 0 {
            return None;
        }

        let next = Self {
            direction: self.direction,
            road_index: self.road_index,
            section_index: new_section_index,
        };

        match next.valid() {
            Ok(_) => Some(next),
            Err(_) => None,
        }
    }

    fn turn(self, right: bool) -> Option<Self> {
        let new_direction = match right {
            true => self.direction.clockwise(),
            false => self.direction.counterclockwise(),
        };

        let was_towards_positive = self.direction.towards_positive();
        let is_towards_positive = new_direction.towards_positive();

        // after turning, the old road index is the new section index and vice-versa
        // both + or - an offset

        let new_road_index_offset = match was_towards_positive {
            true => 1,
            false => 0,
        };
        let new_road_index = self.section_index + new_road_index_offset;

        if new_road_index as usize > new_direction.max_road_index() {
            return None;
        }

        let new_section_index_offset = match is_towards_positive {
            true => 0,
            false => -1,
        };
        let new_section_index = self.road_index + new_section_index_offset;

        if new_section_index < 0 || new_section_index as usize > new_direction.max_section_index() {
            return None;
        }

        let next = Self {
            direction: new_direction,
            road_index: new_road_index,
            section_index: new_section_index,
        };
        assert!(next.valid().is_ok());
        Some(next)
    }

    pub fn take_decision(self, decision: CarDecision) -> Option<Self> {
        match decision {
            CarDecision::GoStraight => self.go_straight(),
            CarDecision::TurnRight => self.turn(true),
            CarDecision::TurnLeft => self.turn(false),
            CarDecision::ChargeBattery => todo!("Is this ever called?"),
        }
    }

    pub fn possible_decisions(self) -> Vec<CarDecision> {
        let mut possible_decisions = Vec::with_capacity(3);

        for decision in [
            CarDecision::GoStraight,
            CarDecision::TurnLeft,
            CarDecision::TurnRight,
        ] {
            if let Some(_next) = self.take_decision(decision) {
                possible_decisions.push(decision);
            }
        }

        assert!(!possible_decisions.is_empty());
        possible_decisions
    }

    pub fn decision_to_go_to(self, destination: RoadSection) -> Option<CarDecision> {
        // I want to go to that other section right there,
        // what decision do I take to get there?
        // not pathfinding btw, only works for sections that can be reached in
        // one decision

        self.possible_decisions()
            .into_iter()
            .find(|d| self.take_decision(*d).is_some_and(|s| s == destination))
    }

    pub fn checkerboard_coords(self) -> (f32, f32) {
        // if the grid was a checkerboard. no horizontal/vertical coords.
        // what would the current x and y be
        // (useful for calculating manhattan distance)
        // if the car is between two roads, the value will be x.5

        let section_index = self.section_index as f32 + 0.5;
        let road_index = self.road_index as f32;

        match self.direction.orientation() {
            Orientation::Horizontal => (section_index, road_index),
            Orientation::Vertical => (road_index, section_index),
        }
    }

    pub fn manhattan_distance(self, other: Self) -> usize {
        let self_coords = self.checkerboard_coords();
        let other_coords = other.checkerboard_coords();

        let dx = (self_coords.0 - other_coords.0).abs() as usize;
        let dy = (self_coords.1 - other_coords.1).abs() as usize;

        dx + dy
    }
}
