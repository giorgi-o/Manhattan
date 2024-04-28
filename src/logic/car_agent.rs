use crate::{
    logic::{car::Car, util::RoadSection},
    python::bridge::{
        bridge::{PyAction, PythonAgentWrapper},
        py_grid::PyGridState,
    },
};
use rand::seq::SliceRandom;
use std::{io::Write, sync::Mutex};

use super::{
    car::{CarDecision, CarId, CarPassenger, CarPosition},
    ev::ChargingStationId,
    grid::Grid,
    grid_util::GridStats,
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

        // if path.next_decision().is_none() {
        //     // bug
        //     // return RandomTurns {}.get_turn(grid, car_id);
        //     panic!("path has no next decision");
        // }

        // let Some(decision) = path.next_decision() else {
        //     println!("{:?}", self);
        //     println!("{:?}", path);
        //     panic!("path has no next decision");
        // };

        let decision = path.next_decision().unwrap_or_else(|| {
            // this should only happen if we are on the same
            // section as the destination
            let car_pos = grid.car_position(car_id);
            assert_eq!(car_pos.road_section, path.destination.road_section);

            if path
                .action
                .is_some_and(|a| matches!(a, AgentAction::ChargeBattery(_)))
            {
                // we are in the charging station, or on the same
                // road section as it.

                if car_pos.is_at_charging_station() {
                    // the path wants to charge, and we are charging.
                    CarDecision::ChargeBattery
                } else if grid.charging_station_entrance_at(car_pos).is_some() {
                    // the path wants to charge, and we are right next
                    // to a charging station.
                    CarDecision::ChargeBattery
                } else {
                    // we are on the charging station's section. keep
                    // going forwards until we get there.
                    // assert!(car_pos.position_in_section < path.destination.position_in_section);
                    // commented out: the car can now reach the charging station
                    // on the opposite side
                    CarDecision::GoStraight
                }
            } else {
                // path has no next decision. this can happen if the
                // path length is 0, for example the car wants to pick
                // up a passenger right next to it.
                // in that case, it doesn't really matter what the
                // decision is, just pick something random.
                RandomTurns {}.get_turn(grid, car_id)
            }
        });

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
        let options = car_position.possible_decisions();
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

            let mut path = car.find_path(destination);
            if path.next_decision().is_none() {
                continue;
            }

            path.action = Some(AgentAction::HeadTowards(Direction::Up));
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
            .min_by_key(|p| grid.car_position(car_id).distance_to(p.start))
            .unwrap();
        Some(closest_passenger)
    }
}

impl CarPathAgent for NearestPassenger {
    fn calculate_path(&mut self, grid: &mut Grid, car_id: CarId) {
        let car = grid.car(car_id);
        if let Some(cs_id) = car.position.in_charging_station {
            if car.battery.get() < 1.0 {
                let cs = grid.charging_stations.get(&cs_id).unwrap();
                let mut path = car.position.path_to(cs.entrance);
                path.action = Some(AgentAction::ChargeBattery(cs_id));
                self.path = Some(path);
                return;
            }
        } else if car.battery.get() < 0.1 {
            let cs_ids_and_paths = grid
                .charging_stations
                .values()
                .filter(|cs| cs.has_space())
                .map(|cs| (cs.id, car.position.path_to(cs.entrance)));
            let cs_id_and_path = cs_ids_and_paths.min_by_key(|(_, p)| p.cost);
            if let Some((cs_id, mut path)) = cs_id_and_path {
                path.action = Some(AgentAction::ChargeBattery(cs_id));
                self.path = Some(path);
                return;
            }
        }

        if car.passengers.is_empty() {
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
                let mut path = car.find_path(passenger.start);

                path.action = Some(AgentAction::PickUp(*passenger_id));
                path
            }

            CarPassenger::DroppingOff(passenger) => {
                let mut path = car.find_path(passenger.destination);
                path.action = Some(AgentAction::DropOff(passenger.id));
                path
            }
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
    ChargeBattery(ChargingStationId),
}

pub struct PythonAgent {
    path: Option<Path>,

    python_wrapper: PythonAgentWrapper,

    // store the state/action pairs, then when we get the new
    // state and reward, we send the full transitions to
    // python to learn from
    half_transitions: Mutex<Option<(PyGridState, PyAction)>>,

    deterministic_agent: Option<NearestPassenger>,
}

impl PythonAgent {
    pub fn new(python_wrapper: PythonAgentWrapper, deterministic_mode: bool) -> Self {
        let deterministic_agent = deterministic_mode.then(|| NearestPassenger::default());

        Self {
            path: None,

            python_wrapper,
            half_transitions: Mutex::new(None),

            deterministic_agent,
        }
    }

    pub fn end_of_tick(&self, new_state: PyGridState) {
        let mut guard = self.half_transitions.lock().unwrap();

        let (old_state, _action) = match guard.take() {
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

        if let Some(agent) = &mut self.deterministic_agent {
            agent.calculate_path(grid, car_id);
            self.path = agent.get_path().cloned();
            return;
        }

        if let Some((_, n_closest)) = py_action.pick_up_passenger {
            grid.stats.ticks_picking_up_n_closest_passenger[n_closest] += 1;
        } else if let Some((_, n_closest)) = py_action.drop_off_passenger {
            grid.stats.ticks_dropping_off_n_closest_passenger[n_closest] += 1;
        }

        // we use this instead of grid.car_mut() so that we only hold the
        // &mut on grid.cars, not the whole grid
        let car = grid.cars.get_mut(&car_id).unwrap();
        car.took_action(py_action.clone());

        let agent_action: AgentAction = py_action.into();
        let agent_action_dbg = format!("{:?}", agent_action);

        let mut path = match agent_action {
            AgentAction::PickUp(passenger_id) => {
                grid.stats.pick_up_requests += 1;

                grid.assign_car_to_passenger(car_id, passenger_id);

                let passenger = grid
                    .get_idle_passenger(passenger_id)
                    .expect("Tried picking up passenger not on the grid");

                let car = grid.car(car_id);
                let path = car.find_path(passenger.start);
                path
            }

            AgentAction::DropOff(passenger_id) => {
                grid.stats.drop_off_requests += 1;

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
                path
            }

            AgentAction::HeadTowards(direction) => {
                grid.stats.head_towards_requests += 1;

                let current_road_section = car.position.road_section;

                let possible_decisions = car.position.possible_decisions();
                let possible_next_positions = possible_decisions
                    .into_iter()
                    .filter(|d| *d != CarDecision::ChargeBattery)
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
                    in_charging_station: None,
                };

                let path = car.find_path(destination);
                path
            }

            AgentAction::ChargeBattery(station_id) => {
                grid.stats.charge_requests += 1;

                let charging_station = grid.charging_stations.get(&station_id).unwrap();

                let positions = [
                    charging_station.entrance,
                    charging_station.entrance.other_side_of_road(),
                ];
                let paths = positions.iter().map(|p| car.find_path(*p));

                let path = paths.min_by_key(|p| p.cost).unwrap();
                path
            }
        };

        path.action = Some(agent_action);
        self.path = Some(path);

        let verbose = grid.opts.verbose;
        let car = grid.car_mut(car_id);

        if matches!(agent_action, AgentAction::HeadTowards(_))
            && car.position.position_in_section == 0
        {
            // the agent just reached where it wanted to HeadTowards
            car.active_action = None;
        } else {
            car.active_action = Some(py_action);
        }

        if verbose {
            let passenger_count = car
                .passengers
                .iter()
                .filter(|p| p.is_dropping_off())
                .count();

            print!("{agent_action_dbg: <25} {passenger_count: <2} ");
            std::io::stdout().flush().unwrap();
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
