import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/contact")({
  beforeLoad: () => {
    throw redirect({
      href: "mailto:support@char.com",
    } as any);
  },
});
