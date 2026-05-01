import { createSpecialist } from "../factory";

export const posthogSpecialist = createSpecialist({
  name: "posthog",
  promptDir: import.meta.dirname,
});
