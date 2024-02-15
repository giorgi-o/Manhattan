use pyo3::prelude::*;

#[pyclass]
struct AgentInputData {
    pub car_distances: Vec<usize>,
}
