import type { GetTypeByName } from "@content-collections/core";

import type configuration from "../../content-collections";

export type Article = GetTypeByName<typeof configuration, "articles">;
export declare const allArticles: Array<Article>;

export type Legal = GetTypeByName<typeof configuration, "legal">;
export declare const allLegals: Array<Legal>;

export type Doc = GetTypeByName<typeof configuration, "docs">;
export declare const allDocs: Array<Doc>;

export type Handbook = GetTypeByName<typeof configuration, "handbook">;
export declare const allHandbooks: Array<Handbook>;

export type Template = GetTypeByName<typeof configuration, "templates">;
export declare const allTemplates: Array<Template>;

export type Shortcut = GetTypeByName<typeof configuration, "shortcuts">;
export declare const allShortcuts: Array<Shortcut>;
