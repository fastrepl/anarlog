import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/x")({
  beforeLoad: () => {
    throw redirect({
      href: "https://x.com/getcharnotes",
    } as any);
  },
});
