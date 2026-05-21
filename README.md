# 🎯 cs2-killtimer

A tiny external overlay for Counter-Strike 2 that shows a countdown timer between your kills. Read-only memory access, no DLL injection, no hooks.


## 🌟 Highlights

- Lightweight Win32 layered-window overlay, no egui or game hooks
- External read-only attach to `cs2.exe`, no code injection
- Adaptive polling (idle / combat / timer) to keep CPU near zero
- Color-banded countdown (green → orange → red) so you can read it at a glance
- Single Windows exe, no installer, no runtime deps
- Offsets fetched from [a2x/cs2-dumper](https://github.com/a2x/cs2-dumper) at startup


## ℹ️ Overview

When you get a kill, a streak timer appears on screen. Stack kills fast and it stays green. Let it run down and it turns red. The overlay never touches the game process beyond `ReadProcessMemory`, so it does not modify game state and does not interact with VAC-protected code paths.

This is a personal project for tracking streak windows during deathmatch and casual play. Use at your own discretion.


## ⬇️ Install

Grab the latest `cs2-killtimer.exe` from the [Releases page](https://github.com/samhoque/cs2-killtimer/releases) and run it. Windows 10/11, x64 only.


## 🛠️ Build from source

```bash
cargo build --release
```

Output lands at `target/release/cs2-killtimer.exe`.


## 🚀 Usage

1. Launch CS2.
2. Run `cs2-killtimer.exe`.
3. The overlay appears as a transparent layer on top of your game.

Close the console window to exit.


## 🙏 Credits

- Offsets courtesy of [cs2-dumper](https://github.com/a2x/cs2-dumper).
- Overlay rendering uses [ab_glyph](https://github.com/alexheretic/ab-glyph) for text rasterization.
