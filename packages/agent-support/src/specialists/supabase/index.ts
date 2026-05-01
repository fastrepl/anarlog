import { createSpecialist } from "../factory";
import { fetchDatabaseSchema } from "./schema";

export const supabaseSpecialist = createSpecialist({
  name: "supabase",
  promptDir: import.meta.dirname,
  getContext: async () => ({ schema: await fetchDatabaseSchema() }),
});
