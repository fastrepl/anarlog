import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/github")({
  beforeLoad: () => {
    throw redirect({
      href: "https://github.com/fastrepl/char",
    } as any);
  },
});
