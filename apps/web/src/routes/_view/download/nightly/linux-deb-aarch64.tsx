import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute(
  "/_view/download/nightly/linux-deb-aarch64",
)({
  beforeLoad: async () => {
    throw redirect({
      href: "https://desktop2.hyprnote.com/download/latest/debian-aarch64?channel=nightly",
    } as any);
  },
});
