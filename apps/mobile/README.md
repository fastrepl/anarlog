# Mobile Apps

This directory is organized by platform:

```text
apps/mobile/
├── ios/
│   └── HyprMobile/
└── android/
```

## iOS

The iOS app lives at `apps/mobile/ios/HyprMobile` and is a native SwiftUI app.

Generate iOS Rust bridge artifacts with:

```bash
cargo xtask mobile-bridge
```

`cargo xtask mobile-bridge` writes generated files to:

- `apps/mobile/ios/HyprMobile/Generated/`
- `apps/mobile/ios/HyprMobile/MobileBridge.xcframework/`

Those outputs are generated artifacts and are gitignored.

## Android

`apps/mobile/android/` is a placeholder for future Android work.
