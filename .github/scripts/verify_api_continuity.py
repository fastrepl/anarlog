#!/usr/bin/env python3
"""Exercise real AI requests through replacement, rollback, and repeated drain on Fly."""

import argparse
import asyncio
import json
import time
import wave
from pathlib import Path

import httpx
from websockets.asyncio.client import connect

import deploy_api_drain as deploy


def event(stage, **details):
    print(json.dumps({"time": time.time(), "stage": stage, **details}), flush=True)


async def authenticate(client, secrets_file, email):
    secrets = {
        entry["key"]: entry["value"]
        for entry in json.loads(Path(secrets_file).read_text())
    }
    base = secrets["SUPABASE_URL"].rstrip("/")
    key = secrets["SUPABASE_SERVICE_ROLE_KEY"]
    response = await client.post(
        base + "/auth/v1/admin/generate_link",
        headers={"apikey": key, "Authorization": "Bearer " + key},
        json={"type": "magiclink", "email": email},
    )
    if response.status_code != 200:
        raise RuntimeError(f"QA link generation HTTP {response.status_code}")
    link = response.json()
    token_hash = link.get("hashed_token") or link.get("properties", {}).get(
        "hashed_token"
    )
    response = await client.post(
        base + "/auth/v1/verify",
        headers={"apikey": secrets["SUPABASE_ANON_KEY"]},
        json={"type": "magiclink", "token_hash": token_hash},
    )
    if response.status_code != 200:
        raise RuntimeError(f"QA sign-in HTTP {response.status_code}")
    return response.json()["access_token"]


class Traffic:
    def __init__(self, base, token, audio):
        self.base = base
        self.headers = {"Authorization": "Bearer " + token}
        self.audio = audio
        self.streams = []
        self.tasks = []
        self.errors = []
        self.closing = False
        self.health = 0
        self.llms = 0

    async def record(self, machine_id):
        socket = await connect(
            self.base.replace("https:", "wss:")
            + "/listen?provider=deepgram&model=nova-3&encoding=linear16&sample_rate=16000&channels=1&language=en&interim_results=true",
            additional_headers={**self.headers, "fly-force-instance-id": machine_id},
            open_timeout=30,
            close_timeout=10,
        )
        state = {
            "id": machine_id,
            "transcripts": 0,
            "last_transcript": time.monotonic(),
            "sent_bytes": 0,
            "closed": False,
        }
        self.streams.append((socket, state))

        async def send():
            offset = 0
            try:
                while not self.closing:
                    chunk = self.audio[offset : offset + 3200]
                    await socket.send(chunk)
                    state["sent_bytes"] += len(chunk)
                    offset = (offset + len(chunk)) % len(self.audio)
                    await asyncio.sleep(0.1)
            except Exception as error:
                if not self.closing:
                    self.errors.append(f"{machine_id} sender: {type(error).__name__}")

        async def receive():
            try:
                async for raw in socket:
                    message = json.loads(raw)
                    if any(
                        item.get("transcript", "").strip()
                        for item in message.get("channel", {}).get("alternatives", [])
                    ):
                        state["transcripts"] += 1
                        state["last_transcript"] = time.monotonic()
                    if message.get("type") == "Error":
                        self.errors.append(f"{machine_id} provider error")
            except Exception as error:
                if not self.closing:
                    self.errors.append(f"{machine_id} receiver: {type(error).__name__}")
            finally:
                state["closed"] = True
                if not self.closing:
                    self.errors.append(f"{machine_id} closed before QA finished")

        self.tasks.extend([asyncio.create_task(send()), asyncio.create_task(receive())])
        deadline = time.monotonic() + 45
        while state["transcripts"] == 0:
            self.check()
            if time.monotonic() > deadline:
                raise RuntimeError("No initial transcript from " + machine_id)
            await asyncio.sleep(1)
        event("recording", machine=machine_id)

    def check(self):
        for _, state in self.streams:
            if state["closed"] or time.monotonic() - state["last_transcript"] > 60:
                raise RuntimeError("Recording continuity failed on " + state["id"])
        if self.errors:
            raise RuntimeError(self.errors[0])

    async def requests(self):
        async with httpx.AsyncClient(timeout=60) as client:

            async def health():
                while not self.closing:
                    try:
                        response = await client.get(
                            self.base + "/health/ready/ai", timeout=15
                        )
                        if response.status_code != 200:
                            self.errors.append(f"health HTTP {response.status_code}")
                        self.health += 1
                    except Exception as error:
                        self.errors.append("health: " + type(error).__name__)
                    await asyncio.sleep(2)

            async def llm():
                while not self.closing:
                    try:
                        async with client.stream(
                            "POST",
                            self.base + "/llm/chat/completions",
                            headers=self.headers,
                            json={
                                "messages": [
                                    {
                                        "role": "user",
                                        "content": "Count from 1 to 100, one number per line.",
                                    }
                                ],
                                "stream": True,
                                "max_tokens": 500,
                            },
                        ) as response:
                            if response.status_code != 200:
                                self.errors.append(f"LLM HTTP {response.status_code}")
                            done = False
                            async for line in response.aiter_lines():
                                done |= "[DONE]" in line
                            if not done:
                                self.errors.append("LLM missing final DONE")
                            self.llms += int(done)
                    except Exception as error:
                        self.errors.append("LLM: " + type(error).__name__)
                    await asyncio.sleep(2)

            await asyncio.gather(health(), llm())

    async def hold(self, seconds, stage):
        for elapsed in range(seconds):
            self.check()
            if elapsed % 30 == 0:
                event(
                    stage,
                    elapsed=elapsed,
                    health=self.health,
                    llms=self.llms,
                    streams=[
                        {
                            key: value
                            for key, value in state.items()
                            if key != "last_transcript"
                        }
                        for _, state in self.streams
                    ],
                )
            await asyncio.sleep(1)
        self.check()

    async def close(self):
        self.closing = True
        for socket, _ in self.streams:
            await socket.close()
        for task in self.tasks:
            task.cancel()
        await asyncio.gather(*self.tasks, return_exceptions=True)


async def run(args):
    if args.app != "anarlog-inference":
        raise RuntimeError(
            "Continuity QA currently supports only the isolated AI runtime"
        )
    # The immutable candidate is built separately so recording time excludes remote builds.
    machines = deploy.serving_machines(
        await asyncio.to_thread(deploy.list_machines, args.app)
    )
    if len(machines) < 2 or any(
        not deploy.supports_session_drain(machine) for machine in machines
    ):
        raise RuntimeError("Need two serving machines with a verified drain protocol")
    digests = {machine["image_ref"]["digest"] for machine in machines}
    if len(digests) != 1:
        raise RuntimeError("Serving set must have one known rollback image")
    repository = machines[0]["config"]["image"].split("@")[0].split(":")[0]
    rollback = repository + "@" + digests.pop()
    if rollback == args.image:
        raise RuntimeError(
            "Use a different immutable image to exercise a real rollback"
        )
    with wave.open("crates/data/src/english_1/audio.wav") as source:
        if (source.getnchannels(), source.getsampwidth(), source.getframerate()) != (
            1,
            2,
            16000,
        ):
            raise RuntimeError("Unexpected fixture audio format")
        audio = source.readframes(source.getnframes())
    async with httpx.AsyncClient(timeout=30) as client:
        token = await authenticate(client, args.secrets, args.email)
    traffic = Traffic("https://" + args.app + ".fly.dev", token, audio)
    monitor = asyncio.create_task(traffic.requests())
    result = {
        "passed": False,
        "candidate": args.image,
        "rollback": rollback,
        "stages": [],
    }
    try:
        for machine in machines:
            await traffic.record(machine["id"])
        for stage, image, hold in [
            ("replacement", args.image, 330),
            ("rollback", rollback, 65),
            ("rollforward", args.image, 65),
        ]:
            traffic.check()
            event(stage + "_start", image=image)
            await asyncio.to_thread(
                deploy.deploy,
                args.app,
                args.config,
                args.dockerfile,
                args.version,
                image,
            )
            await traffic.hold(hold, stage)
            result["stages"].append(stage)
            serving = deploy.serving_machines(
                await asyncio.to_thread(deploy.list_machines, args.app)
            )
            if stage != "rollforward":
                for machine in serving:
                    await traffic.record(machine["id"])
        if traffic.llms == 0 or traffic.health == 0:
            raise RuntimeError("Missing HTTP coverage")
        await traffic.close()
        await monitor
        if traffic.errors:
            raise RuntimeError(traffic.errors[0])
        # Closing QA sockets must release every old drain permit without forced termination.
        deadline = time.monotonic() + 90
        ids = {state["id"] for _, state in traffic.streams}
        while time.monotonic() < deadline:
            current = await asyncio.to_thread(deploy.list_machines, args.app)
            if all(
                not deploy.is_started(machine)
                for machine in current
                if machine["id"] in ids
            ):
                event("old_qa_machines_stopped")
                break
            await asyncio.sleep(5)
        else:
            raise RuntimeError(
                "Old machines have not stopped; inspect non-QA sessions before retirement"
            )
        result["passed"] = True
    finally:
        await traffic.close()
        await monitor
        result.update(
            health=traffic.health,
            llms=traffic.llms,
            errors=traffic.errors,
            streams=[state for _, state in traffic.streams],
        )
        Path(args.output).write_text(json.dumps(result, indent=2))
        event(
            "finished",
            passed=result["passed"],
            health=traffic.health,
            llms=traffic.llms,
            errors=traffic.errors,
        )


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    for name in [
        "app",
        "config",
        "dockerfile",
        "version",
        "image",
        "secrets",
        "email",
        "output",
    ]:
        parser.add_argument("--" + name, required=True)
    asyncio.run(run(parser.parse_args()))
