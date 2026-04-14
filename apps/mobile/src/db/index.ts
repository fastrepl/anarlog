import { createDb } from "@hypr/db";
import { createUseDrizzleLiveQuery, createUseLiveQuery } from "@hypr/db-react";

import { mobileLiveQueryClient } from "./mobileBridge";

export const db = createDb(mobileLiveQueryClient);
export const useLiveQuery = createUseLiveQuery(mobileLiveQueryClient);
export const useDrizzleLiveQuery = createUseDrizzleLiveQuery(
  mobileLiveQueryClient,
);
