import * as Sentry from "@sentry/bun";
import { Hono } from "hono";
import { bodyLimit } from "hono/body-limit";
import { cors } from "hono/cors";
import { logger } from "hono/logger";

import { env } from "./env";
import type { AppBindings } from "./hono-bindings";
import { verifyStripeWebhook } from "./middleware";
import { routes } from "./routes";

Sentry.init({
  dsn: Bun.env.SENTRY_DSN,
  environment: env.NODE_ENV,
  enabled: env.NODE_ENV === "production",
});

const app = new Hono<AppBindings>();

app.use(logger());
app.use(bodyLimit({ maxSize: 1024 * 1024 * 5 }));

const corsMiddleware = cors({
  origin: "*",
  allowHeaders: ["content-type", "stripe-signature"],
  allowMethods: ["GET", "POST", "OPTIONS"],
});

app.use("*", corsMiddleware);

app.use("/webhook/stripe", verifyStripeWebhook);

app.route("/", routes);

app.onError((err, c) => {
  Sentry.captureException(err, {
    extra: { path: c.req.path, method: c.req.method },
  });
  return c.json({ error: "internal_server_error" }, 500);
});

app.notFound((c) => c.text("not_found", 404));

export default {
  port: env.PORT,
  fetch: app.fetch,
};
