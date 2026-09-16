import { Directory, File, Paths } from "expo-file-system";

const DATABASE_FILENAME = "anarlog.db";
const RESET_MARKER_FILENAME = "anarlog.db.reset-requested";

function databaseDirectory(): Directory {
  return new Directory(Paths.document, "SQLite");
}

// Runs before the bridge opens. Returns the backup name when a reset happened.
export function applyPendingLocalDatabaseReset(): string | null {
  const directory = databaseDirectory();
  const marker = new File(directory, RESET_MARKER_FILENAME);
  if (!marker.exists) return null;
  const stamp = Math.floor(Date.now() / 1000);
  const backupName = `${DATABASE_FILENAME}.${stamp}.bak`;
  for (const suffix of ["", "-wal", "-shm"]) {
    const source = new File(directory, `${DATABASE_FILENAME}${suffix}`);
    if (!source.exists) continue;
    source.move(new File(directory, `${backupName}${suffix}`));
  }
  marker.delete();
  return backupName;
}
