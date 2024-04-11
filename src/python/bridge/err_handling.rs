use pyo3::{types::PyTracebackMethods, PyResult, Python};

pub trait UnwrapPyErr<T> {
    fn unwrap_py(self) -> T;
}

impl<T> UnwrapPyErr<T> for PyResult<T> {
    #[track_caller]
    fn unwrap_py(self) -> T {
        match self {
            Ok(ok) => return ok,

            Err(e) => Python::with_gil(|py| {
                if let Some(traceback) = e.traceback_bound(py) {
                    panic!("{}\n{}", e, traceback.format().unwrap());
                } else {
                    panic!("{}", e);
                }
            }),
        }
    }
}
