import {
  createBuilder,
  suppressDeprecatedWarnings,
} from "@content-collections/core";
import { dirname, join } from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const configPath = join(__dirname, "../content-collections.ts");

// Keep typecheck output focused on actual failures until the deprecated
// content-collections config shape is migrated separately.
suppressDeprecatedWarnings(
  "collectionsConfigProperty",
  "implicitContentProperty",
);

async function main() {
  const builder = await createBuilder(configPath);
  await builder.build();
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
