import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/_view/download/nightly/linux-appimage")({
  beforeLoad: async () => {
    throw redirect({
      href: "https://desktop2.hyprnote.com/download/latest/appimage-x86_64?channel=nightly",
    } as any);
  },
});
