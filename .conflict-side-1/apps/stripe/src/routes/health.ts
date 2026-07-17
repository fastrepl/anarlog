import { Hono } from "hono";

import type { AppBindings } from "../hono-bindings";

export const health = new Hono<AppBindings>();

health.get("/", (c) => {
  return c.json({ status: "ok" }, 200);
});
