use std::{
    borrow::BorrowMut,
    cell::RefCell,
    collections::HashMap,
    ops::{Deref, DerefMut},
    panic,
    rc::Rc,
    time::{Duration, Instant},
};

use rand::{seq::SliceRandom, Rng};

use crate::logic::car::CarState;

use super::car::{Car, CarAgent, CarDecision, CarOrientation, CarPosition};

#[derive(Clone)]
pub struct Grid(Rc<RefCell<GridInner>>);

impl Grid {
    pub fn borrow(&self) -> impl Deref<Target = GridInner> + DerefMut + '_ {
        self.0.deref().borrow_mut()
    }
}

pub struct GridInner {
    cars: Vec<RefCell<Car>>,
    // note: when a car is turning, it says on the road it comes from until
    // it has finished turning
    grid: HashMap<CarPosition, Vec<RefCell<Car>>>,

    last_tick: Instant,
}
#[derive(Hash, PartialEq, Eq, Clone)]
pub struct RoadId {
    pub orientation: RoadOrientation,
    pub index: usize,
}

impl RoadId {
    pub fn new(index: usize, orientation: RoadOrientation) -> Self {
        let max_index = match orientation {
            RoadOrientation::Horizontal => Grid::HORIZONTAL_ROADS,
            RoadOrientation::Vertical => Grid::VERTICAL_ROADS,
        } - 1;
        assert!(index <= max_index, "Car tried to crash into the edge!");

        Self { orientation, index }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoadOrientation {
    Horizontal,
    Vertical,
}

impl Grid {
    const HORIZONTAL_ROADS: usize = 5;
    const VERTICAL_ROADS: usize = 7;

    const MAX_TICK_LENGTH: Duration = Duration::from_millis(100);

    pub fn new() -> Self {
        let inner = GridInner {
            grid: HashMap::new(),
            cars: Vec::new(),
            last_tick: Instant::now(),
        };
        let this = RefCell::new(inner);
        let this = Rc::new(this);
        Self(this)
    }

    pub fn vertical_roads() -> usize {
        Self::VERTICAL_ROADS
    }

    pub fn horizontal_roads() -> usize {
        Self::HORIZONTAL_ROADS
    }

    pub fn add_car(&self, car_agent: impl CarAgent + 'static) {
        let car = Car::new(car_agent);
        let car = RefCell::new(car);
        self.borrow().cars.push(car);
    }

    pub fn tick(&self) {
        let this = self.borrow();

        // only process max 100ms per tick
        let time_since_last_tick = this.last_tick.elapsed();
        let time_since_last_tick = time_since_last_tick.max(Duration::from_millis(100));

        for car in this.cars.iter() {
            let car_state = car.borrow().state.clone();
            let new_state = match car_state {
                CarState::NotSpawnedYet => this.find_empty_space(),
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
                let a = this.grid.entry(position.clone()).or_insert_with(Vec::new);
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
}

impl GridInner {
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
