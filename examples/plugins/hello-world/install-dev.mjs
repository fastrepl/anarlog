import { cpSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const appId = process.env.CHAR_APP_ID || "com.hyprnote.dev";
const pluginRoot = resolve(__dirname);
const targetRoot =
  process.env.CHAR_PLUGIN_DIR ||
  (() => {
    switch (process.platform) {
      case "darwin":
        return join(
          homedir(),
          "Library",
          "Application Support",
          appId,
          "plugins",
        );
      case "linux":
        return join(homedir(), ".local", "share", appId, "plugins");
      case "win32":
        return join(homedir(), "AppData", "Roaming", appId, "plugins");
      default:
        throw new Error(`Unsupported platform: ${process.platform}`);
    }
  })();

const targetDir = join(targetRoot, "hello-world");

if (!existsSync(join(pluginRoot, "plugin.json"))) {
  throw new Error("plugin.json not found in hello-world plugin directory");
}

if (!existsSync(join(pluginRoot, "dist", "main.js"))) {
  throw new Error(
    "dist/main.js not found. Run `pnpm --dir examples/plugins/hello-world build` first.",
  );
}

mkdirSync(targetRoot, { recursive: true });
if (existsSync(targetDir)) {
  rmSync(targetDir, { recursive: true, force: true });
}

cpSync(pluginRoot, targetDir, {
  recursive: true,
  filter: (source) => !source.includes("node_modules"),
});

console.log(`Installed hello-world plugin to: ${targetDir}`);
