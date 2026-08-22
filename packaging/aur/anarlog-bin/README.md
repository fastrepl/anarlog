# anarlog-bin

Arch Linux package for Anarlog, repackaged from the official Linux `.deb` release.
Works on Arch and Arch-based distros such as Omarchy, EndeavourOS, and Manjaro.

## Install

```bash
git clone https://github.com/fastrepl/anarlog.git
cd anarlog/packaging/aur/anarlog-bin
makepkg -si
```

`makepkg` downloads the release `.deb` for your architecture (x86_64 or aarch64),
verifies its checksum, and installs it through pacman.

## Notes

- Tray icons need `libayatana-appindicator`, which is an optional dependency.
- On Wayland with NVIDIA, a blank window is a known WebKitGTK issue. The app
  disables the DMA-BUF renderer by default. To override, launch with
  `WEBKIT_DISABLE_DMABUF_RENDERER=0 anarlog`. If a window is still blank, launch
  with `WEBKIT_DISABLE_DMABUF_RENDERER=1 WEBKIT_DISABLE_COMPOSITING_MODE=1 anarlog`.

## Updating

Each stable publish opens a `chore(packaging): bump anarlog-bin to <version>` pull
request automatically, so this normally needs no manual work.

To bump by hand, or to refresh the files before an AUR push:

```bash
node scripts/update-aur-pkgbuild.mjs --version 1.4.9
```

It reads the checksums from that release's published `.sha256` assets and rewrites
`pkgver`, `pkgrel`, and every checksum in both `PKGBUILD` and `.SRCINFO`.

After changing package metadata by hand (dependencies, description, options),
regenerate `.SRCINFO` on an Arch machine so it stays in sync:

```bash
makepkg --printsrcinfo > .SRCINFO
```
