import type { Frameworks } from "@wdio/types";
import { type ChildProcess, spawn, spawnSync } from "node:child_process";
import { Socket } from "node:net";
import os from "node:os";
import path from "node:path";

import { TestRecorder } from "./record.js";

const videoRecorder = new TestRecorder();
let tauriDriver: ChildProcess;
let killedTauriDriver = false;
let testRunnerBackend: ChildProcess | undefined;
let killedTestRunnerBackend = false;

const defaultAppPath = path.resolve(
  import.meta.dirname,
  "../../apps/desktop/src-tauri/target/release/anarlog-dev",
);
const appPath = process.env.APP_BINARY_PATH
  ? path.resolve(process.env.APP_BINARY_PATH)
  : defaultAppPath;

console.log("App binary path:", appPath);

export const config = {
  hostname: "127.0.0.1",
  runner: "local",
  port: 4444,
  specs: ["./tests/**/*.spec.ts"],
  maxInstances: 1,
  capabilities: [
    {
      maxInstances: 1,
      "tauri:options": {
        application: appPath,
        args: ["--onboarding", "0"],
      },
    },
  ],
  reporters: ["spec"],
  framework: "mocha",
  mochaOpts: {
    ui: "bdd",
    timeout: process.env.E2E_VIRTUAL_AUDIO === "1" ? 180_000 : 60_000,
  },
  autoCompileOpts: {
    autoCompile: true,
    tsNodeOpts: {
      project: "./tsconfig.json",
      transpileOnly: true,
    },
  },

  waitforTimeout: 10000,
  connectionRetryTimeout: 120000,
  connectionRetryCount: 0,

  onPrepare: async () => {
    if (process.platform === "darwin") {
      if (!process.env.CN_API_KEY_WEBDRIVER) {
        console.error(
          "CN_API_KEY_WEBDRIVER is not set, required for CrabNebula Webdriver on macOS",
        );
        process.exit(1);
      }

      const { waitTestRunnerBackendReady } =
        await import("@crabnebula/test-runner-backend");
      testRunnerBackend = spawn("pnpm", ["exec", "test-runner-backend"], {
        stdio: "inherit",
        shell: true,
      });

      testRunnerBackend.on("error", (error) => {
        console.error("test-runner-backend error:", error);
        process.exit(1);
      });
      testRunnerBackend.on("exit", (code) => {
        if (!killedTestRunnerBackend) {
          console.error("test-runner-backend exited with code:", code);
          process.exit(1);
        }
      });

      await waitTestRunnerBackendReady();
      process.env.REMOTE_WEBDRIVER_URL = `http://127.0.0.1:3000`;
    }
  },

  beforeTest: async function (test: Frameworks.Test) {
    const videoPath = path.join(import.meta.dirname, "videos");
    videoRecorder.start(test, videoPath);
  },

  afterTest: async function () {
    await sleep(2000);
    videoRecorder.stop();
  },

  beforeSession: async () => {
    const env = { ...process.env, ONBOARDING: "0" };
    const useXvfb = process.platform === "linux" && !!process.env.CI;
    const tauriDriverCommand =
      process.platform === "linux"
        ? path.join(
            process.env.CARGO_HOME ?? path.join(os.homedir(), ".cargo"),
            "bin",
            "tauri-driver",
          )
        : "tauri-driver";
    killedTauriDriver = false;

    if (useXvfb) {
      console.log("Starting tauri-driver with xvfb-run (ONBOARDING=0)...");
      tauriDriver = spawn("xvfb-run", ["-a", tauriDriverCommand], {
        stdio: [null, process.stdout, process.stderr],
        env,
      });
    } else {
      console.log(
        `Starting tauri-driver on ${process.platform} (ONBOARDING=0)...`,
      );
      tauriDriver = spawn(tauriDriverCommand, [], {
        stdio: [null, process.stdout, process.stderr],
        env,
      });
    }

    tauriDriver.on("error", (error) => {
      console.error("tauri-driver error:", error);
      process.exit(1);
    });
    tauriDriver.on("exit", (code) => {
      if (!killedTauriDriver) {
        console.error("tauri-driver exited with code:", code);
        process.exit(1);
      }
    });

    await waitTauriDriverReady();
  },

  afterSession: async () => {
    await closeTauriDriver();
  },

  onComplete: () => {
    killedTestRunnerBackend = true;
    testRunnerBackend?.kill();
  },
};

async function closeTauriDriver() {
  killedTauriDriver = true;
  if (!tauriDriver || tauriDriver.exitCode !== null) {
    return;
  }
  await new Promise<void>((resolve) => {
    tauriDriver.once("exit", () => resolve());
    tauriDriver.kill();
  });
}

function killTauriDriver() {
  killedTauriDriver = true;
  tauriDriver?.kill();
}

function onShutdown(fn: () => void) {
  const cleanup = () => {
    try {
      fn();
    } finally {
      process.exit();
    }
  };

  process.on("exit", cleanup);
  process.on("SIGINT", cleanup);
  process.on("SIGTERM", cleanup);
  process.on("SIGHUP", cleanup);
  process.on("SIGBREAK", cleanup);
}

onShutdown(killTauriDriver);

async function sleep(ms: number) {
  return await new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitTauriDriverReady() {
  await new Promise<void>((resolve) => {
    const tryPort = () => {
      const socket = new Socket();
      socket.setTimeout(1000);
      socket.once("connect", () => {
        socket.destroy();
        resolve();
      });
      socket.once("timeout", () => {
        socket.destroy();
        setTimeout(tryPort, 200);
      });
      socket.once("error", () => {
        socket.destroy();
        setTimeout(tryPort, 200);
      });
      socket.connect(4444, "127.0.0.1");
    };
    tryPort();
  });
}
