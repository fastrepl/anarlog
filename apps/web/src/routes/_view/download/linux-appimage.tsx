import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/_view/download/linux-appimage")({
  beforeLoad: async () => {
    throw redirect({
      href: "https://desktop2.char.com/download/latest/appimage-x86_64?channel=nightly",
    } as any);
  },
});
