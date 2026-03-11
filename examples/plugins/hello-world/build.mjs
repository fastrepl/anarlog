import * as esbuild from "esbuild";

await esbuild.build({
  bundle: true,
  entryPoints: ["src/main.tsx"],
  format: "iife",
  outfile: "dist/main.js",
  platform: "browser",
  jsx: "transform",
  jsxFactory: "React.createElement",
  jsxFragment: "React.Fragment",
  minify: false,
  sourcemap: true,
});
