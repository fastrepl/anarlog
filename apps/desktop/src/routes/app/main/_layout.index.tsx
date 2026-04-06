import { createFileRoute } from "@tanstack/react-router";

import { ClassicMainShellFrame } from "~/main/shell-frame";

export const Route = createFileRoute("/app/main/_layout/")({
  component: Component,
});

function Component() {
  return <ClassicMainShellFrame />;
}
