# sqlite-sync (CloudSync extension)

| Field      | Value                                                                                                             |
| ---------- | ----------------------------------------------------------------------------------------------------------------- |
| Upstream   | https://github.com/sqliteai/sqlite-sync                                                                           |
| Release    | 1.2.2, published November 6, 2026                                                                                 |
| Revision   | `cc07d35861e961d97e4554aa05abd406a5151553`                                                                        |
| License    | Apache License 2.0; the upstream `LICENSE` at that revision is copied to [LICENSE](LICENSE)                       |
| Copyright  | SQLite Cloud, Inc. (the `sqliteai` organization). Upstream has no `NOTICE` file and its sources carry no notices. |
| How used   | Unmodified binaries, loaded as a SQLite extension. No upstream source is copied into this repository.            |
| Bundled in | Desktop (macOS, Linux, Windows) and mobile (iOS, Android) apps                                                    |

Releases before 1.2.1 were under a modified Elastic License 2.0. Do not vendor one.

## Files

Each directory holds the binary from the matching release archive, unmodified. The SHA-256 is of the archive as downloaded.

| Path                              | Release archive                           | SHA-256                                                            |
| --------------------------------- | ----------------------------------------- | ------------------------------------------------------------------ |
| `macos/aarch64/cloudsync.dylib`   | `cloudsync-macos-arm64-1.2.2.zip`         | `e84998e180e665dd36f04ca6fcff8790c2e4b8840e68c85cee2df7cc2951ec08` |
| `macos/x86_64/cloudsync.dylib`    | `cloudsync-macos-x86_64-1.2.2.zip`        | `3e858e19d0fd2cdb8dd1a095f1e5dadc6db49e362012ef16a4194f33836423e5` |
| `linux/gnu/x86_64/cloudsync.so`   | `cloudsync-linux-x86_64-1.2.2.zip`        | `1d655d3fccc00fdd0ff8bbf12b3d7144972e6fccbe690c486209e421807d3367` |
| `linux/gnu/aarch64/cloudsync.so`  | `cloudsync-linux-arm64-1.2.2.zip`         | `fd5ca5520771d52de0b7a431fdfcef15f78b5347421b3e9674e42fd4b1a8ea7f` |
| `linux/musl/x86_64/cloudsync.so`  | `cloudsync-linux-musl-x86_64-1.2.2.zip`   | `4c900c9dafdc2583ef2be9ca8cb6db520792f0146f415182da322364baabd70d` |
| `linux/musl/aarch64/cloudsync.so` | `cloudsync-linux-musl-arm64-1.2.2.zip`    | `c9c9b7d94b9d47f95ceb74337fcd78dd52915fc1ac8ca4984c1f262ae30bae8b` |
| `windows/x86_64/cloudsync.dll`    | `cloudsync-windows-x86_64-1.2.2.zip`      | `2723f25060fa201f4b5d309274f59ebbffc7fc29325ea668abccd2690a090b93` |
| `android/arm64-v8a/cloudsync.so`  | `cloudsync-android-arm64-v8a-1.2.2.zip`   | `d56dedb7a2c666e4568d512c4716ef2a8f08edd2d316dcdbc808937afc756a9b` |
| `android/armeabi-v7a/cloudsync.so` | `cloudsync-android-armeabi-v7a-1.2.2.zip` | `c5f416b9d8dafac8cd85f14c567652d5068b2ef411fba17934194c02f2071c68` |
| `android/x86_64/cloudsync.so`     | `cloudsync-android-x86_64-1.2.2.zip`      | `83234728d2d0ba4956d170526270d9f6111556fb388cf4744ac76dada41d2c92` |
| `apple/CloudSync.xcframework`     | `cloudsync-apple-xcframework-1.2.2.zip`   | `a985836078871d10bb99f462b2a53e8b12b5c3479f06c1580758234aa22818ee` |

The `scripts/rebuild-*.sh` scripts build the same revision from source; a release that rebuilds replaces these files with its own build of that revision.

## Notices

- The desktop bundle ships `LICENSE` as `sqlite-sync-LICENSE.txt` (`apps/desktop/src-tauri/tauri.conf.json`).
- Not done yet: the mobile apps do not surface the license, and upstream's binaries statically link libcurl 8.12.1 (and OpenSSL 3.6.0 on Android) whose notices are not collected here.
