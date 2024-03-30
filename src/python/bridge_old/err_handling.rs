use pyo3::{PyResult, Python};

pub trait UnwrapPyErr {
    fn unwrap_py(self);
}

impl<T> UnwrapPyErr for PyResult<T> {
    #[track_caller]
    fn unwrap_py(self) {
        if let Err(e) = self {
            Python::with_gil(|py| {
                if let Some(traceback) = e.traceback(py) {
                    panic!("{}\n{}", e, traceback.format().unwrap());
                } else {
                    panic!("{}", e);
                }
            })
        }
    }
}
