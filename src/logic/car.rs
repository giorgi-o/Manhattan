use macroquad::color::Color;
use rand::{seq::SliceRandom, Rng};

use super::{
    grid::{Grid, RoadSection},
    passenger::{Passenger, PassengerId},
    pathfinding::Path,
};

use macroquad::color::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CarPosition {
    pub road_section: RoadSection,
    pub position_in_section: usize, // higher = further along
}

pub enum NextCarPosition {
    OnlyStraight(CarPosition),
    MustChoose(Vec<CarDecision>),
}

impl CarPosition {
    pub fn random(mut rng: impl Rng) -> Self {
        let road_section = RoadSection::random(&mut rng);

        Self {
            position_in_section: rng
                .gen_range(0..=road_section.direction.max_position_in_section()),
            road_section,
        }
    }

    pub fn next(&self) -> NextCarPosition {
        let next_position = self.position_in_section + 1;

        let max_position = self.road_section.direction.max_position_in_section();
        if next_position > max_position {
            // reached end of section, needs to make a decision
            let possible_decisions = self.road_section.possible_decisions();
            return NextCarPosition::MustChoose(possible_decisions);
        }

        let next = Self {
            road_section: self.road_section,
            position_in_section: next_position,
        };
        NextCarPosition::OnlyStraight(next)
    }

    pub fn take_decision(&self, decision: CarDecision) -> Self {
        let new_road_section = self.road_section.take_decision(decision).unwrap();

        Self {
            road_section: new_road_section,
            position_in_section: 0,
        }
    }
}

pub struct CarProps {
    pub agent: Box<dyn CarAgent + Send + Sync>,
    pub colour: Color,
    pub speed: usize, // ticks per movement
}

impl CarProps {
    pub fn new(agent: impl CarAgent + Send + Sync + 'static, speed: usize) -> Self {
        Self {
            agent: Box::new(agent),
            colour: Self::random_colour(),
            speed,
        }
    }

    fn random_colour() -> Color {
        let mut rng = rand::thread_rng();
        // Color {
        //     r: rng.gen(),
        //     g: rng.gen(),
        //     b: rng.gen(),
        //     a: 1.0,
        // }

        const POSSIBLE_COLOURS: &[Color] = &[BLUE, RED, PURPLE];
        *POSSIBLE_COLOURS.choose(&mut rng).unwrap()
    }
}

pub enum CarPassenger {
    None,
    PickingUp(PassengerId),
    DroppingOff(Passenger),
}

impl CarPassenger {
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::None)
    }

    pub fn is_picking_up(&self) -> bool {
        matches!(self, Self::PickingUp(_))
    }

    pub fn is_dropping_off(&self) -> bool {
        matches!(self, Self::DroppingOff(_))
    }
}

pub struct Car {
    pub props: CarProps,

    // variable data
    pub position: CarPosition,
    pub ticks_since_last_movement: usize,
    pub passenger: CarPassenger,
}

impl Car {
    // speed unit: "progress" per second
    // pub const STRAIGHT_SPEED: f32 = 0.3;
    // pub const TURN_SPEED: f32 = 1.;
    // pub const SPEED: f32 = 0.3; // progress per second

    // pub fn new(position: CarPosition, agent: impl CarAgent + Send + Sync + 'static) -> Self {
    //     Self {
    //         position,
    //         agent: Box::new(agent),
    //     }
    // }

    pub fn new(props: CarProps, position: CarPosition) -> Self {
        Self {
            props,
            position,
            ticks_since_last_movement: 0,
            passenger: CarPassenger::None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CarDecision {
    TurnLeft,
    GoStraight,
    TurnRight,
}

pub trait CarAgent {
    fn turn(&mut self, grid: &mut Grid, car: &mut Car) -> CarDecision;
    fn path(&self) -> Option<&Path> {
        None
    }
}

// temporary placeholder agent to put instead of the real agent
pub struct NullAgent {}

impl CarAgent for NullAgent {
    fn turn(&mut self, _grid: &mut Grid, _car: &mut Car) -> CarDecision {
        unreachable!()
    }
}

pub struct RandomTurns {}

impl CarAgent for RandomTurns {
    fn turn(&mut self, _grid: &mut Grid, car: &mut Car) -> CarDecision {
        // *options
        //     .choose(&mut rand::thread_rng())
        //     .expect("List of possible car decisions is empty")
        let options = car.position.road_section.possible_decisions();
        *options
            .choose(&mut rand::thread_rng())
            .expect("List of possible car decisions is empty")
    }
}

#[derive(Default)]
pub struct RandomDestination {
    path: Option<Path>,
}

impl CarAgent for RandomDestination {
    fn turn(&mut self, _grid: &mut Grid, car: &mut Car) -> CarDecision {
        if self.path.is_none() {
            let destination = CarPosition::random(&mut rand::thread_rng());
            let path = Path::find(car.position, destination);
            self.path = Some(path);
        }
        let path = self.path.as_mut().unwrap();
        path.pop_next_decision().unwrap_or_else(|| {
            // we already arrived, delete path and recursively call ourselves
            // to create new one
            self.path = None;
            self.turn(_grid, car)
        })
    }

    fn path(&self) -> Option<&Path> {
        self.path.as_ref()
    }
}

#[derive(Default)]
pub struct NearestPassenger {
    path: Option<Path>,
}

impl NearestPassenger {
    pub fn pick_passenger<'g>(&self, grid: &'g Grid, car: &Car) -> Option<&'g Passenger> {
        let waiting_passengers = grid.unassigned_passengers();
        if waiting_passengers.is_empty() {
            // no passengers waiting to be picked up
            return None;
        }

        // find closest one
        // todo: use a* path cost instead of manhattan distance
        let closest_passenger = waiting_passengers
            .into_iter()
            .min_by_key(|p| {
                p.start
                    .road_section
                    .manhattan_distance(car.position.road_section)
            })
            .unwrap();
        Some(closest_passenger)
    }
}

impl CarAgent for NearestPassenger {
    fn turn(&mut self, grid: &mut Grid, car: &mut Car) -> CarDecision {
        let mut path = match &car.passenger {
            CarPassenger::None => {
                // assign ourselves to the closest passenger
                let closest_passenger = self.pick_passenger(grid, car);
                let Some(closest_passenger) = closest_passenger else {
                    return RandomTurns {}.turn(grid, car);
                };
                let closest_passenger_position = closest_passenger.start;

                grid.assign_car_to_passenger(car, closest_passenger.id);

                Path::find(car.position, closest_passenger_position)
            }

            CarPassenger::PickingUp(passenger_id) => {
                let passenger = grid.get_passenger(*passenger_id).unwrap();
                Path::find(car.position, passenger.start)
            }

            CarPassenger::DroppingOff(passenger) => Path::find(car.position, passenger.destination),
        };

        let decision = path.pop_next_decision().unwrap();
        self.path = Some(path);
        decision
    }

    fn path(&self) -> Option<&Path> {
        self.path.as_ref()
    }
}
