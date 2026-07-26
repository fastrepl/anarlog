export type ToolScope = "chat-general" | "enhancing";

interface ToolEntry<TTool> {
  id: symbol;
  scopes: ToolScope[];
  key: string;
  tool: TTool;
}

export interface ToolRegistry<TTool = any> {
  register(scopes: ToolScope | ToolScope[], key: string, tool: TTool): symbol;
  unregister(id: symbol): void;
  getTools(scope?: ToolScope): Record<string, TTool>;
}

export function createToolRegistry<TTool = any>(): ToolRegistry<TTool> {
  const entries = new Map<symbol, ToolEntry<TTool>>();

  return {
    register(scopes, key, tool) {
      const scopeArray = Array.isArray(scopes) ? scopes : [scopes];
      const id = Symbol(`${scopeArray.join(",")}:${key}`);
      const entry: ToolEntry<TTool> = {
        id,
        scopes: scopeArray,
        key,
        tool,
      };
      entries.set(id, entry);
      return id;
    },

    unregister(id) {
      const entry = entries.get(id);
      if (!entry) {
        return;
      }

      entries.delete(id);
    },

    getTools(scope) {
      return Array.from(entries.values())
        .filter((entry) => (scope ? entry.scopes.includes(scope) : true))
        .reduce<Record<string, TTool>>((acc, entry) => {
          acc[entry.key] = entry.tool;
          return acc;
        }, {});
    },
  };
}
