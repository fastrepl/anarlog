import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute(
  "/_view/download/nightly/linux-appimage-aarch64",
)({
  beforeLoad: async () => {
    throw redirect({
      href: "https://desktop2.hyprnote.com/download/latest/appimage-aarch64?channel=nightly",
    } as any);
  },
});
