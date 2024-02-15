use pyo3::prelude::*;

pub fn initialise_python() {
    Python::with_gil(|py| {
        // add src/python/src to sys.path
        let cwd = std::env::current_dir().unwrap();
        let src_dir = cwd.join("src").join("python").join("src");

        let sys = py.import("sys").unwrap();
        let path = sys.getattr("path").unwrap();
        path.call_method("append", (src_dir,), None).unwrap();
    })
}

pub fn import_main<'py>(py: &'py Python<'_>) -> &'py PyModule {
    py.import("main").unwrap()
}

pub fn get_agent_decision(passenger_distances: Vec<usize>) -> usize {
    Python::with_gil(|py| {
        let main = import_main(&py);
        // main.call_method("hello_world", (), None).unwrap();

        let res = main
            .call_method("agent", (passenger_distances,), None)
            .unwrap();
        let res: usize = res.extract().unwrap();

        res
    })
}
