import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/bluesky")({
  beforeLoad: () => {
    throw redirect({
      href: "https://bsky.app/profile/char.bsky.social",
    } as any);
  },
});
