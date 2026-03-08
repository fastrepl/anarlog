import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/_view/app/")({
  beforeLoad: async () => {
    throw redirect({
      to: "/app/file-transcription/",
      search: { id: undefined, tab: undefined },
    });
  },
});
