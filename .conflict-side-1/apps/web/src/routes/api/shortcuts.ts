import { createFileRoute } from "@tanstack/react-router";
import { json } from "@tanstack/react-start";
import { allShortcuts } from "content-collections";

import { corsMiddleware } from "../../middleware/cors";

export const Route = createFileRoute("/api/shortcuts")({
  server: {
    middleware: [corsMiddleware],
    handlers: {
      GET: async () => {
        const shortcuts = allShortcuts.map((shortcut) => ({
          slug: shortcut.slug,
          title: shortcut.title,
          description: shortcut.description,
          category: shortcut.category,
          prompt: shortcut.prompt,
          targets: shortcut.targets,
        }));

        return json(shortcuts);
      },
    },
  },
});
