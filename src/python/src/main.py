print("Importing main.py v2")

import time

import debugpy
from stable_baselines3 import A2C

from env import GridVecEnv, EnvOpts
from rl import dqn


def start(rust):
    # start_debug()

    grid_opts = rust.GridOpts(
        initial_passenger_count=2,
        passenger_spawn_rate=0.01,
        agent_car_count=2,
        npc_car_count=15,
        passengers_per_car=4,
        verbose=False,
    )
    env_opts = EnvOpts(
        passenger_radius=10,
        car_radius=10,
        render=True,
    )

    env = GridVecEnv(rust, grid_opts, env_opts)
    dqn(env)


def start_debug():
    debugpy.listen(("0.0.0.0", 5678), in_process_debug_adapter=True)
    print(f"Waiting for debugger on port 5678...")
    debugpy.debug_this_thread()
    debugpy.wait_for_client()
    print("Attached!")
    # debugpy.breakpoint()
