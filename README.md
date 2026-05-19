# MuseStudio (Windows / cross-platform)

A Tauri 2.0 + React + Monaco port of MuseStudio Mac (`/Users/gongyoonkyoung/Projects/MuseEdit`).

Produces native `.exe` / `.app` / `.deb` / `.AppImage` from a single codebase.

## MVP scope (v0.1)

- Folder tree sidebar (`list_dir` Rust command)
- Multi-tab editor with Monaco syntax highlighting (20+ languages)
- File open / save / save-as via native dialogs
- Keyboard shortcuts: `Ctrl/⌘+O`, `Ctrl/⌘+Shift+O` (folder), `Ctrl/⌘+S`, `Ctrl/⌘+Shift+S` (save as), `Ctrl/⌘+W` (close tab)
- Status bar (language, line/char count, dirty flag)
- Dark theme matching the Mac version's `#0b0d10` palette

## What is NOT in v0.1 (vs Mac parity)

See `/Users/gongyoonkyoung/Projects/MuseEdit` for the full feature list. Deferred:
- Git integration (clone/status/commit/diff/branches)
- Integrated terminal
- Code execution (`run_code` Rust command stub exists)
- Multi-cursor, code folding, minimap (Monaco gives folding/minimap free, multi-cursor exists)
- Learning Mode side panel + `museedit://` URL scheme
- Settings UI (font/theme picker)
- Plugin system
- Auto-update (Tauri has an updater plugin; not yet wired)

## Develop on Mac

```bash
cd /Users/gongyoonkyoung/Projects/MuseStudioWin
pnpm install
pnpm tauri dev          # spawn dev window
pnpm tauri build        # produce .app (Mac) — outputs in src-tauri/target/release/bundle/
```

First run will compile ~400 Rust crates; ~3–8 min cold cache. Subsequent builds are incremental.

## Build for Windows

Tauri **cannot** cross-compile to Windows from macOS reliably (the WebView2 / MSVC dependency makes it fragile). The recommended path is one of:

1. **Build on Windows host**: install Rust + Node + WebView2 runtime, then
   ```powershell
   git clone <repo>
   cd MuseStudioWin
   pnpm install
   pnpm tauri build
   ```
   Outputs `src-tauri\target\release\bundle\msi\MuseStudio_0.1.0_x64.msi` and `nsis\MuseStudio_0.1.0_x64-setup.exe`.

2. **GitHub Actions (recommended for releases)**: a `windows-latest` runner can build and sign the `.msi`/`.exe` automatically. See `https://tauri.app/distribute/pipelines/github/` for the workflow template.

3. **Local Windows VM (Parallels/UTM)**: clone the repo, install Rust + Node, run `pnpm tauri build`.

## Project layout

```
src/                 # React + Monaco UI
  components/        # Sidebar, TabBar, EditorPane, StatusBar
  App.tsx            # Orchestration + keyboard shortcuts
  types.ts           # Tab, DirEntry
src-tauri/
  src/lib.rs         # Tauri commands: read_file, write_file, list_dir, search_files, run_code
  src/main.rs        # Entry point
  tauri.conf.json    # App config (productName, identifier, window size)
  capabilities/default.json  # Permission grants (dialog, fs, shell, opener)
  Cargo.toml         # Rust deps: tauri, walkdir, serde
```

## Next steps for parity

| Feature | Approach |
|---|---|
| Git integration | `git2` crate in Rust → expose `git_status`, `git_diff`, `git_commit` commands; reuse Mac's `DiffViewer` patterns in React |
| Integrated terminal | `xterm.js` + Rust PTY (`portable-pty` crate) bridge |
| Code execution | `run_code` exists — wire UI bottom panel + streaming stdout |
| `museedit://` deep link | Tauri 2.0 single-instance + deep-link plugin |
| Auto-update | `tauri-plugin-updater` + signing key |
| Learning Mode | Port the Swift `LearningModeSidePanel` view + env var detection |

## Why Tauri (vs Electron / Flutter)

- **30 MB installer** vs Electron 150 MB. Uses system WebView2 (Edge runtime ships with Win10+).
- **Native Rust backend** — git/FS/process management is native speed.
- **Same Web/React skills** as LLMStudy (Next.js + TS). Easy cross-team handoff.
- **Multi-platform from one codebase**: same `pnpm tauri build` produces Win/Mac/Linux.
