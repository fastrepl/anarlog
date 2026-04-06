import { createFileRoute, Outlet } from "@tanstack/react-router";

import { Main2Layout } from "~/main2/layout";
import { useMain2Lifecycle } from "~/main2/lifecycle";

export const Route = createFileRoute("/app/main2/_layout")({
  component: Component,
});

function Component() {
  useMain2Lifecycle();

  return (
    <Main2Layout>
      <Outlet />
    </Main2Layout>
  );
}
