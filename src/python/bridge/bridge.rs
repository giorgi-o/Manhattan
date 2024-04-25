use std::sync::OnceLock;

use pyo3::{prelude::*, types::PyList};

use crate::{
    logic::{
        car::CarPosition,
        car_agent::AgentAction,
        ev::ChargingStationId,
        grid::{Grid, GridOpts},
        util::Direction,
    },
    render::render_main::GridLock,
};

use super::{
    err_handling::UnwrapPyErr,
    py_grid::{PyCarType, PyChargingStation, PyCoords, PyGridState, PyPassenger},
};

static MAIN_MODULE: OnceLock<Py<PyModule>> = OnceLock::new();

pub fn initialise_python() {
    pyo3::prepare_freethreaded_python();

    // find "manhattan" dir, regardless of cwd
    let exe_path = std::env::current_exe().unwrap();
    #[rustfmt::skip]
    let manhattan_dir = exe_path
        .parent().unwrap() // debug/release
        .parent().unwrap() // target
        .parent().unwrap(); // manhattan

    let res = Python::with_gil(|py| -> PyResult<()> {
        // add src/python/src to sys.path
        let src_dir = manhattan_dir.join("src").join("python").join("src");
        let sys = py.import_bound("sys")?;
        let sys_path = sys.getattr("path")?;
        sys_path.call_method1("append", (src_dir,))?;

        // import main
        let main = py.import_bound("main")?;
        let main = main.unbind();
        MAIN_MODULE.set(main).unwrap();

        Ok(())
    });

    res.unwrap_py();
}

pub fn main_module<'py>(py: Python<'py>) -> &Bound<'py, PyModule> {
    let main = MAIN_MODULE.get().expect("Main module not imported!");
    main.bind(py)
}

pub fn start_python() {
    let res: PyResult<()> = {
        Python::with_gil(|py| {
            let main_module = main_module(py);
            let rust_module = exported_python_module(py)?;

            let start = main_module.getattr("start")?;
            start.call1((rust_module,))?;

            Ok(())
        })
    };

    if let Err(e) = res {
        Python::with_gil(|py| {
            panic!(
                "{}\n{}",
                e,
                e.traceback_bound(py).unwrap().format().unwrap()
            );
        })
    }
}

#[pyfunction]
fn grid_dimensions() -> (usize, usize) {
    (Grid::VERTICAL_ROADS, Grid::HORIZONTAL_ROADS)
}

#[pyfunction]
fn calculate_distance(a: PyCoords, b: PyCoords) -> usize {
    let a: CarPosition = a.into();
    let b: CarPosition = b.into();

    a.distance_to(b)
}

fn exported_python_module<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyModule>> {
    let module = PyModule::new_bound(py, "rust")?;

    module.add_class::<PyGridEnv>()?;
    module.add_class::<GridOpts>()?;
    module.add_class::<PyAction>()?;
    module.add_class::<Direction>()?;
    module.add_class::<PyCarType>()?;
    module.add_class::<CarPosition>()?;

    module.add_function(wrap_pyfunction!(grid_dimensions, &module)?)?;
    module.add_function(wrap_pyfunction!(calculate_distance, &module)?)?;

    Ok(module)
}

#[pyclass]
pub struct PyGridEnv {
    grid_ref: GridLock,
}

#[pymethods]
impl PyGridEnv {
    #[new]
    fn py_new(python_agents: Py<PyList>, opts: GridOpts, render: bool) -> Self {
        let mut agents = vec![];
        Python::with_gil(|py| {
            let python_agents = python_agents.bind(py);
            for agent_obj in python_agents.iter() {
                let agent = PythonAgentWrapper::new(agent_obj.to_object(py));
                agents.push(agent);
            }
        });

        let grid = Grid::new(opts, agents);
        let grid_ref = GridLock::new(grid);

        if render {
            let grid_ref = grid_ref.clone();
            std::thread::spawn(move || {
                crate::render::render_main::new_grid(grid_ref);
            });
        }

        Self { grid_ref }
    }

    fn tick(&self) {
        self.grid_ref.lock().tick();
    }
}

#[derive(Clone)]
pub struct PythonAgentWrapper {
    py_obj: PyObject,
}

impl PythonAgentWrapper {
    fn new(py_obj: PyObject) -> Self {
        // check it has all the required methods
        Python::with_gil(|py| {
            let obj = py_obj.bind(py);
            assert!(obj.hasattr("get_action").unwrap());
            assert!(obj.hasattr("transition_happened").unwrap());
        });

        Self { py_obj }
    }

    pub fn get_action(&self, state: PyGridState) -> PyAction {
        assert!(state.has_pov());

        Python::with_gil(|py| {
            let obj = self.py_obj.bind(py);

            let action = obj.call_method1("get_action", (state,)).unwrap_py();
            let action: PyAction = action.extract().unwrap();

            action.assert_valid();
            action
        })
    }

    pub fn transition_happened(&self, state: Option<PyGridState>, new_state: PyGridState) {
        Python::with_gil(|py| {
            let obj = self.py_obj.bind(py);

            obj.call_method1("transition_happened", (state, new_state))
                .unwrap_py();
        });
    }
}

// might want to replace with pytorch tensor or something
type RawAction = Option<usize>;

#[derive(Clone, Copy, Hash, Debug)]
#[pyclass]
pub struct PyAction {
    #[pyo3(get)]
    raw: RawAction,
    #[pyo3(get)]
    pub pick_up_passenger: Option<(PyPassenger, usize /* n closest */)>,
    #[pyo3(get)]
    pub drop_off_passenger: Option<(PyPassenger, usize /* n closest */)>,
    #[pyo3(get)]
    head_towards: Option<Direction>,
    #[pyo3(get)]
    charge_battery: Option<ChargingStationId>,
}

impl Default for PyAction {
    fn default() -> Self {
        Self {
            raw: None,
            pick_up_passenger: None,
            drop_off_passenger: None,
            head_towards: None,
            charge_battery: None,
        }
    }
}

#[pymethods]
impl PyAction {
    #[staticmethod]
    fn pick_up_passenger(passenger: PyPassenger, raw: RawAction, n_closest: usize) -> Self {
        Self {
            raw,
            pick_up_passenger: Some((passenger, n_closest)),
            ..Default::default()
        }
    }

    #[staticmethod]
    fn drop_off_passenger(passenger: PyPassenger, raw: RawAction, n_closest: usize) -> Self {
        Self {
            raw,
            drop_off_passenger: Some((passenger, n_closest)),
            ..Default::default()
        }
    }

    #[staticmethod]
    fn head_towards(dir: Direction, raw: RawAction) -> Self {
        Self {
            raw,
            head_towards: Some(dir),
            ..Default::default()
        }
    }

    #[staticmethod]
    fn charge_battery(charging_station: PyChargingStation, raw: RawAction) -> Self {
        Self {
            raw,
            charge_battery: Some(charging_station.id),
            ..Default::default()
        }
    }

    pub fn assert_valid(&self) {
        let mut somes = 0;
        self.pick_up_passenger.is_some().then(|| somes += 1);
        self.drop_off_passenger.is_some().then(|| somes += 1);
        self.head_towards.is_some().then(|| somes += 1);
        self.charge_battery.is_some().then(|| somes += 1);
        assert_eq!(somes, 1);
    }

    fn is_pick_up(&self) -> bool {
        self.assert_valid();
        self.pick_up_passenger.is_some()
    }

    fn is_drop_off(&self) -> bool {
        self.assert_valid();
        self.drop_off_passenger.is_some()
    }

    fn is_head_towards(&self) -> bool {
        self.assert_valid();
        self.head_towards.is_some()
    }

    fn is_charge(&self) -> bool {
        self.assert_valid();
        self.charge_battery.is_some()
    }

    fn __eq__(&self, other: &Self) -> bool {
        self == other
    }

    fn __repr__(&self) -> String {
        let agent_action: AgentAction = self.into();
        format!("<PyAction {:?}>", agent_action)
    }
}

impl PartialEq for PyAction {
    fn eq(&self, other: &Self) -> bool {
        AgentAction::from(self) == AgentAction::from(other)
    }
}

impl Eq for PyAction {}

impl From<PyAction> for AgentAction {
    fn from(py_action: PyAction) -> Self {
        AgentAction::from(&py_action)
    }
}

impl From<&PyAction> for AgentAction {
    fn from(py_action: &PyAction) -> Self {
        py_action.assert_valid();

        if let Some((passenger, _)) = &py_action.pick_up_passenger {
            Self::PickUp(passenger.id)
        } else if let Some((passenger, _)) = &py_action.drop_off_passenger {
            Self::DropOff(passenger.id)
        } else if let Some(head_towards) = py_action.head_towards {
            Self::HeadTowards(head_towards)
        } else if let Some(charge_battery) = py_action.charge_battery {
            Self::ChargeBattery(charge_battery)
        } else {
            unreachable!()
        }
    }
}
