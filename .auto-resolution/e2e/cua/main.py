import asyncio

from agent import ComputerAgent
from computer import Computer


async def main():
    computer = Computer(
        os_type="macos",
        provider_type="lume",
        name="macos-sequoia-cua:latest",
    )

    await computer.run()

    try:
        agent = ComputerAgent(
            model="anthropic/claude-sonnet-4-5-20250929",
            tools=[computer],
        )

        messages = [{"role": "user", "content": "Go to github.com"}]

        async for result in agent.run(messages):
            for item in result["output"]:
                if item["type"] == "message":
                    print(item["content"][0]["text"])
    finally:
        await computer.disconnect()
        print("Disconnected from sandbox")


if __name__ == "__main__":
    asyncio.run(main())
