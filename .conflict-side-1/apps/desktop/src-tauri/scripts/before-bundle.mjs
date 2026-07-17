import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));

function runScript(scriptName) {
  return new Promise((resolve, reject) => {
    const child = spawn("bash", [path.join(scriptDir, scriptName)], {
      stdio: "inherit",
    });

    child.on("error", reject);
    child.on("close", (code) => {
      if (code === 0) {
        resolve();
        return;
      }

      reject(new Error(`${scriptName} exited with code ${code ?? "unknown"}`));
    });
  });
}

if (process.platform === "win32") {
  console.log("[before-bundle] Windows detected, skipping shell bundle hooks.");
  process.exit(0);
}

if (process.platform === "darwin") {
  await runScript("compile-icons.sh");
}

await runScript("fix-dylib.sh");
