import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/yc")({
  beforeLoad: () => {
    throw redirect({
      href: "https://www.ycombinator.com/companies/char",
    } as any);
  },
});
