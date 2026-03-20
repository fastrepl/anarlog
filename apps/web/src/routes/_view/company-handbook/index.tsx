import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/_view/company-handbook/")({
  beforeLoad: () => {
    throw redirect({
      to: "/company-handbook/$/",
      params: { _splat: "about/what-char-is" },
    });
  },
});
