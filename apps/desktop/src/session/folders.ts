export function normalizeFolderPath(path: string): string | null {
  const replaced = path.replace(/\\/g, "/").trim();
  if (replaced.startsWith("/")) {
    return null;
  }

  const trimmed = replaced.replace(/^\/+|\/+$/g, "");
  if (!trimmed) {
    return "";
  }

  const segments = trimmed.split("/");
  if (
    segments.some((segment) => !segment || segment === "." || segment === "..")
  ) {
    return null;
  }

  return segments.join("/");
}

export function folderPathSegments(path: string): string[] {
  return path.split("/").filter(Boolean);
}

export function collectFolderPaths(paths: Iterable<string>): string[] {
  const collected = new Set<string>();

  for (const path of paths) {
    const normalized = normalizeFolderPath(path);
    if (!normalized) {
      continue;
    }

    const segments = folderPathSegments(normalized);
    for (let index = 1; index <= segments.length; index += 1) {
      collected.add(segments.slice(0, index).join("/"));
    }
  }

  return [...collected].sort((left, right) => left.localeCompare(right));
}

export function folderBreadcrumbLabel(path: string): string {
  return folderPathSegments(path).join(" / ");
}
