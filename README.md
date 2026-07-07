# MuseStudio (Windows / cross-platform)

## 이 도구의 목적

**MuseStudio** 는 [LLM Master (LLMStudy)](https://www.resonance-space.net/llmstudy.html) 의 동반 IDE 입니다. 강의 본문 안에 박혀 있는 코드 블록을 클릭 한 번에 열어, 그 자리에서 편집 · 실행 · 결과 확인까지 한 앱에서 할 수 있게 해주는 게 목적입니다.

LLM Agent / AI 엔지니어를 학습하는 사람이 매번 "강의 → 새 탭 → 코드 옮기기 → 환경 만들기 → 실행" 4단계를 거치는 마찰을 0으로 줄이고 싶었습니다. 별도 가입 · 결제 · 데이터 수집 없이, 로컬에서만 동작합니다.

Mac 사용자는 풀 기능 네이티브 빌드 ([MuseEdit](https://www.resonance-space.net/llmstudy.html#musestudio)) 를 권장합니다. 이 repo 의 Tauri 빌드는 **Windows 사용자를 위한 cross-platform 포트**가 1차 목적이고, Mac/Linux 도 같은 코드에서 같이 빌드됩니다.

## Stack

A Tauri 2.0 + React + Monaco port of **MuseEdit** (the macOS-native sibling).

Produces native `.exe` / `.app` / `.deb` / `.AppImage` from a single codebase.

## MVP scope (v0.1)

- Folder tree sidebar (`list_dir` Rust command)
- Multi-tab editor with Monaco syntax highlighting (20+ languages)
- File open / save / save-as via native dialogs
- Keyboard shortcuts: `Ctrl/⌘+O`, `Ctrl/⌘+Shift+O` (folder), `Ctrl/⌘+S`, `Ctrl/⌘+Shift+S` (save as), `Ctrl/⌘+W` (close tab)
- Status bar (language, line/char count, dirty flag)
- Dark theme matching the Mac version's `#0b0d10` palette

## What is NOT in v0.1 (vs Mac parity)

See the MuseEdit macOS-native sibling for the full feature list.

Since shipped (v0.2–v0.4):
- Code execution (`run_code`) with Output panel, Stop button, stderr diagnosis (missing module → one-click pip install)
- Integrated terminal (xterm.js + real PTY, login-shell env) — `pip install`, `python3` REPL, Ctrl+C, Korean I/O. `Ctrl/⌘+\`` to toggle
- Learning Mode side panel + `museedit://` / `musestudio://` URL schemes (deep link + chapter bundle import)

Still deferred:
- Git integration (clone/status/commit/diff/branches)
- Multi-cursor, code folding, minimap (Monaco gives folding/minimap free, multi-cursor exists)
- Settings UI (font/theme picker)
- Plugin system
- Auto-update (Tauri has an updater plugin; not yet wired)

## macOS에서 열기 (Gatekeeper 안내)

릴리스 `.dmg` / `.app` 은 아직 Apple 공증(notarization)·서명이 되어 있지 않아, 처음 실행하면 macOS Gatekeeper 가 "확인되지 않은 개발자" 또는 "손상되었기 때문에 열 수 없습니다" 경고를 띄웁니다. 앱은 정상이며, 아래 방법 중 하나로 열 수 있습니다.

1. **우클릭으로 열기 (권장)**: Finder 에서 `MuseStudio.app` 을 **우클릭(Control+클릭) → 열기 → 열기**. 최초 1회만 하면 이후는 더블클릭으로 실행됩니다.
2. **quarantine 속성 제거 (터미널)**:
   ```bash
   xattr -d com.apple.quarantine /Applications/MuseStudio.app
   ```
3. macOS Ventura 이상에서 1번이 막히면 **시스템 설정 → 개인정보 보호 및 보안 → 보안** 하단의 "확인 없이 열기" 를 클릭하세요.

> English: the release build is not yet notarized/signed. On first launch, right-click the app → **Open → Open**, or run `xattr -d com.apple.quarantine /Applications/MuseStudio.app`. This is required only once.

## Develop on Mac

```bash
cd /path/to/MuseStudioWin
pnpm install            # pnpm 11+ (pnpm-workspace.yaml 의 allowBuilds 필드는 pnpm 9 가 파싱 못 함)
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
