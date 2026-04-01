import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/linkedin")({
  beforeLoad: () => {
    throw redirect({
      href: "https://www.linkedin.com/company/charnotes",
    } as any);
  },
});
