print("Rust is importing main.py...")

import time

import debugpy
import random

from env import GridVecEnv, EnvOpts
from util import LogStep
from rl import dqn


def start(rust):
    # start_debug()

    with LogStep("Creating environment..."):
        env = GridVecEnv(rust)

    dqn(env)


def start_debug():
    debugpy.listen(("0.0.0.0", 5678), in_process_debug_adapter=True)
    print(f"Waiting for debugger on port 5678...")
    debugpy.debug_this_thread()
    debugpy.wait_for_client()
    print("Attached!")
    # debugpy.breakpoint()
