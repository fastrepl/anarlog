import Foundation

// Get the current process path (this binary's name)
let fileManager = FileManager.default
let myName = CommandLine.arguments[0]
let myBasename = URL(fileURLWithPath: myName).lastPathComponent

// Arguments to pass through (excluding the first one which is this process name)
let passthroughArgs = Array(CommandLine.arguments.dropFirst())

// Check if we have at least one argument (the executable to run)
guard !passthroughArgs.isEmpty else {
  fputs("Usage: \(myBasename) <executable> [arguments...]\n", stderr)
  exit(1)
}

// The first argument is the executable to run
let executableToRun = passthroughArgs[0]
// The remaining arguments are passed to that executable
let argsForExecutable = Array(passthroughArgs.dropFirst())

// Function to resolve executable path
func resolveExecutablePath(_ executable: String) -> String? {
  // If it's already an absolute path or relative path with /, use it directly
  if executable.contains("/") {
    let url = URL(fileURLWithPath: executable)
    let expandedPath = url.path.replacingOccurrences(
      of: "~", with: FileManager.default.homeDirectoryForCurrentUser.path)
    if FileManager.default.fileExists(atPath: expandedPath) {
      return expandedPath
    }
    return nil
  }

  // Otherwise, search in PATH
  let pathEnv = ProcessInfo.processInfo.environment["PATH"] ?? "/usr/local/bin:/usr/bin:/bin"
  let paths = pathEnv.split(separator: ":").map(String.init)

  for path in paths {
    let fullPath = "\(path)/\(executable)"
    let expandedPath = fullPath.replacingOccurrences(
      of: "~", with: FileManager.default.homeDirectoryForCurrentUser.path)
    if FileManager.default.fileExists(atPath: expandedPath) {
      return expandedPath
    }
  }

  return nil
}

// Resolve the executable path
guard let resolvedPath = resolveExecutablePath(executableToRun) else {
  fputs(
    "Error running command '\(executableToRun)': Error Domain=NSCocoaErrorDomain Code=4 \"The file \\\"\(executableToRun)\\\" doesn't exist.\" UserInfo={NSFilePath=\(executableToRun)}\n",
    stderr)
  exit(1)
}

// Build the new command to run
let task = Process()
task.executableURL = URL(fileURLWithPath: resolvedPath)
task.arguments = argsForExecutable

// Pipe stdout and stderr
task.standardOutput = FileHandle.standardOutput
task.standardError = FileHandle.standardError
task.standardInput = FileHandle.standardInput

// Set up signal handling for clean termination
signal(SIGINT) { _ in
  task.terminate()
  exit(130)  // Standard exit code for SIGINT
}

signal(SIGTERM) { _ in
  task.terminate()
  exit(143)  // Standard exit code for SIGTERM
}

do {
  try task.run()
  task.waitUntilExit()
  exit(task.terminationStatus)
} catch {
  fputs("Error running command '\(executableToRun)': \(error)\n", stderr)
  exit(1)
}
