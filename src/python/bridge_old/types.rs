use std::sync::{Arc, Mutex};

use macroquad::color::{Color, BLUE, RED};
use pyo3::prelude::*;

use crate::{
    logic::{
        car::{Car, CarAgent, CarPassenger, CarPosition, CarProps, PythonAgent, RandomDestination},
        grid::Grid,
        passenger::{Passenger, PassengerId},
        pathfinding::Path,
    },
    render::render_main::GridBridge,
};

use super::{
    bridge::{agent_cb, transition_cb},
    err_handling::UnwrapPyErr,
};

pub fn start_of_tick(grid: &Grid) {
    let py_grid = PyGridState::from(grid);

    let transition_cb = transition_cb();
    Python::with_gil(|py| {
        transition_cb.call1(py, (py_grid,)).unwrap_py();
    });
}

#[pyclass(name = "Grid")]
pub struct PyGrid {
    grid_bridge: GridBridge,
}

impl PyGrid {
    fn grid(&self) -> std::sync::MutexGuard<'_, Grid> {
        self.grid_bridge.mutex.lock().unwrap()
    }

    fn add_car(
        &mut self,
        agent: impl CarAgent + Send + Sync + 'static,
        position: Option<CarPosition>,
        colour: Color,
    ) {
        let props = CarProps::new(agent, CarProps::SPEED, colour);
        self.grid().add_car(props, position);
    }
}

#[pymethods]
impl PyGrid {
    #[new]
    fn py_new(
        initial_passengers: u32,
        passenger_spawn_rate: f32,
        agent_car_count: u32,
        npc_car_count: u32,
        render: bool,
    ) -> Self {
        

        // println!("PyGrid::new(render={render})");

        let grid = Grid::new();

        let mutex = Arc::new(Mutex::new(grid));
        let grid_bridge = GridBridge { mutex };

        if render {
            crate::render::render_main::start(grid_bridge.clone());
        }

        Self { grid_bridge }
    }

    fn add_npc_car(&mut self, position: Option<CarPosition>) {
        let agent = RandomDestination::default();
        self.add_car(agent, position, BLUE);
    }

    fn add_agent_car(&mut self, position: Option<CarPosition>) {
        // let agent = todo!();
        // self.add_car(agent, position);

        let agent_callback = move |grid: &mut Grid, car: &mut Car| -> PassengerId {
            let py_grid = PyGridState::from(&*grid);
            let py_car = PyCar::from(&*car);

            Python::with_gil(|py| {
                let agent_cb = agent_cb();
                agent_cb.call1(py, (py_car,)).unwrap_py();

                let passengers = grid.unassigned_passengers();
                passengers[0].id
            })
        };

        let agent = PythonAgent::new(agent_callback);
        self.add_car(agent, position, RED);
    }

    fn state(&self) -> PyGridState {
        PyGridState::from(&*self.grid())
    }

    fn tick(&mut self) {
        self.grid().tick();
    }

    fn done(&self) -> bool {
        let grid = self.grid();

        if !grid.unassigned_passengers().is_empty() {
            return false;
        }

        let all_cars_empty = grid.cars().all(|c| c.passengers.is_empty());
        all_cars_empty
    }
}

#[pyclass]
struct PyGridState {
    #[pyo3(get)]
    roads_count: (usize, usize),
    #[pyo3(get)]
    cars: Vec<PyCar>,
    #[pyo3(get)]
    waiting_passengers: Vec<PyPassenger>,
}

impl From<&Grid> for PyGridState {
    fn from(grid: &Grid) -> Self {
        Self {
            roads_count: (Grid::HORIZONTAL_ROADS, Grid::VERTICAL_ROADS),
            cars: grid.cars().map(PyCar::from).collect(),
            waiting_passengers: grid.waiting_passengers().map(PyPassenger::from).collect(),
        }
    }
}

impl PyGridState {
    // fn get(&self) -> std::sync::MutexGuard<'_, Grid> {
    //     self.grid.mutex.lock().unwrap()
    // }
}

/*
#[pymethods]
impl PyGridState {
    #[getter]
    fn roads_count(&self) -> (usize, usize) {
        (Grid::HORIZONTAL_ROADS, Grid::VERTICAL_ROADS)
    }

    #[getter]
    fn cars(&self) -> PyResult<Vec<PyCar>> {
        let py_cars: Vec<PyCar> = self.get().cars().map(PyCar::from).collect();

        Ok(py_cars)
    }
}
*/

#[derive(Clone)]
#[pyclass(name = "Car")]
struct PyCar {
    #[pyo3(get)]
    speed: usize,

    #[pyo3(get)]
    position: CarPosition,
    #[pyo3(get)]
    ticks_since_last_movement: usize,

    #[pyo3(get)]
    riding_passengers: Vec<PyPassenger>,
    // #[pyo3(get)]
    // picking_up_passenger: Option<PyPassenger>,
    // #[pyo3(get)]
    // dropping_off_passenger: Option<usize /* passenger id */>,
}

#[pymethods]
impl PyCar {
    fn distance(&self, pos: CarPosition) -> usize {
        let path = Path::find(self.position, pos, self.speed);
        path.cost
    }

    fn distance_from(&self, from: CarPosition, to: CarPosition) -> usize {
        let path = Path::find(from, to, self.speed);
        path.cost
    }
}

impl From<&Car> for PyCar {
    fn from(car: &Car) -> Self {
        let riding_passengers = car
            .passengers
            .iter()
            .filter_map(|p| match p {
                CarPassenger::DroppingOff(p) => Some(PyPassenger::from(p)),
                CarPassenger::PickingUp(_) => None,
            })
            .collect();

        Self {
            speed: car.props.speed,

            position: car.position,
            ticks_since_last_movement: car.ticks_since_last_movement,

            riding_passengers,
        }
    }
}

#[derive(Clone)]
#[pyclass(name = "Passenger")]
struct PyPassenger {
    #[pyo3(get)]
    id: usize,
    #[pyo3(get)]
    start: CarPosition,
    #[pyo3(get)]
    destination: CarPosition,
}

impl From<&Passenger> for PyPassenger {
    fn from(p: &Passenger) -> Self {
        Self {
            id: p.id.inner(),
            start: p.start,
            destination: p.destination,
        }
    }
}
