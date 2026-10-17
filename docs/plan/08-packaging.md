# 08. Packaging and installation

## Desktop: one file per platform

Tauri's bundler builds, from one codebase:

| Platform | File | Notes |
|---|---|---|
| Windows | `wayfarer-setup.exe` (NSIS) | Per-user install, no administrator rights needed |
| macOS | `wayfarer.dmg` | Universal (Apple silicon and Intel) |
| Linux | `wayfarer.AppImage` | Runs anywhere; a `.deb` as well |

Each contains the core (gateway and brain in one process), the web UI, the stock world pack
and the fonts. Double-click, install, run: the first-run wizard (`06-accounts-and-setup.md`)
takes it from there.

**Updates.** Tauri's updater checks GitHub Releases for a signed update manifest and offers a
one-click update. Settings, logs and statistics live in the user's data directory and survive
updates and reinstalls.

## Signing (*Needs John* before a public release)

Unsigned installers work but scare non-technical users:

- **Windows** shows SmartScreen ("Windows protected your PC", then "Run anyway") until the
  installer is signed and builds reputation. Options: a code-signing certificate, or
  Microsoft's Trusted Signing service (a monthly fee).
- **macOS** blocks unnotarised apps; the user must allow them in System Settings. Signing and
  notarisation need an Apple Developer account (yearly fee).

During development everything ships unsigned.

## Headless

`wayfarer` is also a plain binary for Linux, macOS and Windows with no desktop parts:
`wayfarer serve` (core plus the web UI on a port) and `wayfarer tui`. A Docker image is
optional. This is what runs on a server for long bot sessions, watched from a browser or over
SSH.

## Release builds

GitHub Actions builds every platform on a tag (Windows and macOS runners for their
installers), attaches the files to the GitHub Release, and publishes the update manifest.
