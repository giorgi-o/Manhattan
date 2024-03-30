use std::{
    collections::{HashMap, HashSet},
    mem,
};

use macroquad::color::{BLUE, RED};
use pyo3::prelude::*;
use rand::Rng;

use crate::{
    logic::car::{NextCarPosition, NullAgent},
    python::bridge::{bridge::PythonAgentWrapper, py_grid::PyGridState},
};

use super::{
    car::{
        Car, CarDecision, CarPassenger, CarPosition, CarProps, NearestPassenger, PythonAgent,
        RandomDestination, RandomTurns,
    },
    passenger::{Passenger, PassengerId},
};

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

        if possible_decisions.is_empty() {
            println!("decisions list is empty");
            return self.possible_decisions();
        }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[pyclass]
pub enum LightState {
    Red,
    Green,
}

impl LightState {
    pub fn toggle(&mut self) {
        *self = match self {
            LightState::Red => LightState::Green,
            LightState::Green => LightState::Red,
        }
    }

    pub fn random(mut rng: impl Rng) -> Self {
        match rng.gen() {
            true => LightState::Green,
            false => LightState::Red,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[pyclass]
pub struct TrafficLight {
    #[pyo3(get)]
    pub toggle_every_ticks: usize,
    #[pyo3(get)]
    pub state: LightState,
    #[pyo3(get)]
    pub ticks_left: usize,
}

impl TrafficLight {
    pub fn tick(&mut self) {
        if self.ticks_left > 0 {
            self.ticks_left -= 1;
        } else {
            self.state.toggle();
            self.ticks_left = self.toggle_every_ticks;
        }
    }

    // see what the light will be like in X ticks
    pub fn time_travel(&self, ticks: usize) -> Self {
        let state_changes = ticks / self.toggle_every_ticks;
        let remainder = ticks % self.toggle_every_ticks;

        let mut new_state = self.state;
        if state_changes % 2 == 1 {
            new_state.toggle();
        }

        let mut ticks_left = self.ticks_left;
        if remainder <= ticks_left {
            ticks_left -= remainder;
        } else {
            new_state.toggle();
            ticks_left = self.toggle_every_ticks - remainder;
        }

        Self {
            toggle_every_ticks: self.toggle_every_ticks,
            state: new_state,
            ticks_left,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[pyclass]
pub struct GridOpts {
    #[pyo3(get)]
    pub initial_passenger_count: u32, // number of passengers on the grid at the start
    #[pyo3(get)]
    pub passenger_spawn_rate: f32, // chance of spawning a new passenger per tick
    #[pyo3(get)]
    pub agent_car_count: u32,
    #[pyo3(get)]
    pub npc_car_count: u32,
}

#[pymethods]
impl GridOpts {
    #[new]
    fn new(
        initial_passenger_count: u32,
        passenger_spawn_rate: f32,
        agent_car_count: u32,
        npc_car_count: u32,
    ) -> Self {
        Self {
            initial_passenger_count,
            passenger_spawn_rate,
            agent_car_count,
            npc_car_count,
        }
    }
}

#[derive(Debug, Default)]
struct TickEvents {
    // to calculate tick reward
    passengers_dropped_off: Vec<Passenger>,
}

pub struct Grid {
    pub opts: GridOpts,

    pub cars: Vec<Car>,
    taken_positions: HashSet<CarPosition>,

    pub waiting_passengers: HashMap<PassengerId, Passenger>,

    // None position = random spawn point
    pub cars_to_spawn: Vec<(CarProps, Option<CarPosition>)>,

    pub traffic_lights: HashMap<RoadSection, TrafficLight>,

    pub ticks_passed: usize,

    pub tick_state: Option<PyGridState>,
    tick_events: TickEvents,
}

impl Grid {
    pub const HORIZONTAL_ROADS: usize = 5;
    pub const VERTICAL_ROADS: usize = 7;
    pub const HORIZONTAL_SECTION_SLOTS: usize = 5;
    pub const VERTICAL_SECTION_SLOTS: usize = 5;

    pub const TRAFFIC_LIGHT_TOGGLE_TICKS: usize = 60; // 3s at 20TPS

    pub const CAR_SPEED: usize = 3;

    // pub const MAX_TOTAL_PASSENGERS: usize = Self::HORIZONTAL_ROADS * Self::VERTICAL_ROADS;
    // pub const MAX_WAITING_PASSENGERS: usize = Self::MAX_TOTAL_PASSENGERS / 2;
    pub const MAX_WAITING_PASSENGERS: usize = 20;

    pub fn new(opts: GridOpts, python_agent: PythonAgentWrapper) -> Self {
        // assign a traffic light to every road
        let traffic_lights = Self::generate_traffic_lights();
        let waiting_passengers = Self::generate_passengers(opts.initial_passenger_count);

        let mut this = Self {
            opts,

            // grid: HashMap::new(),
            cars: Vec::new(),
            taken_positions: HashSet::new(),

            waiting_passengers,

            cars_to_spawn: Vec::new(),

            traffic_lights,

            ticks_passed: 0,

            tick_state: None,
            tick_events: TickEvents::default(),
        };

        // tmp: spawn X random cars
        // for _ in 0..1 {
        //     // let agent = RandomTurns {};
        //     // let agent = RandomDestination::default();
        //     // let agent = NearestPassenger::default();
        //     let agent = PythonAgent::default();
        //     let car = CarProps::new(agent, 3);
        //     this.add_car(car);
        // }

        // spawn required npc cars
        for _ in 0..opts.npc_car_count {
            let npc_props = CarProps::new(RandomTurns {}, Self::CAR_SPEED, BLUE);
            this.add_car(npc_props, None);
        }

        // spawn required agent cars
        for _ in 0..opts.agent_car_count {
            // let agent = PythonAgent::new();
            // let car = CarProps::new(agent, 3);
            // this.add_car(car);

            let python_agent = PythonAgent::new(python_agent.clone());
            let agent_props = CarProps::new(python_agent, Self::CAR_SPEED, RED);
            this.add_car(agent_props, None);
        }

        this
    }

    fn generate_traffic_lights() -> HashMap<RoadSection, TrafficLight> {
        let mut traffic_lights = HashMap::new();

        for section in RoadSection::all() {
            let state = match section.direction.orientation() {
                Orientation::Horizontal => LightState::Green,
                Orientation::Vertical => LightState::Red,
            };
            let traffic_light = TrafficLight {
                toggle_every_ticks: Self::TRAFFIC_LIGHT_TOGGLE_TICKS,
                state,
                ticks_left: Self::TRAFFIC_LIGHT_TOGGLE_TICKS,
            };
            traffic_lights.insert(section, traffic_light);
        }

        traffic_lights
    }

    fn generate_passengers(count: u32) -> HashMap<PassengerId, Passenger> {
        let mut waiting_passengers = HashMap::with_capacity(Self::MAX_WAITING_PASSENGERS);
        let mut rng = rand::thread_rng();

        for _ in 0..count {
            let passenger = Passenger::random(&mut rng, 0);
            waiting_passengers.insert(passenger.id, passenger);
        }

        waiting_passengers
    }

    pub fn cars(&self) -> impl Iterator<Item = &Car> {
        self.cars.iter()
    }

    pub fn add_car(&mut self, props: CarProps, position: Option<CarPosition>) {
        self.cars_to_spawn.push((props, position));
    }

    pub fn has_car_at(&self, position: &CarPosition) -> bool {
        self.taken_positions.contains(position)
    }

    pub fn traffic_light_at(&self, section: &RoadSection) -> &TrafficLight {
        &self.traffic_lights[section]
    }

    pub fn tick(&mut self) -> f32 /* reward */ {
        let tick_state = PyGridState::build(self);
        self.tick_state = Some(tick_state);

        self.tick_traffic_lights();
        self.tick_cars();
        self.tick_passengers();

        self.tick_state = None;
        self.ticks_passed += 1;

        let post_tick_state = PyGridState::build(self);
        let reward = self.calculate_reward();
        self.send_transition_results(post_tick_state, reward);

        reward
    }

    fn tick_traffic_lights(&mut self) {
        // up next
        for traffic_light in &mut self.traffic_lights.values_mut() {
            traffic_light.ticks_left -= 1;

            if traffic_light.ticks_left == 0 {
                traffic_light.state.toggle();
                traffic_light.ticks_left = traffic_light.toggle_every_ticks;
            }
        }
    }

    fn tick_cars(&mut self) {
        // move all the cars in the grid
        // this is done in 2 passes: first we calculate which cars want to move
        // where, while checking two cars don't want to move to the same place.
        // then we actually move them in phase 2.

        // to double check we don't lose cars
        let cars_count = self.cars.len();

        // list of before-and-after positions
        let mut cars_to_move = HashMap::with_capacity(self.cars.len());

        // set of after positions, to see if another car is already moving there
        let mut next_positions = HashSet::with_capacity(self.cars.len());

        // hashmap of positions, to easily check for car presence at coords
        let old_positions = self
            .cars
            .iter()
            .map(|car| car.position)
            .collect::<HashSet<_>>();

        // temporarily move cars out of grid (to have a &mut cars and &self)
        let mut cars = std::mem::take(&mut self.cars);

        for car in &mut cars {
            // delete all "pick up" commands (they are per-tick)
            // if !car.props.agent.is_npc() {
            //     println!("passengers before prune: {:?}", car.passengers);
            // }
            car.passengers.retain(|p| p.is_dropping_off());
            // if !car.props.agent.is_npc() {
            //     println!("passengers after prune: {:?}", car.passengers);
            // }

            let old_position = car.position;

            // by default, the car stays still
            assert!(!next_positions.contains(&old_position));
            next_positions.insert(old_position);

            // if the car is at a red light, sit still
            if car.position.position_in_section
                == car
                    .position
                    .road_section
                    .direction
                    .max_position_in_section()
            {
                let traffic_light = &self.traffic_lights[&car.position.road_section];
                if traffic_light.state == LightState::Red {
                    car.ticks_since_last_movement = 0;
                    continue;
                }
            }

            // cars can only move every "speed" ticks
            if car.ticks_since_last_movement < car.props.speed {
                car.ticks_since_last_movement += 1;
                continue;
            }

            // tick agent
            // temporarily take agent out of car
            let null_agent = Box::new(NullAgent {});
            let mut agent = std::mem::replace(&mut car.props.agent, null_agent);

            let decision = agent.get_turn(self, car);

            car.props.agent = agent;

            // calculate next position, using decision if needex
            let next_position = old_position.next();
            let next_position = match next_position {
                NextCarPosition::OnlyStraight(next) => next,
                NextCarPosition::MustChoose(possible_decisions) => {
                    old_position.take_decision(decision)
                }
            };

            if next_position == old_position {
                panic!("car stayed still"); // tmp
                continue; // the car stays still, nothing to do
            }

            // if there is a car already there -> don't move there, cause that
            // car might not move (e.g. red light)
            // if there will be a car there next turn -> don't move either
            if old_positions.contains(&next_position) || next_positions.contains(&next_position) {
                continue;
            }

            // the car should move.
            next_positions.remove(&old_position);
            // cars_to_move.push((old_position, next_position));
            cars_to_move.insert(old_position, next_position);
            next_positions.insert(next_position);

            car.ticks_since_last_movement = 0;
        }

        // move the cars
        for car in &mut cars {
            let Some(next_position) = cars_to_move.remove(&car.position) else {
                continue; // car stays still
            };
            assert_ne!(car.position, next_position);

            car.position = next_position;
        }

        self.taken_positions = next_positions;

        let new_cars_count = self.cars_to_spawn.len();

        // spawn cars waiting to be spawned
        if !self.cars_to_spawn.is_empty() {
            let cars_to_spawn = std::mem::take(&mut self.cars_to_spawn);
            let mut rng = rand::thread_rng();

            for (props, position) in cars_to_spawn {
                let position = position.unwrap_or_else(|| self.random_empty_car_position(&mut rng));

                let car = Car::new(props, position);
                // self.grid.insert(position, car);
                cars.push(car);
                self.taken_positions.insert(position);
            }
        }

        // put cars back
        self.cars = cars;

        // check we didn't lose any cars in the process
        assert_eq!(cars_count + new_cars_count, self.cars.len());
        assert_eq!(self.taken_positions.len(), self.cars.len());
    }

    fn tick_passengers(&mut self) {
        // // spawn passengers if we need to
        // // let waiting_passengers = self.waiting_passengers.len();
        // // let riding_passengers: usize = self
        // //     .cars
        // //     .iter()
        // //     .map(|c| c.passengers.iter().filter(|p| p.is_dropping_off()).count())
        // //     .sum();
        // // let total_passengers = waiting_passengers + riding_passengers;

        // let passengers_to_spawn = Self::MAX_WAITING_PASSENGERS
        //     - waiting_passengers.min(Self::MAX_TOTAL_PASSENGERS - total_passengers);

        // if passengers_to_spawn > 0 {
        //     // println!("spawning {passengers_to_spawn} passengers");
        //     let mut rng = rand::thread_rng();

        //     for _ in 0..passengers_to_spawn {
        //         let passenger = Passenger::random(&mut rng);
        //         self.waiting_passengers.insert(passenger.id, passenger);
        //     }
        // }

        // spawn passengers
        let mut rng = rand::thread_rng();
        while rng.gen::<f32>() < self.opts.passenger_spawn_rate {
            let passenger = Passenger::random(&mut rng, self.ticks_passed);
            self.waiting_passengers.insert(passenger.id, passenger);
        }

        // make cars pick up passengers
        for car in &mut self.cars {
            // for every passenger in this car
            for passenger_in_vec in &mut car.passengers {
                // if this passenger is a "reserved seat" i.e. a passenger towards
                // whom the car is heading towards
                let CarPassenger::PickingUp(passenger_id) = passenger_in_vec else {
                    continue; // if not, go next
                };

                let passenger = self
                    .waiting_passengers
                    .get(passenger_id)
                    .expect("Car is picking up passenger that doesn't exist");

                // if the car is right now next to that passenger
                if car.position == passenger.start {
                    // pick them up:
                    // remove them from the sidewalk
                    let passenger = self.waiting_passengers.remove(passenger_id).unwrap();
                    // put them into the car
                    *passenger_in_vec = CarPassenger::DroppingOff(passenger);
                }
            }
        }

        // make cars drop off passengers
        for car in &mut self.cars {
            car.passengers.retain(|passenger| {
                let CarPassenger::DroppingOff(passenger) = passenger else {
                    return true;
                };

                // if they are different, we don't drop them off
                // => we retain the passenger
                passenger.destination != car.position
            });
        }
    }

    fn calculate_reward(&mut self) -> f32 {
        let events = mem::take(&mut self.tick_events);
        let mut reward = 0.0;

        // -1 for every waiting passenger
        reward -= self.waiting_passengers.len() as f32;

        // +100 for every dropped off passenger
        reward += events.passengers_dropped_off.len() as f32 * 100.0;

        reward
    }

    fn send_transition_results(&self, new_state: PyGridState, reward: f32) {
        for car in &self.cars {
            let Some(py_agent) = car.props.agent.as_py_agent() else {
                continue;
            };

            let new_state = new_state.with_pov(car);
            py_agent.end_of_tick(new_state, reward);
        }
    }

    fn random_empty_car_position(&self, mut rng: impl Rng) -> CarPosition {
        for _ in 0..1000 {
            let position = CarPosition::random(&mut rng);
            if !self.has_car_at(&position) {
                return position;
            }
        }

        panic!("Grid is full!")
    }

    pub fn waiting_passengers(&self) -> impl Iterator<Item = &Passenger> {
        self.waiting_passengers.values()
    }

    pub fn unassigned_passengers(&self) -> Vec<&Passenger> {
        self.waiting_passengers
            .values()
            .filter(|p| !p.car_on_its_way)
            .collect()
    }

    fn spawn_passenger(&mut self, passenger: Passenger) {
        self.waiting_passengers.insert(passenger.id, passenger);
    }

    pub fn assign_car_to_passenger(&mut self, car: &mut Car, passenger: PassengerId) {
        let passenger = self
            .waiting_passengers
            .get(&passenger)
            .expect("Car tried to assign to non-existent passenger");
        car.passengers.push(CarPassenger::PickingUp(passenger.id));
    }

    pub fn get_idle_passenger(&self, passenger: PassengerId) -> Option<&Passenger> {
        self.waiting_passengers.get(&passenger)
    }

    pub fn py_state(&self, pov_car: &Car) -> PyGridState {
        self.tick_state
            .as_ref()
            .expect("Grid::py_state() called outside of tick")
            .with_pov(pov_car)
    }
}

// impl Default for Grid {
//     fn default() -> Self {
//         // Self::new()
//         todo!()
//     }
// }
