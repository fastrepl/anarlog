import { defineConfig } from "@hey-api/openapi-ts";

export default defineConfig({
  input: "../../crates/api-client/openapi.gen.json",
  output: "src/generated",
});
