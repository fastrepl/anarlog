import { createSpecialist } from "../factory";

export const stripeSpecialist = createSpecialist({
  name: "stripe",
  promptDir: import.meta.dirname,
});
