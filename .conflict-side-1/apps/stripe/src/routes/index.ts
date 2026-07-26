import { Hono } from "hono";

import type { AppBindings } from "../hono-bindings";
import { health } from "./health";
import { webhook } from "./webhook";

export const routes = new Hono<AppBindings>();

routes.route("/health", health);
routes.route("/webhook", webhook);
