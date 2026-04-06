// swift-tools-version: 5.9

import PackageDescription

let package = Package(
  name: "CliUI",
  platforms: [.macOS(.v14)],
  targets: [
    .executableTarget(
      name: "char-cli-ui",
      path: "Sources/CliUI"
    )
  ]
)
