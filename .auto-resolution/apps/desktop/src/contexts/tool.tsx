import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
} from "react";

import {
  createToolRegistry,
  type ToolRegistry,
  type ToolScope,
} from "./tool-registry/core";

export type { ToolRegistry, ToolScope };

const ToolRegistryContext = createContext<ToolRegistry | null>(null);

export function ToolRegistryProvider({
  registry: providedRegistry,
  children,
}: {
  registry?: ToolRegistry;
  children: React.ReactNode;
}) {
  const registry = useMemo(
    () => providedRegistry ?? createToolRegistry(),
    [providedRegistry],
  );

  return (
    <ToolRegistryContext.Provider value={registry}>
      {children}
    </ToolRegistryContext.Provider>
  );
}

export function useToolRegistry(): ToolRegistry {
  const registry = useContext(ToolRegistryContext);
  if (!registry) {
    throw new Error("useToolRegistry must be used within ToolRegistryProvider");
  }
  return registry;
}

export function useRegisterTools(
  scopes: ToolScope | ToolScope[],
  factory: () => Record<string, any>,
  deps: React.DependencyList,
): void {
  const registry = useToolRegistry();
  // oxlint-disable-next-line eslint-plugin-react-hooks(exhaustive-deps)
  const memoFactory = useCallback(factory, deps);

  useEffect(() => {
    const tools = memoFactory();
    const ids = Object.entries(tools).map(([key, tool]) =>
      registry.register(scopes, key, tool),
    );

    return () => {
      ids.forEach((id) => registry.unregister(id));
    };
  }, [memoFactory, registry, scopes]);
}
