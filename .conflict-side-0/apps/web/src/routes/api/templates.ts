import { createFileRoute } from "@tanstack/react-router";
import { json } from "@tanstack/react-start";
import { allTemplates } from "content-collections";

import { corsMiddleware } from "../../middleware/cors";

export const Route = createFileRoute("/api/templates")({
  server: {
    middleware: [corsMiddleware],
    handlers: {
      GET: async () => {
        const templates = allTemplates.map((template) => ({
          slug: template.slug,
          title: template.title,
          description: template.description,
          category: template.category,
          targets: template.targets,
          sections: template.sections,
        }));

        return json(templates);
      },
    },
  },
});
