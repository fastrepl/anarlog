import yaml from "js-yaml";

import { createMdxFormatter } from "./mdx-format-core.js";

const { format } = createMdxFormatter(yaml);

let input = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", (chunk) => (input += chunk));
process.stdin.on("end", () => {
  process.stdout.write(format(input));
});
