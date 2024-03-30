import random
from dataclasses import dataclass
import collections
from datetime import datetime
import math
from typing import Iterable, Deque
import time

print("Importing PyTorch...")
import torch

print("PyTorch imported!")
import numpy as np
from dqn.dqn_network import DqnNetwork, DqnNetworkResult

from environment.environment import DiscreteAction, DiscreteActionEnv, Environment, State, Transition
from network import NeuralNetwork
from data_helper import LivePlot
from environment.manhattan import ManhattanEnv
from replay_buffer import TransitionBatch, TransitionBuffer


@dataclass
class TdTargetBatch:
    # Tensor[TDTarget, TDTarget, ...]
    tensor: torch.Tensor


class DQN:
    def __init__(
        self,
        environment: ManhattanEnv,
        episode_count: int,
        timestep_count: int,
        gamma: float,
        epsilon_start: float = 0.9,
        epsilon_min: float = 0.01,
        epsilon_decay: float = 0.05,
        buffer_batch_size: int = 100,
        checkpoint_id: str | None = None,
        double_dqn: bool = True,
        prioritised_replay: bool = True,
        polyak: bool = True,
        random: bool = False,
    ):
        self.episode_count = episode_count
        self.timestep_count = timestep_count

        self.epsilon_start = epsilon_start
        self.epsilon_min = epsilon_min
        self.decay_rate = epsilon_decay
        self.epsilon = epsilon_start

        self.gamma = gamma
        self.buffer_batch_size = buffer_batch_size

        self.double_dqn = double_dqn
        self.prioritised_replay = prioritised_replay
        self.polyak = polyak
        self.random = random

        self.environment = environment
        environment.dqn = self

        self.transition_buffer = TransitionBuffer(omega=0.5, prioritised=prioritised_replay)
        self.policy_network = DqnNetwork(self.environment)  # q1 / θ
        self.target_network = self.policy_network.create_copy()  # q2 / θ-

        self.latest_checkpoint = None
        self.checkpoint_id = checkpoint_id
        if checkpoint_id is not None:
            self.policy_network.load_checkpoint(checkpoint_id, "policy_weights")
            self.target_network.load_checkpoint(checkpoint_id, "target_weights")

        self.total_timesteps = 0

        self.recent_rewards = collections.deque(maxlen=30)
        self.plot = LivePlot()

        self.start = datetime.now()

    def get_policy_action(self, state: State, stochastic: bool = False) -> DiscreteAction:
        if stochastic:
            action_preferences = self.policy_network.get_q_values(state).tensor
            action_preferences = (action_preferences + 1) / 2

            probabilities = action_preferences / torch.sum(action_preferences)
            probabilities = probabilities.detach().cpu().numpy()

            action = np.random.choice(np.arange(len(probabilities)), p=probabilities)
            return DiscreteAction(action)

        else:
            return self.policy_network.get_best_action(state)

    def get_action_using_epsilon_greedy(self, state: State):
        if np.random.uniform(0, 1) < self.epsilon or self.random:
            # pick random action
            return self.environment.random_action()
        else:
            # pick best action
            return self.get_policy_action(state)

    def execute_action(self, action: DiscreteAction) -> Transition:
        return self.environment.take_action(action)

    # using policy
    def get_q_values(self, state: State) -> DqnNetworkResult:
        return self.policy_network.get_q_values(state)

    # using target network here to estimate q values
    def get_q_value_for_action(self, state: State, action: DiscreteAction, policy_net=False) -> float:
        network = self.policy_network if policy_net else self.target_network
        neural_network_result = network.get_q_values(state)
        return neural_network_result.q_value_for_action(action)

    def compute_td_target(self, experience: Transition) -> float:
        # TD Target is the last reward + the expected reward of the
        # best action in the next state, discounted.
        # Note: this function does not use double dqn! (yet)

        # the reward and state after the last action was taken:
        last_reward = experience.reward  # R_t
        current_state = experience.new_state  # S_t+1

        if current_state.terminal:  # terminal experience
            td_target = last_reward
        else:
            target_net_result = self.target_network.get_q_values(current_state)
            best_q_value = target_net_result.best_action_q_value()
            td_target = last_reward + self.gamma * best_q_value

        return td_target

    def compute_td_targets_batch(self, experiences: TransitionBatch) -> TdTargetBatch:
        if self.double_dqn:
            # td_target = R_t+1 + γ * max_a' q_θ-(S_t+1, argmax_a' q_θ(S_t+1, a'))

            # the best action in S_t+1, according to the policy network
            best_actions = self.policy_network.get_q_values_batch(experiences.new_states).best_actions()
            best_actions = best_actions.unsqueeze(1)

            # the q-value of that action, according to the target network
            q_values = self.target_network.get_q_values_batch(experiences.new_states)
            q_values = q_values.for_actions(best_actions)
            q_values = q_values.squeeze(1)

        else:
            # td_target = R_t+1 + γ * max_a' q_θ-(S_t+1, a')
            q_values = self.target_network.get_q_values_batch(experiences.new_states)
            best_actions = q_values.best_actions().unsqueeze(1)
            q_values = q_values.for_actions(best_actions).squeeze(1)

        q_values[experiences.terminal] = 0
        q_values *= self.gamma

        # Tensor[-0.99, -0.99, ...]
        rewards = experiences.rewards

        # Tensor[TDTarget, TDTarget, ...]
        td_targets = rewards + q_values
        return TdTargetBatch(td_targets)

    def update_experiences_td_errors(self, experiences: TransitionBatch):
        td_targets = self.compute_td_targets_batch(experiences).tensor

        q_values = self.policy_network.get_q_values_batch(experiences.old_states)
        q_values = q_values.for_actions(experiences.actions).squeeze(1)

        c = 0.0001  # small constant (ϵ in Prioritized Replay Experience paper)
        td_errors = ((td_targets - q_values).abs() + c) ** self.transition_buffer.omega
        td_errors = td_errors.detach().cpu().numpy()
        experiences.update_td_errors(td_errors)

    def decay_epsilon(self, episode):
        # epsilon = epsilon_min + (epsilon_start - epsilon_min) x epsilon^-decay_rate * episode
        self.epsilon = self.epsilon_min + (self.epsilon_start - self.epsilon_min) * math.exp(
            -self.decay_rate * episode
        )

    def update_target_network(self):
        if self.polyak:
            target_net_state = self.target_network.state_dict()
            policy_net_state = self.policy_network.state_dict()
            tau = 0.005

            for key in policy_net_state:
                target_net_state[key] = tau * policy_net_state[key] + (1 - tau) * target_net_state[key]

            self.target_network.load_state_dict(target_net_state)

        else:
            C = 20
            if self.total_timesteps % C > 0:
                return

            self.target_network = self.policy_network.create_copy()

    def backprop(self, experiences: TransitionBatch, td_targets: TdTargetBatch) -> float:
        return self.policy_network.train(experiences, td_targets)

    def episode(self, episode_number: int):
        self.environment.reset()

        episode = DqnEpisode(self, episode_number)

        # reward_sum = 0
        # transition = None
        # timestep = 0

        # for timestep in range(self.timestep_count):
        # self.total_timesteps += 1

        # state = self.environment.current_state  # S_t
        # action = self.get_action_using_epsilon_greedy(state)  # A_t

        # transition = self.execute_action(action)
        # reward_sum += transition.reward

        # process termination
        # if transition.end_of_episode():
        # break

        episode.start()

        # episode ended
        # self.recent_rewards.append(reward_sum)

        # time_taken = time.time() - self.environment.last_reset

        # print episode result
        transition = episode.last_transition
        assert transition is not None

        won = self.environment.won(transition)
        won_str = "(won) " if won else "(lost)"
        running_avg = sum(self.recent_rewards) / len(self.recent_rewards)

        print(
            f"Episode {episode_number+1: <3} | {episode.timesteps+1: >3} timesteps {won_str}"
            f" | reward {episode.reward_sum: <6.2f} | avg {running_avg: <6.2f} {f'(last {len(self.recent_rewards)})': <9}"
            f" | ε {self.epsilon:.2f}"
            # f" | time_taken {time_taken:.2f}"
        )

        now = datetime.now()
        running_for = now - self.start

        suffix = None  # if should create checkpoint, will be a str
        if episode_number == self.episode_count - 1:  # create checkpoint after 10 episodes for startup
            suffix = " (startup checkpoint)"
        if episode_number % 100 == 0 and episode_number != 0:  # create checkpoint every 100 episodes
            suffix = f" (ep {episode_number})"

        should_save_checkpoint = suffix is not None
        if should_save_checkpoint:
            checkpoint_info = {
                "episode_number": episode_number,
                "reward": episode.reward_sum,
                "won": won,
                "epsilon": self.epsilon,
                "running_since": self.start,
                "running_for": running_for,
                "start_checkpoint": self.checkpoint_id,
                "previous_checkpoint": self.latest_checkpoint,
                "double_dqn": self.double_dqn,
                "prioritised_replay": self.prioritised_replay,
                "polyak": self.polyak,
                "random": self.random,
                "suffix": suffix,
            }
            checkpoint_folder, self.latest_checkpoint = self.policy_network.save_checkpoint(
                **checkpoint_info, filename="policy_weights"
            )
            self.target_network.save_checkpoint(**checkpoint_info, filename="target_weights")

            self.plot.save_img(checkpoint_folder / "plot.png")
            self.plot.save_csv(checkpoint_folder / "data.csv")

        self.decay_epsilon(episode_number)

        # if not won:
        time_taken = -1.0
        self.plot.add_episode(episode.reward_sum, won, running_avg, time_taken)
        # self.environment.save_replay()

    def train(self, new_transition: Transition):
        self.transition_buffer.add(new_transition)

        if self.transition_buffer.size() > self.buffer_batch_size:
            replay_batch = self.transition_buffer.get_batch(self.buffer_batch_size)
            td_targets = self.compute_td_targets_batch(replay_batch)

            loss = self.backprop(replay_batch, td_targets)
            self.plot.add_losses(loss, can_redraw=False)

            self.update_experiences_td_errors(replay_batch)

        self.update_target_network()

    def launch(self):
        self.start = datetime.now()

        try:
            for episode_number in range(self.episode_count):
                self.episode(episode_number)

        except KeyboardInterrupt:
            pass

        try:
            self.plot.draw()
        except KeyboardInterrupt:
            pass  # ctrl-c to close plot

        self.plot.close()


class DqnEpisode:

    def __init__(self, dqn: DQN, episode_number: int):
        self.dqn = dqn
        self.episode_number = episode_number

        dqn.environment.dqn_episode = self

        self.reward_sum: float = 0
        self.timesteps: int = 0

        self.last_transition: Transition | None = None
        # self.transitions: list[Transition] = []

    def start(self):
        TPS: int | None = 10

        last_tick = time.time()

        while not self.ended():
            self.dqn.environment.tick()

            if TPS is None:
                continue

            now = time.time()
            if now - last_tick < 1 / TPS:
                time.sleep(1 / TPS - (now - last_tick))

            last_tick = now

        self.dqn.recent_rewards.append(self.reward_sum)

    def process_transition(self, transition: Transition):
        # self.transitions.append(transition)
        self.last_transition = transition
        self.timesteps += 1
        self.reward_sum += transition.reward

        self.dqn.train(transition)

    def ended(self) -> bool:
        # if len(self.transitions) == 0:
        if self.last_transition is None:
            return False  # not started yet

        # return self.transitions[-1].end_of_episode()
        return self.last_transition.end_of_episode()
