import { createFileRoute, Outlet } from "@tanstack/react-router";

import { ClassicMainLayout } from "~/main/layout";
import { useClassicMainLifecycle } from "~/main/lifecycle";

export const Route = createFileRoute("/app/main/_layout")({
  component: Component,
});

function Component() {
  useClassicMainLifecycle();

  return (
    <ClassicMainLayout>
      <Outlet />
    </ClassicMainLayout>
  );
}
