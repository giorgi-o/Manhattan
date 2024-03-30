use std::sync::Mutex;

use macroquad::color::Color;
use pyo3::prelude::*;
use rand::{seq::SliceRandom, Rng};

use crate::python::bridge::{
    bridge::{PyAction, PythonAgentWrapper},
    py_grid::{PyCar, PyGridState},
};

use super::{
    grid::{Direction, Grid, RoadSection},
    passenger::{Passenger, PassengerId},
    pathfinding::Path,
};

use macroquad::color::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[pyclass]
pub struct CarPosition {
    #[pyo3(get)]
    pub road_section: RoadSection,
    #[pyo3(get)]
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
    pub agent: Box<dyn CarAgent>,
    pub colour: Color,
    pub speed: usize, // ticks per movement
}

impl CarProps {
    pub const SPEED: usize = 3;

    pub fn new(agent: impl CarAgent + 'static, speed: usize, colour: Color) -> Self {
        Self {
            agent: Box::new(agent),
            colour,
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

#[derive(Debug)]
pub enum CarPassenger {
    PickingUp(PassengerId),
    DroppingOff(Passenger),
}

impl CarPassenger {
    pub fn is_dropping_off(&self) -> bool {
        matches!(self, Self::DroppingOff(_))
    }

    pub fn is_id(&self, id: PassengerId) -> bool {
        match self {
            Self::PickingUp(passenger_id) => *passenger_id == id,
            Self::DroppingOff(passenger) => passenger.id == id,
        }
    }
}

pub struct Car {
    pub props: CarProps,

    // variable data
    pub position: CarPosition,
    pub ticks_since_last_movement: usize,
    pub passengers: Vec<CarPassenger>,
}

impl Car {
    pub fn new(props: CarProps, position: CarPosition) -> Self {
        Self {
            props,
            position,
            ticks_since_last_movement: 0,
            passengers: vec![],
        }
    }

    pub fn find_path(&self, destination: CarPosition) -> Path {
        Path::find(self.position, destination, self.props.speed)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CarDecision {
    TurnLeft,
    GoStraight,
    TurnRight,
}

// pub trait CarAgent: Send + Sync {
pub trait CarAgent: Send {
    fn get_turn(&mut self, grid: &mut Grid, car: &mut Car) -> CarDecision;
    fn as_path_agent(&self) -> Option<&dyn CarPathAgent> {
        None
    }

    fn as_py_agent(&self) -> Option<&PythonAgent> {
        None
    }
    fn is_npc(&self) -> bool {
        self.as_py_agent().is_none()
    }
}
pub trait CarPathAgent: CarAgent + std::fmt::Debug {
    // pick a destination, generate a path, and store it.
    // grid and car are &mut because passengers can move from
    // the grid to the car.
    fn calculate_path(&mut self, grid: &mut Grid, car: &mut Car);

    // return the previously generated path if there is one.
    fn get_path(&self) -> Option<&Path> {
        None
    }

    fn as_py_agent(&self) -> Option<&PythonAgent> {
        None
    }
}

impl<T: CarPathAgent> CarAgent for T {
    fn get_turn(&mut self, grid: &mut Grid, car: &mut Car) -> CarDecision {
        self.calculate_path(grid, car);
        let Some(path) = self.get_path() else {
            // turn randomly
            return RandomTurns {}.get_turn(grid, car);
        };

        if path.next_decision().is_none() {
            // bug
            return RandomTurns {}.get_turn(grid, car);
        }

        let Some(decision) = path.next_decision() else {
            println!("{:?}", self);
            println!("{:?}", path);
            panic!("path has no next decision");
        };

        decision
    }

    fn as_path_agent(&self) -> Option<&dyn CarPathAgent> {
        Some(self)
    }

    fn as_py_agent(&self) -> Option<&PythonAgent> {
        CarPathAgent::as_py_agent(self)
    }
}

// temporary placeholder agent to put instead of the real agent
pub struct NullAgent {}

impl CarAgent for NullAgent {
    fn get_turn(&mut self, _grid: &mut Grid, _car: &mut Car) -> CarDecision {
        unreachable!()
    }
}

pub struct RandomTurns {}

impl CarAgent for RandomTurns {
    fn get_turn(&mut self, _grid: &mut Grid, car: &mut Car) -> CarDecision {
        let options = car.position.road_section.possible_decisions();
        *options
            .choose(&mut rand::thread_rng())
            .expect("List of possible car decisions is empty")
    }
}

#[derive(Default, Debug)]
pub struct RandomDestination {
    path: Option<Path>,
}

impl CarPathAgent for RandomDestination {
    fn calculate_path(&mut self, grid: &mut Grid, car: &mut Car) {
        loop {
            let destination = CarPosition::random(&mut rand::thread_rng());

            let path = car.find_path(destination);
            if path.next_decision().is_none() {
                continue;
            }

            self.path = Some(path);
            break;
        }
    }

    fn get_path(&self) -> Option<&Path> {
        self.path.as_ref()
    }
}

#[derive(Default, Debug)]
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
        let closest_passenger = waiting_passengers
            .into_iter()
            .min_by_key(|p| {
                let path_to_passenger = car.find_path(p.start);
                path_to_passenger.cost
            })
            .unwrap();
        Some(closest_passenger)
    }
}

impl CarPathAgent for NearestPassenger {
    fn calculate_path(&mut self, grid: &mut Grid, car: &mut Car) {
        if car.passengers.is_empty() {
            // assign ourselves to the closest passenger
            let closest_passenger = self.pick_passenger(grid, car);
            let Some(closest_passenger) = closest_passenger else {
                // no available passengers, just roam randomly
                let mut random_agent = RandomDestination::default();
                random_agent.calculate_path(grid, car);
                self.path = random_agent.get_path().cloned();
                return;
            };

            grid.assign_car_to_passenger(car, closest_passenger.id);
        }

        let first_passenger = &car.passengers[0];

        let path = match &first_passenger {
            CarPassenger::PickingUp(passenger_id) => {
                let passenger = grid.get_idle_passenger(*passenger_id).unwrap();
                car.find_path(passenger.start)
            }

            CarPassenger::DroppingOff(passenger) => car.find_path(passenger.destination),
        };

        self.path = Some(path);
    }

    fn get_path(&self) -> Option<&Path> {
        self.path.as_ref()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AgentAction {
    PickUp(PassengerId),
    DropOff(PassengerId),
    HeadTowards(Direction),
}

pub struct PythonAgent {
    path: Option<Path>,
    python_wrapper: PythonAgentWrapper,

    // store the state/action pairs, then when we get the new
    // state and reward, we send the full transitions to
    // python to learn from
    half_transitions: Mutex<Option<(PyGridState, PyAction)>>,
}

impl PythonAgent {
    pub fn new(python_wrapper: PythonAgentWrapper) -> Self {
        Self {
            path: None,
            python_wrapper,
            half_transitions: Mutex::new(None),
        }
    }

    pub fn end_of_tick(&self, new_state: PyGridState, reward: f32) {
        let mut guard = self.half_transitions.lock().unwrap();
        let Some((old_state, action)) = guard.take() else {
            return; // first tick, car just spawned
        };

        self.python_wrapper
            .transition_happened(old_state, action, new_state, reward);
    }
}

impl CarPathAgent for PythonAgent {
    fn calculate_path(&mut self, grid: &mut Grid, car: &mut Car) {
        let py_state = grid.py_state(car);
        let py_action = self.python_wrapper.get_action(py_state.clone());

        let half_transition = (py_state, py_action.clone());
        let mut guard = self.half_transitions.lock().unwrap();
        assert!(guard.is_none());
        *guard = Some(half_transition);

        let agent_action: AgentAction = py_action.into();
        match agent_action {
            AgentAction::PickUp(passenger_id) => {
                grid.assign_car_to_passenger(car, passenger_id);

                let passenger = grid
                    .get_idle_passenger(passenger_id)
                    .expect("Tried picking up passenger not on the grid");

                let path = car.find_path(passenger.start);
                self.path = Some(path);
            }

            AgentAction::DropOff(passenger_id) => {
                let passenger = car
                    .passengers
                    .iter()
                    .find_map(|p| {
                        let CarPassenger::DroppingOff(p) = p else {
                            return None;
                        };
                        (p.id == passenger_id).then_some(p)
                    })
                    .expect("Tried dropping off passenger not in the car");

                let path = car.find_path(passenger.destination);
                self.path = Some(path);
            }

            AgentAction::HeadTowards(direction) => {
                let current_road_section = car.position.road_section;
                let current_direction = current_road_section.direction;

                let possible_decisions = current_road_section.possible_decisions();
                let possible_next_positions = possible_decisions
                    .into_iter()
                    .filter_map(|d| current_road_section.take_decision(d))
                    .collect::<Vec<_>>();

                let sort_fn = |a: &RoadSection, b: &RoadSection| {
                    let (ax, ay) = a.checkerboard_coords();
                    let (bx, by) = b.checkerboard_coords();

                    match direction {
                        Direction::Up => ay.total_cmp(&by),
                        Direction::Down => by.total_cmp(&ay),
                        Direction::Left => ax.total_cmp(&bx),
                        Direction::Right => bx.total_cmp(&ax),
                    }
                };

                let new_road_section = possible_next_positions.into_iter().min_by(sort_fn).unwrap();

                // let mut new_road_section = current_road_section.clone();

                // let horizontal = direction.is_horizontal();
                // let towards_positive = direction.towards_positive();
                // let offset = if towards_positive { 1 } else { -1 };

                // if horizontal == current_direction.is_horizontal() {
                //     new_road_section.section_index += offset;
                // } else {
                //     new_road_section.road_index += offset;
                // }

                // if new_road_section.valid().is_err() {
                //     new_road_section = current_road_section;
                //     new_road_section.direction = new_road_section.direction.inverted();
                // }

                let destination = CarPosition {
                    road_section: new_road_section,
                    position_in_section: 0,
                };

                let path = car.find_path(destination);
                self.path = Some(path);
            }
        }
    }

    fn get_path(&self) -> Option<&Path> {
        self.path.as_ref()
    }

    fn as_py_agent(&self) -> Option<&PythonAgent> {
        Some(self)
    }
}

impl std::fmt::Debug for PythonAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PythonAgent")
    }
}
