import { createFileRoute, Outlet } from "@tanstack/react-router";

export const Route = createFileRoute("/app/main2/_layout")({
  component: Component,
});

function Component() {
  return <Outlet />;
}
