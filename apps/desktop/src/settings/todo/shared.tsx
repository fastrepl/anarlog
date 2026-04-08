import type { ReactNode } from "react";

export type TodoProvider = {
  id: string;
  displayName: string;
  icon: ReactNode;
  nangoIntegrationId: string;
  filterLabel: string;
  filterPlaceholder: string;
};

export const TODO_PROVIDERS: TodoProvider[] = [
  {
    id: "linear",
    displayName: "Linear",
    icon: <img src="/assets/linear-icon.svg" alt="Linear" className="size-5" />,
    nangoIntegrationId: "linear",
    filterLabel: "Team / Project",
    filterPlaceholder: "e.g. Team name or project",
  },
  {
    id: "github",
    displayName: "GitHub",
    icon: <img src="/assets/github-icon.svg" alt="GitHub" className="size-5" />,
    nangoIntegrationId: "github",
    filterLabel: "Repository",
    filterPlaceholder: "e.g. owner/repo",
  },
];
