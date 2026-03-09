import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/youtube")({
  beforeLoad: () => {
    throw redirect({
      href: "https://www.youtube.com/@charnotes",
    } as any);
  },
});
