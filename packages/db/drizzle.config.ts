import { defineConfig } from "drizzle-kit";

export default defineConfig({
  dialect: "sqlite",
  dbCredentials: {
    url:
      process.env.DB_PATH ??
      `${process.env.HOME}/Library/Application Support/com.anthropic.char/app.db`,
  },
  out: "./src/generated",
});
