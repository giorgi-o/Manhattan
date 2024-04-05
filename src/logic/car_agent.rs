use crate::{
    logic::util::RoadSection,
    python::bridge::{
        bridge::{PyAction, PythonAgentWrapper},
        py_grid::PyGridState,
    },
};
use rand::{seq::SliceRandom, Rng};
use std::{io::Write, sync::Mutex};

use super::{
    car::{Car, CarDecision, CarId, CarPassenger, CarPosition},
    grid::Grid,
    passenger::{Passenger, PassengerId},
    pathfinding::Path,
    util::Direction,
};

// pub trait CarAgent: Send + Sync {
pub trait CarAgent: Send + std::fmt::Debug {
    fn get_turn(&mut self, grid: &mut Grid, car_id: CarId) -> CarDecision;
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
pub trait CarPathAgent: CarAgent {
    // pick a destination, generate a path, and store it.
    // grid and car are &mut because passengers can move from
    // the grid to the car.
    fn calculate_path(&mut self, grid: &mut Grid, car_id: CarId);

    // return the previously generated path if there is one.
    fn get_path(&self) -> Option<&Path> {
        None
    }

    fn as_py_agent(&self) -> Option<&PythonAgent> {
        None
    }
}

impl<T: CarPathAgent> CarAgent for T {
    fn get_turn(&mut self, grid: &mut Grid, car_id: CarId) -> CarDecision {
        self.calculate_path(grid, car_id);
        let Some(path) = self.get_path() else {
            // turn randomly
            return RandomTurns {}.get_turn(grid, car_id);
        };

        if path.next_decision().is_none() {
            // bug
            return RandomTurns {}.get_turn(grid, car_id);
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
#[derive(Debug)]
pub struct NullAgent {}

impl CarAgent for NullAgent {
    fn get_turn(&mut self, _grid: &mut Grid, _car_id: CarId) -> CarDecision {
        unreachable!()
    }
}

#[derive(Debug)]
pub struct RandomTurns {}

impl CarAgent for RandomTurns {
    fn get_turn(&mut self, grid: &mut Grid, car_id: CarId) -> CarDecision {
        let car_position = grid.car_position(car_id);
        let options = car_position.road_section.possible_decisions();
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
    fn calculate_path(&mut self, grid: &mut Grid, car_id: CarId) {
        loop {
            let car = grid.car(car_id);
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
    pub fn pick_passenger<'g>(&self, grid: &'g Grid, car_id: CarId) -> Option<&'g Passenger> {
        let waiting_passengers = grid.unassigned_passengers();
        if waiting_passengers.is_empty() {
            // no passengers waiting to be picked up
            return None;
        }

        // find closest one
        let closest_passenger = waiting_passengers
            .into_iter()
            .min_by_key(|p| {
                let path_to_passenger = grid.car(car_id).find_path(p.start);
                path_to_passenger.cost
            })
            .unwrap();
        Some(closest_passenger)
    }
}

impl CarPathAgent for NearestPassenger {
    fn calculate_path(&mut self, grid: &mut Grid, car_id: CarId) {
        if grid.car(car_id).passengers.is_empty() {
            // assign ourselves to the closest passenger
            let closest_passenger = self.pick_passenger(grid, car_id);
            let Some(closest_passenger) = closest_passenger else {
                // no available passengers, just roam randomly
                let mut random_agent = RandomDestination::default();
                random_agent.calculate_path(grid, car_id);
                self.path = random_agent.get_path().cloned();
                return;
            };

            grid.assign_car_to_passenger(car_id, closest_passenger.id);
        }

        let car = grid.car(car_id);
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

    pub fn end_of_tick(&self, new_state: PyGridState) {
        let mut guard = self.half_transitions.lock().unwrap();

        let (old_state, action) = match guard.take() {
            Some((old_state, action)) => (Some(old_state), Some(action)),
            None => (None, None), // first tick, car just spawned
        };

        self.python_wrapper
            .transition_happened(old_state, new_state);
    }
}

impl CarPathAgent for PythonAgent {
    fn calculate_path(&mut self, grid: &mut Grid, car_id: CarId) {
        let py_state = grid.py_state(car_id);
        let py_action = self.python_wrapper.get_action(py_state.clone());

        let half_transition = (py_state, py_action.clone());
        let mut guard = self.half_transitions.lock().unwrap();
        assert!(guard.is_none());
        *guard = Some(half_transition);

        let car = grid.car_mut(car_id);
        car.took_action(py_action.clone());

        let agent_action: AgentAction = py_action.into();
        let agent_action_dbg = format!("{:?}", agent_action);
        print!("{agent_action_dbg: <25} ");
        std::io::stdout().flush().unwrap();

        match agent_action {
            AgentAction::PickUp(passenger_id) => {
                grid.assign_car_to_passenger(car_id, passenger_id);

                let passenger = grid
                    .get_idle_passenger(passenger_id)
                    .expect("Tried picking up passenger not on the grid");

                let car = grid.car(car_id);
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
