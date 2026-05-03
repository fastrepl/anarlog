import type { GetTypeByName } from "@content-collections/core";

import type configuration from "../../content-collections";

export type Article = GetTypeByName<typeof configuration, "articles">;
export declare const allArticles: Array<Article>;

export type Legal = GetTypeByName<typeof configuration, "legal">;
export declare const allLegals: Array<Legal>;
