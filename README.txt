This project is a mix of both Rust and Python. It has been tested to work under Windows 10/11, Rust 1.77.1 and Python 3.11.9.

Most parameters are modifiable at the top of src/python/src/env.py. Some others are defined as constants in src/logic/grid.rs.

To instal python dependencies:
- install pytorch using instructions on https://pytorch.org/
- then, `pip install tianshou gymnasium numpy debugpy`

Rust dependencies are automatically installed when running it for the first time.

To start the simulation, run:
`cargo run --release`

