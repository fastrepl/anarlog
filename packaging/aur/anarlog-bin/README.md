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
- On Wayland with NVIDIA, a blank window is a known WebKitGTK issue. Launch with
  `WEBKIT_DISABLE_DMABUF_RENDERER=1 anarlog`.

## Updating

Bump `pkgver` and replace both `sha256sums_*` entries with the checksums published
on the matching `desktop_v<version>` release, then regenerate metadata:

```bash
makepkg --printsrcinfo > .SRCINFO
```
