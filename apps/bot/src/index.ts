import { ApplicationFunctionOptions, Probot } from "probot";

import { registerPrClosedHandler } from "./features/index.js";

export default (app: Probot, { getRouter }: ApplicationFunctionOptions) => {
  if (getRouter) {
    const router = getRouter("/");

    router.get("/health", (_req, res) => {
      res.send("OK");
    });
  }

  registerPrClosedHandler(app);
};
