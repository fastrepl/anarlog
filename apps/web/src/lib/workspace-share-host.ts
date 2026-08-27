const RESERVED_WORKSPACE_SHARE_HOSTS = new Set([
  "admin",
  "api",
  "app",
  "assets",
  "auth",
  "cdn",
  "desktop",
  "dev",
  "docs",
  "mail",
  "staging",
  "static",
  "status",
  "support",
  "www",
]);

export const isWorkspaceShareHostname = (hostname: string) => {
  const suffix = ".anarlog.so";
  if (!hostname.endsWith(suffix)) return false;
  const slug = hostname.slice(0, -suffix.length);
  return (
    /^[a-z0-9][a-z0-9-]{1,61}[a-z0-9]$/.test(slug) &&
    !RESERVED_WORKSPACE_SHARE_HOSTS.has(slug)
  );
};
