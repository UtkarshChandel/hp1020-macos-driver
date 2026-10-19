# AGENTS.md: start here

Native macOS driver for the HP LaserJet 1018 / 1020 / **1020 Plus** (host-based ZjStream
lasers): a Rust CUPS filter + Rust USB backend, shipped as an unsigned `.pkg`.
Public repo: https://github.com/UtkarshChandel/hp1020-macos-driver (GPL-2.0-or-later).
Status: v1.0.0 released 2026-10-19 and printing on the maintainer's 1020 Plus (macOS 15.6, arm64).

## Read before changing anything

| Doc | What's in it |
|---|---|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Pipeline, components, file map, install layout |
| [docs/PROTOCOL.md](docs/PROTOCOL.md) | ZjStream records, JBIG parameters, USB, Device ID, firmware blob format |
| [docs/HARDWARE-NOTES.md](docs/HARDWARE-NOTES.md) | Facts measured on the real printer (IDs, endpoints, firmware boot timing) |
| [docs/BRING-UP-LOG.md](docs/BRING-UP-LOG.md) | Chronological log of what was tried, what failed and why. Read it so you don't repeat dead ends |
| [docs/TESTING.md](docs/TESTING.md) | Golden test, no-sudo test loop, real-printer test procedure |
| [docs/RELEASING.md](docs/RELEASING.md) | How to build the `.pkg` and publish a GitHub release |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Open ideas: signing, CI releases, Homebrew, 1018 testing, Wi-Fi bridge |

## Hard rules (each one cost real time to learn)

1. **Never poll or write to the printer during firmware boot.** After uploading firmware,
   wait ≥ 35 s before *any* USB request. A Device-ID request during boot wedges the
   printer until it is power-cycled. Details in HARDWARE-NOTES.md.
2. **Never commit or bundle HP's firmware.** HP's plugin license forbids copying it "onto
   any public network". It is downloaded at install time by `packaging/fetch-firmware.sh`
   and checked against pinned SHA-256s.
3. **Keep `src/jbig.rs` byte-identical to jbig-kit.** `cargo test` (`tests/golden.rs`) must
   pass. Never regenerate `tests/golden/pattern.foo2zjs.zjs` from our own output to make
   the test pass. It is foo2zjs output and is the reference (see docs/TESTING.md).
4. **The package payload must contain no AppleDouble `._` files.** A `._hp1020` would show up
   as a CUPS backend. `packaging/build-pkg.sh` rebuilds the payload with cpio for this
   reason, and fails if any `._` file is left. Don't "simplify" it back to plain `pkgbuild`.
5. **The agent has no sudo.** `sudo` needs the user's password. Installing, uninstalling and
   anything under `/usr/libexec/cups` or `/Library/Printers` must be handed to the user.
   Development and printer tests do **not** need sudo (user-level USB works on macOS 15).
6. The license is GPL because `src/jbig.rs` is a port of jbig-kit. Keep it GPL.
7. Every hardware test may need the user: power-cycling the printer, loading paper,
   reporting what printed. Ask; don't guess what came out.

## Common commands

```bash
cargo build --release && cargo test --release     # build + byte-exact golden test
target/release/hp1020 status                      # model, serial, Device ID, firmware state
target/release/hp1020 send job.zjs                # raw send (uploads firmware first if needed)
packaging/build-pkg.sh                            # -> dist/HP-LaserJet-1020-Driver-<ver>.pkg
```

Release: bump `version` in `Cargo.toml`, commit, `packaging/build-pkg.sh`, then
`gh release create vX.Y.Z dist/*.pkg dist/*.sha256`. See docs/RELEASING.md. (No CI: the
GitHub Actions release workflow was deliberately skipped.)

## Related work on this machine

* `~/Desktop/hp-laser-1008a-macos`: the sibling project (HP Laser 1008a, SPL3 via patched
  SpliX + IOKit backend) whose method this project copied.
* `~/Desktop/hp1020-research` (not in the repo): foo2zjs source, HPLIP plugins, Apple-driver
  extraction from anxkhn/hp1020-driver-mac, scratch rasters/streams from bring-up.
