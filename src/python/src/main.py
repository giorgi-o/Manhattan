print("Rust is importing main.py...")

import time

import debugpy
from stable_baselines3 import A2C

from env import GridVecEnv, EnvOpts
from util import LogStep
from rl import dqn


def start(rust):
    start_debug()

    charging_stations_pos = [
        rust.CarPosition(
            direction=rust.Direction.Up,
            road_index=1,
            section_index=1,
            position_in_section=3,
        ),
        rust.CarPosition(
            direction=rust.Direction.Down,
            road_index=5,
            section_index=3,
            position_in_section=3,
        ),
    ]

    grid_opts = rust.GridOpts(
        initial_passenger_count=20,
        passenger_spawn_rate=0.0,
        max_passengers=30,
        agent_car_count=2,
        npc_car_count=15,
        passengers_per_car=4,
        charging_stations=charging_stations_pos,
        charging_station_capacity=1,
        passenger_radius=5,
        car_radius=2,
        verbose=True,
    )
    env_opts = EnvOpts(
        render=True,
    )

    with LogStep("Creating environment..."):
        env = GridVecEnv(rust, grid_opts, env_opts)

    dqn(env)


def start_debug():
    debugpy.listen(("0.0.0.0", 5678), in_process_debug_adapter=True)
    print(f"Waiting for debugger on port 5678...")
    debugpy.debug_this_thread()
    debugpy.wait_for_client()
    print("Attached!")
    # debugpy.breakpoint()
