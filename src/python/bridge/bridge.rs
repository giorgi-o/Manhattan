use std::sync::{Arc, Mutex, OnceLock};

use pyo3::{prelude::*, types::PyList};

use crate::{
    logic::{
        car_agent::AgentAction,
        grid::{Grid, GridOpts},
        passenger::PassengerId,
        util::Direction,
    },
    render::render_main::GridRef,
};

use super::{
    err_handling::UnwrapPyErr,
    py_grid::{PyCar, PyCarType, PyGridState, PyPassenger},
};

static MAIN_MODULE: OnceLock<Py<PyAny>> = OnceLock::new();

pub fn initialise_python() {
    pyo3::prepare_freethreaded_python();

    let res = Python::with_gil(|py| -> PyResult<()> {
        // add src/python/src to sys.path
        let cwd = std::env::current_dir().unwrap();
        let src_dir = cwd.join("src").join("python").join("src");

        let sys = py.import_bound("sys")?;
        let path = sys.getattr("path")?;
        path.call_method("append", (src_dir,), None)?;

        // import main
        let main = py.import("main")?;
        let main = main.to_object(py);
        MAIN_MODULE.set(main).unwrap();

        Ok(())
    });

    res.unwrap_py();
}

pub fn main_module(py: Python<'_>) -> &PyModule {
    let main = MAIN_MODULE.get().expect("Main module not imported!");
    let main = main.downcast_bound::<PyModule>(py).unwrap();
    main.as_gil_ref()
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

fn exported_python_module(py: Python<'_>) -> PyResult<&PyModule> {
    let module = PyModule::new(py, "rust")?;

    module.add_class::<PyGridEnv>()?;
    module.add_class::<GridOpts>()?;
    module.add_class::<PyAction>()?;
    module.add_class::<Direction>()?;
    module.add_class::<PyCarType>()?;

    module.add_function(wrap_pyfunction!(grid_dimensions, module)?)?;

    Ok(module)
}

#[pyclass]
pub struct PyGridEnv {
    grid_ref: GridRef,
}

#[pymethods]
impl PyGridEnv {
    #[new]
    fn py_new(python_agents: Py<PyList>, opts: GridOpts, render: bool) -> Self {
        // let agent = PythonAgentWrapper::new(python_agent);
        let mut agents = vec![];
        Python::with_gil(|py| {
            let python_agents = python_agents.as_ref(py);
            for agent_obj in python_agents.iter() {
                let agent = PythonAgentWrapper::new(agent_obj.to_object(py));
                agents.push(agent);
            }
        });

        let grid = Grid::new(opts, agents);

        let mutex = Arc::new(Mutex::new(grid));
        let grid_ref = GridRef { mutex };

        if render {
            crate::render::render_main::start(grid_ref.clone());
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

            let action = obj.call_method1("get_action", (state,)).unwrap();
            let action: PyAction = action.extract().unwrap();

            action.assert_valid();
            action
        })
    }

    pub fn transition_happened(&self, state: Option<PyGridState>, new_state: PyGridState) {
        Python::with_gil(|py| {
            let obj = self.py_obj.bind(py);

            obj.call_method1("transition_happened", (state, new_state))
                .unwrap();
        });
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[pyclass]
pub struct PyAction {
    #[pyo3(get)]
    pick_up_passenger: Option<PyPassenger>,
    #[pyo3(get)]
    drop_off_passenger: Option<PyPassenger>,
    #[pyo3(get)]
    head_towards: Option<Direction>,
}

#[pymethods]
impl PyAction {
    #[staticmethod]
    fn pick_up_passenger(passenger: PyPassenger) -> Self {
        Self {
            pick_up_passenger: Some(passenger),
            drop_off_passenger: None,
            head_towards: None,
        }
    }

    #[staticmethod]
    fn drop_off_passenger(passenger: PyPassenger) -> Self {
        Self {
            pick_up_passenger: None,
            drop_off_passenger: Some(passenger),
            head_towards: None,
        }
    }

    #[staticmethod]
    fn head_towards(dir: Direction) -> Self {
        Self {
            pick_up_passenger: None,
            drop_off_passenger: None,
            head_towards: Some(dir),
        }
    }

    pub fn assert_valid(&self) {
        let mut somes = 0;
        self.pick_up_passenger.is_some().then(|| somes += 1);
        self.drop_off_passenger.is_some().then(|| somes += 1);
        self.head_towards.is_some().then(|| somes += 1);
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

    fn __eq__(&self, other: &Self) -> bool {
        self == other
    }
}

impl From<PyAction> for AgentAction {
    fn from(py_action: PyAction) -> Self {
        if let Some(passenger) = py_action.pick_up_passenger {
            Self::PickUp(passenger.id)
        } else if let Some(passenger) = py_action.drop_off_passenger {
            Self::DropOff(passenger.id)
        } else if let Some(head_towards) = py_action.head_towards {
            Self::HeadTowards(head_towards)
        } else {
            panic!("invalid PyAction (all None)")
        }
    }
}
