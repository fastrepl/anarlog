export function tildify(path: string, home: string) {
  return path.startsWith(home + "/") ? "~" + path.slice(home.length) : path;
}

export function shortenPath(path: string, maxLength = 48): string {
  if (path.length <= maxLength) return path;
  const short = path.slice(path.length - maxLength);
  const slash = short.indexOf("/");
  return "\u2026" + (slash > 0 ? short.slice(slash) : short);
}

export function displayPath(
  path: string | undefined,
  home: string | undefined,
): string {
  if (!path) return "Loading...";
  const tildified = home ? tildify(path, home) : path;
  return shortenPath(tildified);
}
