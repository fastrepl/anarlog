import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/_view/download/nightly/apple-silicon")({
  beforeLoad: async () => {
    throw redirect({
      href: "https://desktop2.hyprnote.com/download/latest/dmg-aarch64?channel=nightly",
    } as any);
  },
});
