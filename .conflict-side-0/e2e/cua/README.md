# CUA E2E Tests

End-to-end tests using [CUA](https://cua.ai) (Computer-Use Agent) SDK.

## Setup

Before running tests, you need to set up a sandbox environment. Follow the official guide:

https://cua.ai/docs/cua/guide/get-started/set-up-sandbox

## Usage (from repo root)

```bash
# infisical run --projectId=87dad7b5-72a6-4791-9228-b3b86b169db1 --path="/llm" --env=dev
task -t e2e/cua/Taskfile.yaml py:init
task -t e2e/cua/Taskfile.yaml py:run -- main.py
```

Or with poetry directly:

```bash
poetry -C e2e/cua install
poetry -C e2e/cua run python main.py
```
