import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = fileURLToPath(new URL(".", import.meta.url));
const shimPath = resolve(__dirname, "shim.js");

/** @returns {import('vite').Plugin} */
export function relayShim() {
  return {
    name: "relay-shim",
    configureServer(server) {
      server.middlewares.use("/relay-shim.js", (_req, res) => {
        const content = readFileSync(shimPath, "utf-8");
        res.setHeader("Content-Type", "application/javascript");
        res.end(content);
      });
    },
    transformIndexHtml(html, ctx) {
      if (!ctx.server) {
        return html.replace(
          /<script\s+src="\/relay-shim\.js"><\/script>\s*/,
          "",
        );
      }
    },
  };
}
