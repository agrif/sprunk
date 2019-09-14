__all__ = [
    'Agent',
    'VolumeAgent',
    'VolumePeriodAgent',
]

class Agent:
    def __init__(self):
        self.scheduler = None

    def run(self):
        pass

    def reschedule(self, start):
        self.scheduler.add_agent(start, self)

class VolumeAgent(Agent):
    def __init__(self, volume, duration=0.005):
        self.volume = volume
        self.duration = duration
        super().__init__()

    def run(self):
        self.scheduler.set_volume(0, self.volume, duration=self.duration)

class VolumePeriodAgent(Agent):
    def __init__(self, volume, length, duration=0.005, in_duration=None, out_duration=None):
        if in_duration is None:
            in_duration = duration
        if out_duration is None:
            out_duration = duration
        self.in_duration = in_duration
        self.out_duration = out_duration
        self.volume = volume
        self.length = length
        super().__init__()

    def run(self):
        current = self.scheduler.get_volume(0)
        self.scheduler.set_volume(0, self.volume, duration=self.in_duration)
        a = VolumeAgent(current, duration=self.out_duration)
        self.scheduler.add_agent(self.in_duration + self.length, a)
