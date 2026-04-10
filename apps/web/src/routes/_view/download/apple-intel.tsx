import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/_view/download/apple-intel")({
  beforeLoad: async () => {
    throw redirect({
      href: "https://desktop2.char.com/download/latest/dmg-x86_64?channel=stable",
    } as any);
  },
});
