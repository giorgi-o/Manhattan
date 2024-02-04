use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use rand::Rng;

// use crate::logic::car::CarState;

use crate::{logic::car::NextCarPosition, render::render_main::Game};

use super::car::{Car, CarAgent, CarDecision, CarPosition, CarProps, RandomCar};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
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
}

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
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
pub struct RoadSection {
    pub road_index: usize,
    pub section_index: usize,
    pub direction: Direction,
    // both indexes start from 0
}

impl RoadSection {
    pub fn get(direction: Direction, road_index: usize, section_index: usize) -> Self {
        let this = Self {
            direction,
            road_index,
            section_index,
        };

        this.valid().unwrap();

        this
    }

    pub fn all() -> Vec<Self> {
        let mut all = vec![];

        // horizontal ones
        for road_index in 0..Grid::HORIZONTAL_ROADS {
            for section_index in 0..Grid::VERTICAL_ROADS - 1 {
                for direction in [Direction::Left, Direction::Right] {
                    let this = Self {
                        road_index,
                        section_index,
                        direction,
                    };
                    all.push(this);
                }
            }
        }

        // and now vertical
        for road_index in 0..Grid::VERTICAL_ROADS {
            for section_index in 0..Grid::HORIZONTAL_ROADS - 1 {
                for direction in [Direction::Up, Direction::Down] {
                    let this = Self {
                        road_index,
                        section_index,
                        direction,
                    };
                    all.push(this);
                }
            }
        }

        assert!(all.iter().all(|section| section.valid().is_ok()));

        all
    }

    pub fn random(mut rng: impl Rng) -> Self {
        let direction = Direction::random(&mut rng);

        Self {
            direction,
            road_index: rng.gen_range(0..=direction.max_road_index()),
            section_index: rng.gen_range(0..=direction.max_section_index()),
        }
    }

    pub fn valid(&self) -> Result<(), String> {
        if self.road_index > self.direction.max_road_index() {
            return Err(format!(
                "Road {} going {:?} doesn't exist! (max {})",
                self.road_index,
                self.direction,
                self.direction.max_road_index()
            ));
        }

        if self.section_index > self.direction.max_section_index() {
            return Err(format!(
                "Section {} going {:?} doesn't exist! (max {})",
                self.section_index,
                self.direction,
                self.direction.max_section_index()
            ));
        }

        Ok(())
    }

    pub fn go_straight(&self) -> Option<Self> {
        let new_section_index = self.section_index as isize + self.direction.offset();
        if new_section_index < 0 {
            return None;
        }

        let next = Self {
            direction: self.direction,
            road_index: self.road_index,
            section_index: new_section_index as usize,
        };

        match next.valid() {
            Ok(_) => Some(next),
            Err(_) => None,
        }
    }

    fn turn(&self, right: bool) -> Option<Self> {
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

        if new_road_index > new_direction.max_road_index() {
            return None;
        }

        let new_section_index_offset = match is_towards_positive {
            true => 0,
            false => -1,
        };
        let new_section_index = self.road_index as isize + new_section_index_offset;

        if new_section_index < 0 || new_section_index as usize > new_direction.max_section_index() {
            return None;
        }

        let next = Self {
            direction: new_direction,
            road_index: new_road_index,
            section_index: new_section_index as usize,
        };
        assert!(next.valid().is_ok());
        Some(next)

        /*let new_direction = match right {
            true => self.direction.clockwise(),
            false => self.direction.counterclockwise(),
        };

        let section_to_road_offset = match new_direction.towards_positive() {
            true => 1,
            false => 0,
        };
        let new_road_index = self.section_index + section_to_road_offset;
        if new_road_index > new_direction.max_road_index() {
            return None;
        }

        let road_to_section_offset = match new_direction.towards_positive() {
            true => 0,
            false => -1,
        };
        let new_section_index = self.road_index as isize + road_to_section_offset;
        if new_section_index < 0 {
            return None;
        }

        let next = Self {
            direction: new_direction,
            road_index: new_road_index,
            section_index: new_section_index as usize,
        };
        Some(next)
        */
    }

    pub fn take_decision(&self, decision: CarDecision) -> Option<Self> {
        match decision {
            CarDecision::GoStraight => self.go_straight(),
            CarDecision::TurnRight => self.turn(true),
            CarDecision::TurnLeft => self.turn(false),
        }
    }

    pub fn possible_decisions(&self) -> Vec<CarDecision> {
        let mut possible_decisions = Vec::with_capacity(3);

        for decision in [
            CarDecision::GoStraight,
            CarDecision::TurnLeft,
            CarDecision::TurnRight,
        ] {
            if let Some(next) = self.take_decision(decision) {
                possible_decisions.push(decision);
            }
        }

        if possible_decisions.is_empty() {
            println!("decisions list is empty");
            return self.possible_decisions();
        }

        possible_decisions
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

pub struct TrafficLight {
    pub toggle_every_ticks: usize,
    pub state: LightState,
    pub ticks_left: usize,
}

pub struct Grid {
    // grid: HashMap<CarPosition, Car>,
    cars: Vec<Car>,
    taken_positions: HashSet<CarPosition>,

    last_tick: Instant,

    // None position = random spawn point
    cars_to_spawn: Vec<(CarProps, Option<CarPosition>)>,

    traffic_lights: HashMap<RoadSection, TrafficLight>,
}

// #[derive(Clone, Copy, PartialEq, Eq, Hash)]
// pub enum RoadOrientation {
//     Horizontal,
//     Vertical,
// }

impl Grid {
    pub const HORIZONTAL_ROADS: usize = 5;
    pub const VERTICAL_ROADS: usize = 7;
    // pub const HORIZONTAL_ROADS: usize = 2;
    // pub const VERTICAL_ROADS: usize = 3;
    pub const HORIZONTAL_SECTION_SLOTS: usize = 3;
    pub const VERTICAL_SECTION_SLOTS: usize = 3;

    pub const TRAFFIC_LIGHT_TOGGLE_TICKS: usize = 5 * Game::TICKS_PER_SEC;

    // const MAX_TICK_LENGTH: Duration = Duration::from_millis(100);

    pub fn new() -> Self {
        // let inner = Grid {
        //     grid: HashMap::new(),
        //     cars: Vec::new(),
        //     last_tick: Instant::now(),
        // };
        // let this = RefCell::new(inner);
        // let this = Rc::new(this);
        // Self(this)

        // assign a traffic light to every road
        let traffic_lights = Self::generate_traffic_lights();

        let mut this = Self {
            // grid: HashMap::new(),
            cars: Vec::new(),
            taken_positions: HashSet::new(),

            last_tick: Instant::now(),
            cars_to_spawn: Vec::new(),

            traffic_lights,
        };

        // tmp: spawn 3 random cars
        for _ in 0..70 {
            let agent = RandomCar {};
            let car = CarProps::new(agent, 10);
            this.add_car(car);
        }

        this
    }

    fn generate_traffic_lights() -> HashMap<RoadSection, TrafficLight> {
        let mut traffic_lights = HashMap::new();
        // let mut rng = rand::thread_rng();

        for section in RoadSection::all() {
            let state = match section.direction.orientation() {
                Orientation::Horizontal => LightState::Green,
                Orientation::Vertical => LightState::Red,
            };
            let traffic_light = TrafficLight {
                toggle_every_ticks: Self::TRAFFIC_LIGHT_TOGGLE_TICKS,
                state,
                // ticks_left: rng.gen_range(0..Self::TRAFFIC_LIGHT_TOGGLE_TICKS),
                ticks_left: Self::TRAFFIC_LIGHT_TOGGLE_TICKS,
            };
            traffic_lights.insert(section, traffic_light);
        }

        traffic_lights
    }

    pub fn cars(&self) -> impl Iterator<Item = &Car> {
        // self.grid.values()
        self.cars.iter()
    }

    // pub fn vertical_roads() -> usize {
    //     Self::VERTICAL_ROADS
    // }

    // pub fn horizontal_roads() -> usize {
    //     Self::HORIZONTAL_ROADS
    // }

    pub fn add_car(&mut self, car: CarProps) {
        // let car = Car::new(car_agent);
        // let car = RefCell::new(car);
        // self.borrow().cars.push(car);

        self.cars_to_spawn.push((car, None));
    }

    pub fn has_car_at(&self, position: &CarPosition) -> bool {
        // self.grid.contains_key(position)
        self.taken_positions.contains(position)
    }

    pub fn traffic_light_at(&self, section: &RoadSection) -> &TrafficLight {
        &self.traffic_lights[section]
    }

    pub fn tick(&mut self) {
        self.tick_traffic_lights();
        self.tick_cars();
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

        // to make sure we don't lose cars
        let cars_count = self.cars.len();

        // list of before-and-after positions
        // let mut cars_to_move = Vec::with_capacity(self.cars.len());
        let mut cars_to_move = HashMap::with_capacity(self.cars.len());

        // set of after positions, to see if another car is already moving there
        let mut next_positions = HashSet::with_capacity(self.cars.len());

        // take grid out of self.grid so that we can use self.other stuff
        // this also resets self.grid to an empty hashmap
        // let mut old_grid = std::mem::take(&mut self.grid);
        // let mut old_cars = std::mem::take(&mut self.cars);

        // hashmap of positions, to easily check for car presence at coords
        let old_positions = self
            .cars
            .iter()
            .map(|car| car.position)
            .collect::<HashSet<_>>();

        // for (position, car) in old_grid.iter_mut() {
        for car in &mut self.cars {
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

            // calculate the next position
            let next_position = old_position.next();

            let next_position = match next_position {
                NextCarPosition::OnlyStraight(next) => next,
                NextCarPosition::MustChoose(possible_decisions) => {
                    // the car must choose where to turn
                    let decision = car.props.agent.turn(&old_position, &possible_decisions);
                    old_position.take_decision(decision)
                }
            };

            if next_position == old_position {
                panic!("car stayed still"); // tmp
                continue; // the car stays still, nothing to do
            }

            // if there is a car already there -> don't move there, cause that
            // car might not move
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

        // put the cars into the new grid
        // for (position, next_position) in cars_to_move {
        // let mut car = old_grid.remove(&position).unwrap();

        // move the cars
        for car in &mut self.cars {
            let Some(next_position) = cars_to_move.remove(&car.position) else {
                continue; // car stays still
            };
            assert_ne!(car.position, next_position);

            car.position = next_position;
            // let previous_car = self.grid.insert(next_position, car);
            // assert!(previous_car.is_none());
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
                self.cars.push(car);
                self.taken_positions.insert(position);
            }
        }

        // check we didn't lose any cars in the process
        assert_eq!(cars_count + new_cars_count, self.cars.len());
        assert_eq!(self.taken_positions.len(), self.cars.len());
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

    /*
    pub fn tick(&self) {

        // only process max 100ms per tick
        // let time_since_last_tick = sel.last_tick.elapsed();
        // let time_since_last_tick = time_since_last_tick.max(Duration::from_millis(100));

        for car in sel.cars.iter() {
            let car_state = car.borrow().state.clone();
            let new_state = match car_state {
                CarState::NotSpawnedYet => sel.find_empty_space(),
                CarState::Straight {
                    position,
                    mut progress,
                } => 'straight: {
                    progress += time_since_last_tick.as_secs_f32() * Car::STRAIGHT_SPEED;
                    if progress < 100. {
                        break 'straight CarState::Straight { position, progress };
                    }

                    // the car is at a turn. ask it what it wants to do
                    let available_actions = self.available_actions(&position);
                    let decision = car.borrow().agent.turn(&position, &available_actions);
                    let new_position = self.process_decision(&position, decision);

                    // todo should progress > 200 be handled? in case of lag spike
                    let overflow = progress - 100.;
                    let turn_progress = overflow / Car::STRAIGHT_SPEED * Car::TURN_SPEED;

                    CarState::Turning {
                        from: position,
                        to: new_position,
                        progress: turn_progress,
                    }
                }
                CarState::Turning {
                    from,
                    to,
                    mut progress,
                } => 'turning: {
                    progress += time_since_last_tick.as_secs_f32() * Car::TURN_SPEED;
                    if progress < 100. {
                        break 'turning CarState::Turning { from, to, progress };
                    }

                    let overflow = progress - 100.;
                    let straight_progress = overflow / Car::TURN_SPEED * Car::STRAIGHT_SPEED;

                    CarState::Straight {
                        position: to,
                        progress: straight_progress,
                    }
                }
            };

            if let CarState::Straight { position, progress } = &new_state {
                let a = sel.grid.entry(position.clone()).or_insert_with(Vec::new);
            }

            car.borrow_mut().state = new_state;
        }
    }

    fn available_actions(&self, position: &CarPosition) -> Vec<CarDecision> {
        let mut available_actions = Vec::with_capacity(3);

        match position.orientation() {
            CarOrientation::Right => {
                if position.next_parallel_road.index < Self::VERTICAL_ROADS - 1 {
                    available_actions.push(CarDecision::GoStraight);
                }
                if position.current_road.index > 0 {
                    available_actions.push(CarDecision::TurnLeft);
                }
                if position.current_road.index < Self::HORIZONTAL_ROADS - 1 {
                    available_actions.push(CarDecision::TurnRight);
                }
            }
            CarOrientation::Left => {
                if position.next_parallel_road.index > 0 {
                    available_actions.push(CarDecision::GoStraight);
                }
                if position.current_road.index > 0 {
                    available_actions.push(CarDecision::TurnRight);
                }
                if position.current_road.index < Self::HORIZONTAL_ROADS - 1 {
                    available_actions.push(CarDecision::TurnLeft);
                }
            }
            CarOrientation::Down => {
                if position.next_parallel_road.index < Self::HORIZONTAL_ROADS - 1 {
                    available_actions.push(CarDecision::GoStraight);
                }
                if position.current_road.index > 0 {
                    available_actions.push(CarDecision::TurnRight);
                }
                if position.current_road.index < Self::VERTICAL_ROADS - 1 {
                    available_actions.push(CarDecision::TurnLeft);
                }
            }
            CarOrientation::Up => {
                if position.next_parallel_road.index > 0 {
                    available_actions.push(CarDecision::GoStraight);
                }
                if position.current_road.index > 0 {
                    available_actions.push(CarDecision::TurnLeft);
                }
                if position.current_road.index < Self::VERTICAL_ROADS - 1 {
                    available_actions.push(CarDecision::TurnRight);
                }
            }
        }

        available_actions
    }

    fn create_road_id<T: Into<RoadOrientation>>(&self, index: isize, orientation: T) -> RoadId {
        assert!(index < 0, "Car tried to crash into the edge!");
        RoadId::new(index as usize, orientation.into())
    }

    fn next_road_id(&self, current_road: &RoadId, direction: CarOrientation) -> RoadId {
        if current_road.orientation != direction.into() {
            panic!("Invalid direction for road orientation!")
        }

        let new_road_index_offset = match direction {
            CarOrientation::Right | CarOrientation::Down => 1,
            CarOrientation::Left | CarOrientation::Up => -1,
        };
        self.create_road_id(
            current_road.index as isize + new_road_index_offset,
            direction,
        )
    }

    fn process_decision(&self, position: &CarPosition, decision: CarDecision) -> CarPosition {
        match decision {
            CarDecision::GoStraight => {
                let next_road =
                    self.next_road_id(&position.next_parallel_road, position.orientation());

                CarPosition {
                    current_road: position.current_road.clone(),
                    prev_parallel_road: position.next_parallel_road.clone(),
                    next_parallel_road: next_road,
                }
            }
            CarDecision::TurnLeft | CarDecision::TurnRight => {
                let new_car_orientation = position.orientation().apply_decision(decision);

                let current_road = position.next_parallel_road.clone();
                let prev_parallel_road = position.current_road.clone();
                let next_parallel_road =
                    self.next_road_id(&prev_parallel_road, new_car_orientation);

                CarPosition {
                    current_road,
                    prev_parallel_road,
                    next_parallel_road,
                }
            }
        }
    }
    */
}

/*
impl Grid {
    fn find_empty_space(&self) -> CarState {
        let mut rng = rand::thread_rng();

        // todo: return NotSpawnedYet if unable to find a spot?

        // todo: instead of doing 1000 random tries, take vec of 0..vertical_roads
        // and shuffle it, do same for horizontal roads, and go through the roads in
        // that order
        for _ in 0..1000 {
            // pick a random road
            let (orientation, other_orientation, road_index, section_index) =
                match rng.gen_bool(0.5) {
                    true => (
                        RoadOrientation::Horizontal,
                        RoadOrientation::Vertical,
                        rng.gen_range(0..Grid::HORIZONTAL_ROADS),
                        rng.gen_range(0..Grid::VERTICAL_ROADS - 1),
                    ),
                    false => (
                        RoadOrientation::Vertical,
                        RoadOrientation::Horizontal,
                        rng.gen_range(0..Grid::VERTICAL_ROADS),
                        rng.gen_range(0..Grid::HORIZONTAL_ROADS - 1),
                    ),
                };
            let road_id = RoadId {
                orientation,
                index: road_index,
            };

            // pick a random direction
            let mut prev_parallel_road = RoadId {
                orientation: other_orientation,
                index: section_index,
            };
            let mut next_parallel_road = RoadId {
                orientation: other_orientation,
                index: section_index + 1,
            };
            if rng.gen::<bool>() {
                (prev_parallel_road, next_parallel_road) = (next_parallel_road, prev_parallel_road);
            }

            // we have our road section
            let car_position = CarPosition {
                current_road: road_id,
                next_parallel_road,
                prev_parallel_road,
            };

            // pick a part of the road with no other cars
            let cars_on_this_section = &self.grid[&car_position];
            let margin_between_cars = Car::STRAIGHT_SPEED * Grid::MAX_TICK_LENGTH.as_secs_f32();

            let mut available_ranges = vec![0.0f32..1.0];
            for other_car in cars_on_this_section {
                let other_car = other_car.borrow();
                let CarState::Straight {
                    position: _,
                    progress,
                } = &other_car.state
                else {
                    continue;
                };

                let unavailable_range =
                    progress - margin_between_cars..progress + margin_between_cars;

                // substract unavailable_range from available_ranges
                let mut new_available_ranges = Vec::new();
                for available_range in available_ranges {
                    if unavailable_range.start > available_range.end
                        || unavailable_range.end < available_range.start
                    {
                        continue; // these ranges don't intersect
                    }

                    if available_range.start > unavailable_range.start
                        && available_range.end < unavailable_range.end
                    {
                        continue; // the unavailable range completely covers the available one
                    }

                    if available_range.start < unavailable_range.start
                        && available_range.end > unavailable_range.end
                    {
                        // unavailable is entirely self=contained by available,
                        // split available in two
                        new_available_ranges.push(available_range.start..unavailable_range.end);
                        new_available_ranges.push(unavailable_range.end..available_range.end);
                        continue;
                    }

                    if unavailable_range.start < available_range.start
                        && available_range.contains(&unavailable_range.end)
                    {
                        // unavailable straddles the beginning of available
                        new_available_ranges.push(unavailable_range.end..available_range.end);
                        continue;
                    }

                    if available_range.contains(&unavailable_range.start)
                        && unavailable_range.end > available_range.end
                    {
                        // unavailable straddles the end of available
                        new_available_ranges.push(available_range.start..unavailable_range.start);
                        continue;
                    }

                    panic!("None of the range conditions matched!");
                }

                available_ranges = new_available_ranges;
            }

            if available_ranges.iter().any(|range| range.is_empty()) {
                panic!("Empty range in available_ranges");
            }

            if available_ranges.is_empty() {
                continue; // road section is full
            }

            let chosen_range = available_ranges
                .choose_weighted(&mut rng, |range| range.end - range.start)
                .expect("Error choosing a range from the list")
                .clone();

            let progress = rng.gen_range(chosen_range);
            return CarState::Straight {
                position: car_position,
                progress,
            };
        }

        panic!("No place found for the new car after 1000 iterations");
    }
}
*/
