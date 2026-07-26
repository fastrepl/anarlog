import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/discord")({
  beforeLoad: () => {
    throw redirect({
      href: "https://discord.gg/Vk882WS3gF",
    } as any);
  },
});
