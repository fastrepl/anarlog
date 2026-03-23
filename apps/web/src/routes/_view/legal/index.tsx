import { createFileRoute } from "@tanstack/react-router";

import { LegalIndexLayout } from "./-components";

export const Route = createFileRoute("/_view/legal/")({
  component: Component,
  head: () => ({
    meta: [
      { title: "Legal - Char" },
      {
        name: "description",
        content: "Terms, privacy policy, and other legal documents for Char",
      },
      { property: "og:title", content: "Legal - Char" },
      {
        property: "og:description",
        content: "Terms, privacy policy, and other legal documents for Char",
      },
      { property: "og:type", content: "website" },
      { property: "og:url", content: "https://char.com/legal" },
      { name: "twitter:card", content: "summary" },
      { name: "twitter:title", content: "Legal - Char" },
      {
        name: "twitter:description",
        content: "Terms, privacy policy, and other legal documents for Char",
      },
    ],
  }),
});

function Component() {
  return <LegalIndexLayout />;
}
