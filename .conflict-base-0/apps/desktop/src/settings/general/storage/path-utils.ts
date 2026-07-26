function tildify(path: string, home: string) {
  return path.startsWith(home + "/") ? "~" + path.slice(home.length) : path;
}

function shortenPath(path: string, maxLength = 48): string {
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

// macOS file-provider mounts are named "<Provider>-<AccountName>" under ~/Library/CloudStorage.
const CLOUD_MOUNT_PREFIXES: Array<[string, string]> = [
  ["Dropbox", "Dropbox"],
  ["OneDrive", "OneDrive"],
  ["GoogleDrive", "Google Drive"],
  ["Box", "Box"],
  ["ProtonDrive", "Proton Drive"],
];

export function detectCloudStorageService(path: string): string | null {
  const segments = path.split(/[\\/]+/).filter(Boolean);

  const mobileDocsIndex = segments.indexOf("Mobile Documents");
  if (mobileDocsIndex > 0 && segments[mobileDocsIndex - 1] === "Library") {
    return "iCloud Drive";
  }

  const cloudStorageIndex = segments.indexOf("CloudStorage");
  if (cloudStorageIndex > 0 && segments[cloudStorageIndex - 1] === "Library") {
    const mount = segments[cloudStorageIndex + 1];
    if (mount) {
      for (const [prefix, label] of CLOUD_MOUNT_PREFIXES) {
        if (mount.startsWith(prefix)) return label;
      }
      return mount.split("-")[0] || null;
    }
  }

  for (const segment of segments) {
    if (segment === "Dropbox") return "Dropbox";
    if (segment === "OneDrive" || segment.startsWith("OneDrive - ")) {
      return "OneDrive";
    }
    if (segment === "Google Drive" || segment === "GoogleDrive") {
      return "Google Drive";
    }
    if (segment === "iCloudDrive" || segment === "iCloud Drive") {
      return "iCloud Drive";
    }
  }

  return null;
}
