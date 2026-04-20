import { createSelectSchema } from "drizzle-zod";

import { dailyNotes, sessions, tasks } from "./schema";

export const sessionRowSchema = createSelectSchema(sessions);
export const dailyNoteRowSchema = createSelectSchema(dailyNotes);
export const taskRowSchema = createSelectSchema(tasks);
