use std::{cell::OnceCell, sync::OnceLock};

use pathfinding::grid::Grid;
use pyo3::{prelude::*, types::PyFunction};

use super::types::PyGrid;

pub fn initialise_python() {
    pyo3::prepare_freethreaded_python();

    let res = Python::with_gil(|py| -> PyResult<()> {
        // add src/python/src to sys.path
        let cwd = std::env::current_dir().unwrap();
        let src_dir = cwd.join("src").join("python").join("src");

        let sys = py.import("sys")?;
        let path = sys.getattr("path")?;
        path.call_method("append", (src_dir,), None)?;

        // import main
        let main = py.import("main")?;
        let main = main.to_object(py);
        MAIN_MODULE.set(main).unwrap();

        Ok(())
    });

    if let Err(e) = res {
        Python::with_gil(|py| {
            panic!("{}\n{}", e, e.traceback(py).unwrap().format().unwrap());
        })
    }
}

static MAIN_MODULE: OnceLock<Py<PyAny>> = OnceLock::new();

pub fn main_module(py: Python<'_>) -> &PyModule {
    let main = MAIN_MODULE.get().expect("Main module not imported!");
    let main: &PyModule = main.downcast(py).unwrap();
    main
}

pub fn start_python() {
    let res: PyResult<()> = {
        Python::with_gil(|py| {
            let main = main_module(py);

            let rust_module = rust_module(py)?;

            let start = main.getattr("start")?;
            start.call1((rust_module,))?;

            Ok(())
        })
    };

    if let Err(e) = res {
        Python::with_gil(|py| {
            panic!("{}\n{}", e, e.traceback(py).unwrap().format().unwrap());
        })
    }
}

static AGENT_CB: OnceLock<Py<PyAny>> = OnceLock::new();
static TRANSITION_CB: OnceLock<Py<PyAny>> = OnceLock::new();

#[pyfunction]
fn set_agent_callback(cb: Py<PyAny>) {
    AGENT_CB.set(cb).unwrap();
}

#[pyfunction]
fn set_transition_callback(cb: Py<PyAny>) {
    TRANSITION_CB.set(cb).unwrap();
}

pub fn agent_cb() -> &'static Py<PyAny> {
    AGENT_CB.get().unwrap()
}

pub fn transition_cb() -> &'static Py<PyAny> {
    TRANSITION_CB.get().unwrap()
}

fn rust_module(py: Python<'_>) -> PyResult<&PyModule> {
    let module = PyModule::new(py, "rust")?;

    module.add_class::<PyGrid>()?;
    module.add_function(wrap_pyfunction!(set_agent_callback, module)?)?;
    module.add_function(wrap_pyfunction!(set_transition_callback, module)?)?;

    Ok(module)
}
