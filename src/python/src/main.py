def hello_world():
    print("Hello World")

def hello_world_callback():
    return hello_world

def agent(input: list[int]):
    print(f"agent(): {input}")
    
    # return index of lowest number
    return input.index(min(input))
