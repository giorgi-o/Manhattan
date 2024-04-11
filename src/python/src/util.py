class LogStep:
    def __init__(self, start: str, end: str = "done"):
        self.start = start
        self.end = end

    def __enter__(self):
        print(self.start + " ", end=" ", flush=True)

    def __exit__(self, exc_type, exc_val, exc_tb):
        print(self.end)