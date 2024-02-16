use std::sync::OnceLock;

use pyo3::{prelude::*, FromPyPointer};

pub fn initialise_python() {
    Python::with_gil(|py| {
        // add src/python/src to sys.path
        let cwd = std::env::current_dir().unwrap();
        let src_dir = cwd.join("src").join("python").join("src");

        let sys = py.import("sys").unwrap();
        let path = sys.getattr("path").unwrap();
        path.call_method("append", (src_dir,), None).unwrap();

        // import main
        let main = py.import("main").unwrap();
        let main = main.to_object(py);
        MAIN_MODULE.set(main).unwrap();
    })
}

static MAIN_MODULE: OnceLock<Py<PyAny>> = OnceLock::new();

pub fn main_module(py: Python<'_>) -> &PyModule {
    let main = MAIN_MODULE.get().expect("Main module not imported!");
    let main: &PyModule = main.downcast(py).unwrap();
    main
}

pub fn get_agent_decision(passenger_distances: Vec<usize>) -> usize {
    Python::with_gil(|py| {
        let main = main_module(py);
        main.call_method("hello_world", (), None).unwrap();

        let res = main
            .call_method("agent", (passenger_distances,), None)
            .unwrap();
        let res: usize = res.extract().unwrap();

        res
    })
}
