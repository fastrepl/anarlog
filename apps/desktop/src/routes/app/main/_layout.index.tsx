import { createFileRoute } from "@tanstack/react-router";

import { MainShellFrame } from "~/shared/main";

export const Route = createFileRoute("/app/main/_layout/")({
  component: Component,
});

function Component() {
  return <MainShellFrame />;
}
