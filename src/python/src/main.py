print("Importing main.py v2")

import time


def start(rust):
    GridEnv = rust.PyGridEnv
    GridOpts = rust.GridOpts
    Action = rust.PyAction
    Direction = rust.Direction

    opts = GridOpts(
        initial_passenger_count=10,
        passenger_spawn_rate=0.1,
        agent_car_count=2,
        npc_car_count=15,
    )

    class CarAgent:
        def get_action(self, state):
            pov_car = state.pov_car

            if len(pov_car.passengers) > 0:
                passenger = pov_car.passengers[0]
                return Action.drop_off_passenger(passenger, None)

            if len(state.idle_passengers) > 0:
                closest_passenger = state.idle_passengers[0]
                return Action.pick_up_passenger(closest_passenger, None)

            return Action.head_towards(Direction.Up)

        def transition_happened(self, state, action, new_state, reward):
            print(f"transition_happened: {state=}, {action=}, {new_state=}, {reward=}")

    agent = CarAgent()
    env = GridEnv(agent, opts, render=True)

    while True:
        env.tick()

        time.sleep(0.01)
