import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/founders")({
  validateSearch: (search: Record<string, unknown>) => {
    return {
      source: (search.source as string) || undefined,
    };
  },
  beforeLoad: ({ search }) => {
    const baseUrl = "https://cal.com/team/char/welcome?duration=20";
    const url = search.source ? `${baseUrl}&source=${search.source}` : baseUrl;
    throw redirect({
      href: url,
    } as any);
  },
});
