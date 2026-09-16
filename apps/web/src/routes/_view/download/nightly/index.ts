import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/_view/download/nightly/")({
  beforeLoad: () => {
    throw redirect({ to: "/download/", statusCode: 301 });
  },
});
