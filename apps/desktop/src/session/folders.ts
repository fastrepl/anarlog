export function normalizeFolderPath(path: string): string | null {
  const replaced = path.replace(/\\/g, "/").trim();
  if (!replaced) {
    return "";
  }

  if (replaced.includes("/") || replaced === "." || replaced === "..") {
    return null;
  }

  return replaced;
}

export function folderDisplayName(path: string | null | undefined): string {
  const replaced = (path ?? "").replace(/\\/g, "/").trim();
  if (!replaced) {
    return "";
  }

  const first = replaced.replace(/^\/+/, "").split("/")[0] ?? "";
  if (!first || first === "." || first === "..") {
    return "";
  }

  return first;
}

export function collectFolderPaths(paths: Iterable<string>): string[] {
  const collected = new Set<string>();

  for (const path of paths) {
    const name = folderDisplayName(path);
    if (name) {
      collected.add(name);
    }
  }

  return [...collected].sort((left, right) => left.localeCompare(right));
}
