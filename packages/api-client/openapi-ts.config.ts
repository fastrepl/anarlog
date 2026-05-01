import { defineConfig } from "@hey-api/openapi-ts";

export default defineConfig({
  input: "openapi.gen.json",
  output: "src/generated",
});
