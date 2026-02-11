from fastmcp import FastMCP
import random

# Create an MCP server
mcp = FastMCP("Demo Server")

# Define tools
@mcp.tool()
def add(a: int, b: int) -> int:
    """Add two numbers"""
    return a + b

@mcp.tool()
def multiply(a: int, b: int) -> int:
    """Multiply two numbers"""
    return a * b

@mcp.tool()
def concat(strings: list[str]) -> str:
    """Concatenate a list of strings"""
    return "".join(strings)

@mcp.tool()
def get_random_number(min_val: int = 0, max_val: int = 100) -> int:
    """Get a random integer between min_val and max_val"""
    return random.randint(min_val, max_val)

@mcp.tool()
def echo(message: str) -> str:
    """Echo back the message"""
    return f"Echo: {message}"

# Define resources
@mcp.resource("text://hello")
def get_hello() -> str:
    """A simple greeting resource"""
    return "Hello from the FastMCP Demo Server!"

@mcp.resource("text://random_quote")
def get_quote() -> str:
    """Get a random quote"""
    quotes = [
        "The only way to do great work is to love what you do.",
        "Innovation distinguishes between a leader and a follower.",
        "Stay hungry, stay foolish.",
        "Simplicity is the ultimate sophistication."
    ]
    return random.choice(quotes)

# Define prompts
@mcp.prompt("greet")
def greet_prompt(name: str = "User") -> str:
    """A prompt to greet the user"""
    return f"Please generate a friendly greeting for {name}."

if __name__ == "__main__":
    mcp.run()
