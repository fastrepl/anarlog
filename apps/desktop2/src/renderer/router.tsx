import {
  createRootRoute,
  createRoute,
  createRouter,
} from "@tanstack/react-router";

import { AppRootContainer } from "~/app";
import { IndexRoute } from "~/routes/index";

// The router only owns route declarations. The real composition root —
// lifecycle + providers + shell wiring — lives in `AppRootContainer`, which
// mirrors the `_layout` route in `apps/desktop/src/routes/app/main2/`.
//
// The index route is declared for URL parity with the Tauri app; actual body
// content is still chosen by the tabs store inside `AppRootContainer` via
// `TabContentView`, not by `<Outlet />`. When desktop2 switches to
// router-driven navigation, hoist `TabContentView` into the route tree and
// have `AppRootContainer` render `<Outlet />` instead.
const rootRoute = createRootRoute({
  component: AppRootContainer,
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: IndexRoute,
});

const routeTree = rootRoute.addChildren([indexRoute]);

export const router = createRouter({
  routeTree,
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
