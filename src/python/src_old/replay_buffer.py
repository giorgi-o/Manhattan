import collections
from dataclasses import dataclass
from typing import Deque, Iterable
import typing
import numpy as np
from numpy import ndarray
import torch
from environment.environment import Transition
from network import NeuralNetwork


@dataclass
class Experience:
    transition: Transition
    td_error: float


class TransitionBatch:
    def __init__(self, experiences: list[Experience]):
        self.experiences = experiences
        self.size = len(experiences)

        # Tensor[[0], [2], [1], ...] for disctete
        # Tensor[[0.5, -0.7], [0.1, 0], ...] for continuous
        self.actions = NeuralNetwork.tensorify(
            np.array([exp.transition.action.numpy() for exp in experiences])
        )

        # Tensor[-0.99, -0.99, ...]
        self.rewards = NeuralNetwork.tensorify([exp.transition.reward for exp in experiences])

        # Tensor[State, State, ...]
        # states are already torch tensors, so we can just use torch.stack
        self.old_states = torch.stack([exp.transition.old_state.tensor for exp in experiences])
        self.new_states = torch.stack([exp.transition.new_state.tensor for exp in experiences])

        # Tensor[False, False, True, ...]
        self.terminal = NeuralNetwork.tensorify([exp.transition.new_state.terminal or exp.transition.truncated for exp in experiences])

    def update_td_errors(self, td_errors: Iterable[float]):
        for exp, td_error in zip(self.experiences, td_errors):
            exp.td_error = td_error


class TransitionBuffer:
    def __init__(self, max_len: int = 10000, omega: float = 0.5, prioritised: bool = True):
        self.buffer: Deque[Experience] = collections.deque(maxlen=max_len)
        self.new_transitions: list[Transition] = []
        self.omega = omega
        self.prioritised = prioritised

    def add(self, transition: Transition):
        if self.prioritised:
            self.new_transitions.append(transition)
        else:
            self.buffer.append(Experience(transition, 0.0))

    def get_priorities(self) -> ndarray | None:
        priorities = np.array([exp.td_error for exp in self.buffer])
        priorities /= priorities.sum()
        return priorities

    def get_batch(self, batch_size: int) -> TransitionBatch:
        if not self.prioritised:
            indices = np.random.choice(len(self.buffer), batch_size)
            return TransitionBatch([self.buffer[idx] for idx in indices])

        experiences = []

        # if there are new transitions, add them
        new_experiences = None
        if len(self.new_transitions) > 0:
            new_transitions = self.new_transitions[:batch_size]
            del self.new_transitions[:batch_size]

            new_experiences = [Experience(transition, -1.0) for transition in new_transitions]
            experiences.extend(new_experiences)
            batch_size -= len(new_experiences)

        if batch_size > 0:
            priorities = self.get_priorities()
            indices = np.random.choice(len(self.buffer), batch_size, p=priorities)
            experiences += [self.buffer[idx] for idx in indices]

        # now that we have picked from self.buffer, add the new experiences
        if new_experiences is not None:
            self.buffer.extend(new_experiences)

        return TransitionBatch(experiences)

    def size(self):
        return len(self.buffer) + len(self.new_transitions)
