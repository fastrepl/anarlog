import { createFileRoute } from "@tanstack/react-router";

import { Main2Shell } from "~/main2/shell";

export const Route = createFileRoute("/app/main2/_layout/")({
  component: Main2Layout,
});

export function Main2Layout() {
  return <Main2Shell />;
}
