// TypeScript port of `apps/desktop/src-tauri/src/embedded_cli.rs`.
//
// Same state machine, same install path (`/usr/local/bin/<command>`), same
// channel → command-name mapping, same symlink strategy, same AppleScript
// elevation fallback. Keep the two in sync: changing the command name or
// install path here should be mirrored there until the Tauri app is retired.

import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";

import type {
  EmbeddedCliState,
  EmbeddedCliStatus,
} from "../shared/embedded-cli.js";
import { APP_ID } from "./channel.js";
import { embeddedCliPath } from "./paths.js";

const execFileAsync = promisify(execFile);

const INSTALL_DIR = "/usr/local/bin";

function commandName(): string {
  // Keep the Tauri mapping (`DEV_BUNDLE_ID` → `char-dev`, etc.). The `com.char.*`
  // bundle ids are the Electron-era names; `char-dev` is the unpackaged dev run.
  switch (APP_ID) {
    case "com.char.stable":
      return "char";
    case "com.char.nightly":
      return "char-nightly";
    case "com.char.staging":
      return "char-staging";
    default:
      return "char-dev";
  }
}

function installPathFor(cmd: string): string {
  return path.join(INSTALL_DIR, cmd);
}

async function exists(p: string): Promise<boolean> {
  try {
    await fs.lstat(p);
    return true;
  } catch {
    return false;
  }
}

async function classify(
  installPath: string,
  resourcePath: string,
): Promise<{ state: EmbeddedCliState; details: string | null }> {
  let stat;
  try {
    stat = await fs.lstat(installPath);
  } catch (err) {
    const e = err as NodeJS.ErrnoException;
    if (e.code === "ENOENT") {
      return {
        state: "missing",
        details: `Command is not installed at ${installPath}.`,
      };
    }
    return {
      state: "conflict",
      details: `failed to inspect ${installPath}: ${e.message}`,
    };
  }

  if (!stat.isSymbolicLink()) {
    return {
      state: "conflict",
      details: `A different command already exists at ${installPath}.`,
    };
  }

  try {
    const installedTarget = await fs.realpath(installPath);
    const resourceTarget = await fs.realpath(resourcePath);
    if (installedTarget === resourceTarget) {
      return {
        state: "installed",
        details: "Command is installed and managed by this app.",
      };
    }
    return {
      state: "conflict",
      details: `A different command already exists at ${installPath}.`,
    };
  } catch (err) {
    const e = err as Error;
    return { state: "conflict", details: e.message };
  }
}

async function resolveResourcePath(): Promise<string | null> {
  const p = embeddedCliPath();
  if (!p) return null;
  return (await exists(p)) ? p : null;
}

export async function check(): Promise<EmbeddedCliStatus> {
  const cmd = commandName();
  const installPath = installPathFor(cmd);

  if (process.platform !== "darwin") {
    return {
      supported: false,
      commandName: cmd,
      installPath,
      resourcePath: null,
      state: "unsupported",
      details: "Embedded CLI install is only supported on macOS.",
    };
  }

  const resource = await resolveResourcePath();
  if (!resource) {
    return {
      supported: true,
      commandName: cmd,
      installPath,
      resourcePath: null,
      state: "resource_missing",
      details: "Embedded CLI resource is not available in this build.",
    };
  }

  const { state, details } = await classify(installPath, resource);
  return {
    supported: true,
    commandName: cmd,
    installPath,
    resourcePath: resource,
    state,
    details,
  };
}

export async function install(): Promise<EmbeddedCliStatus> {
  const status = await check();
  if (status.state === "unsupported" || status.state === "resource_missing") {
    return status;
  }
  if (!status.resourcePath) return status;

  await installSymlink(status.resourcePath, status.installPath);

  return check();
}

export async function uninstall(): Promise<EmbeddedCliStatus> {
  const status = await check();
  if (status.state !== "installed" && status.state !== "conflict") {
    return status;
  }
  // Only remove symlinks we own. A non-symlink `conflict` should keep hands off.
  if (status.state === "conflict") {
    return status;
  }

  await removeInstalled(status.installPath);
  return check();
}

async function installSymlink(
  resource: string,
  installPath: string,
): Promise<void> {
  try {
    await installSymlinkDirect(resource, installPath);
    return;
  } catch (err) {
    // Fall back to AppleScript elevation. Matches
    // `embedded_cli.rs::install_symlink` — unwritable `/usr/local/bin` is the
    // common case on macOS without Homebrew.
    const e = err as NodeJS.ErrnoException;
    if (e.code !== "EACCES" && e.code !== "EPERM" && e.code !== "EROFS") {
      // Unexpected failure — bubble up so the UI surfaces it.
      throw err;
    }
  }
  await runPrivileged(buildInstallScript(resource, installPath));
}

async function installSymlinkDirect(
  resource: string,
  installPath: string,
): Promise<void> {
  const parent = path.dirname(installPath) || INSTALL_DIR;
  await fs.mkdir(parent, { recursive: true });
  await fs.rm(installPath, { force: true });
  await fs.symlink(resource, installPath);
}

async function removeInstalled(installPath: string): Promise<void> {
  try {
    await fs.rm(installPath, { force: true });
    return;
  } catch (err) {
    const e = err as NodeJS.ErrnoException;
    if (e.code !== "EACCES" && e.code !== "EPERM" && e.code !== "EROFS") {
      throw err;
    }
  }
  await runPrivileged(buildUninstallScript(installPath));
}

function shellQuote(p: string): string {
  // Same single-quote escaping as `embedded_cli.rs::shell_quote`.
  return `'${p.replaceAll("'", `'"'"'`)}'`;
}

function buildInstallScript(resource: string, installPath: string): string {
  const installDir = path.dirname(installPath) || INSTALL_DIR;
  return [
    "set -e",
    `mkdir -p ${shellQuote(installDir)}`,
    `rm -rf ${shellQuote(installPath)}`,
    `ln -s ${shellQuote(resource)} ${shellQuote(installPath)}`,
  ].join("; ");
}

function buildUninstallScript(installPath: string): string {
  return `set -e; rm -rf ${shellQuote(installPath)}`;
}

async function runPrivileged(script: string): Promise<void> {
  // `do shell script … with administrator privileges` triggers the system
  // auth prompt, same as the Rust side's `osascript` invocation.
  const escaped = script.replaceAll("\\", "\\\\").replaceAll(`"`, `\\"`);
  const osa = `do shell script "${escaped}" with administrator privileges`;
  try {
    await execFileAsync("/usr/bin/osascript", ["-e", osa]);
  } catch (err) {
    const e = err as Error;
    throw new Error(`administrator authorization failed: ${e.message}`);
  }
}
