import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { onOpenUrl, getCurrent as getCurrentDeepLink } from "@tauri-apps/plugin-deep-link";
import { Sidebar } from "./components/Sidebar";
import { EditorPane } from "./components/EditorPane";
import { StatusBar } from "./components/StatusBar";
import { TabBar } from "./components/TabBar";
import { RunOutputPanel } from "./components/RunOutputPanel";
import { LearningModePanel } from "./components/LearningModePanel";
import { parseMuseEditUrl } from "./lib/museeditUrl";
import type { Tab } from "./types";
import "./App.css";

/// run_code 가 인식하는 언어 셋. App.tsx 의 languageFromPath 결과를
/// 그대로 쓸 수 있는 7개 — 다른 (markdown / json / html 등) 은 Run 버튼 비활성.
const RUNNABLE_LANGS = new Set([
  "python", "javascript", "typescript", "shell", "ruby", "go", "rust",
]);

function languageFromPath(path: string): string {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  return (
    {
      js: "javascript",
      jsx: "javascript",
      ts: "typescript",
      tsx: "typescript",
      py: "python",
      rb: "ruby",
      go: "go",
      rs: "rust",
      sh: "shell",
      bash: "shell",
      md: "markdown",
      json: "json",
      yaml: "yaml",
      yml: "yaml",
      toml: "toml",
      html: "html",
      css: "css",
      swift: "swift",
      java: "java",
      kt: "kotlin",
      c: "c",
      cpp: "cpp",
      h: "c",
      hpp: "cpp",
      sql: "sql",
      xml: "xml",
    }[ext] ?? "plaintext"
  );
}

export default function App() {
  const [rootDir, setRootDir] = useState<string | null>(null);
  const [tabs, setTabs] = useState<Tab[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [runPanelOpen, setRunPanelOpen] = useState(false);

  const activeTab = tabs.find((t) => t.id === activeId) ?? null;

  async function openFolder() {
    const picked = await openDialog({ directory: true });
    if (typeof picked === "string") setRootDir(picked);
  }

  async function openFileFromDialog() {
    const picked = await openDialog({ directory: false, multiple: false });
    if (typeof picked === "string") await openPath(picked);
  }

  async function openPath(path: string) {
    const existing = tabs.find((t) => t.path === path);
    if (existing) {
      setActiveId(existing.id);
      return;
    }
    try {
      const content = await invoke<string>("read_file", { path });
      const id = `${path}#${Date.now()}`;
      const tab: Tab = {
        id,
        path,
        name: path.split("/").pop() ?? path,
        language: languageFromPath(path),
        content,
        dirty: false,
      };
      setTabs((prev) => [...prev, tab]);
      setActiveId(id);
    } catch (e) {
      console.error(e);
      alert(`Failed to open: ${e}`);
    }
  }

  /// museedit://open?... deeplink 한 건을 처리. base64 decode → temp 파일 작성 →
  /// 새 탭 추가 + lesson 메타 첨부. run=1 이면 자동으로 Run 패널 띄움 (실제 실행은
  /// 사용자 확인을 위해 한 번 더 클릭). 실패해도 silent (alert 단계만).
  async function handleDeepLink(rawUrl: string) {
    const parsed = parseMuseEditUrl(rawUrl);
    if (!parsed) return;
    try {
      // 임시 파일 경로 — Tauri 의 path.tempDir 으로 system temp 받기.
      const { tempDir, join } = await import("@tauri-apps/api/path");
      const dir = await tempDir();
      // 슬러그 있으면 그 이름 사용, 아니면 timestamp.
      const baseName = parsed.lesson?.slug
        ? `${parsed.lesson.slug.replace(/[^\w.-]+/g, "_")}.${parsed.ext}`
        : `museedit_${Date.now()}.${parsed.ext}`;
      const path = await join(dir, "musestudio", baseName);
      await invoke("write_file", { path, contents: parsed.code });
      const existing = tabs.find((t) => t.path === path);
      if (existing) {
        // 같은 파일 다시 열기 — content 갱신해 새 버전 코드 표시.
        setTabs((prev) =>
          prev.map((t) =>
            t.path === path
              ? { ...t, content: parsed.code, dirty: false, lesson: parsed.lesson }
              : t,
          ),
        );
        setActiveId(existing.id);
      } else {
        const id = `${path}#${Date.now()}`;
        setTabs((prev) => [
          ...prev,
          {
            id, path, name: baseName,
            language: parsed.language,
            content: parsed.code,
            dirty: false,
            lesson: parsed.lesson,
          },
        ]);
        setActiveId(id);
      }
      if (parsed.autoRun && RUNNABLE_LANGS.has(parsed.language)) {
        setRunPanelOpen(true);
      }
    } catch (e) {
      console.error("deeplink handle failed:", e);
    }
  }

  function updateActiveContent(next: string) {
    if (!activeTab) return;
    setTabs((prev) =>
      prev.map((t) =>
        t.id === activeTab.id ? { ...t, content: next, dirty: true } : t,
      ),
    );
  }

  async function saveActive() {
    if (!activeTab) return;
    try {
      await invoke("write_file", {
        path: activeTab.path,
        contents: activeTab.content,
      });
      setTabs((prev) =>
        prev.map((t) => (t.id === activeTab.id ? { ...t, dirty: false } : t)),
      );
    } catch (e) {
      console.error(e);
      alert(`Save failed: ${e}`);
    }
  }

  async function saveActiveAs() {
    if (!activeTab) return;
    const picked = await saveDialog({ defaultPath: activeTab.path });
    if (!picked || typeof picked !== "string") return;
    try {
      await invoke("write_file", { path: picked, contents: activeTab.content });
      setTabs((prev) =>
        prev.map((t) =>
          t.id === activeTab.id
            ? {
                ...t,
                path: picked,
                name: picked.split("/").pop() ?? picked,
                language: languageFromPath(picked),
                dirty: false,
              }
            : t,
        ),
      );
    } catch (e) {
      console.error(e);
      alert(`Save As failed: ${e}`);
    }
  }

  function closeTab(id: string) {
    const tab = tabs.find((t) => t.id === id);
    if (!tab) return;
    if (tab.dirty && !confirm(`'${tab.name}' has unsaved changes. Close anyway?`)) {
      return;
    }
    const next = tabs.filter((t) => t.id !== id);
    setTabs(next);
    if (activeId === id) setActiveId(next[next.length - 1]?.id ?? null);
  }

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const mod = e.metaKey || e.ctrlKey;
      if (!mod) return;
      if (e.key === "s" && !e.shiftKey) {
        e.preventDefault();
        saveActive();
      } else if (e.key === "s" && e.shiftKey) {
        e.preventDefault();
        saveActiveAs();
      } else if (e.key === "o" && !e.shiftKey) {
        e.preventDefault();
        openFileFromDialog();
      } else if (e.key === "o" && e.shiftKey) {
        e.preventDefault();
        openFolder();
      } else if (e.key === "w") {
        e.preventDefault();
        if (activeId) closeTab(activeId);
      } else if (e.key === "r" && !e.shiftKey) {
        // Ctrl/⌘+R 로 Run 패널 열기 (이미 열려 있으면 토글). 실제 실행은 패널의 ▶ Run.
        e.preventDefault();
        if (activeTab && RUNNABLE_LANGS.has(activeTab.language)) {
          setRunPanelOpen(true);
        }
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeTab, tabs, activeId]);

  // museedit:// deeplink — runtime 에 들어오는 URL 도 받고, cold-start 시점에
  // 함께 넘어온 URL 도 처리 (Windows installer 가 OS 가 보낸 첫 URL).
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        unlisten = await onOpenUrl((urls) => {
          for (const u of urls) handleDeepLink(u);
        });
        const initial = await getCurrentDeepLink();
        if (initial) {
          for (const u of initial) handleDeepLink(u);
        }
      } catch (e) {
        // Plugin 등록 안 됐거나 권한 거부 — silent (deep-link 미지원 OS).
        console.warn("deep-link init skipped:", e);
      }
    })();
    return () => { unlisten?.(); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div className="flex flex-col h-full">
      <div className="flex flex-1 overflow-hidden">
        <Sidebar
          rootDir={rootDir}
          onOpenFolder={openFolder}
          onOpenFile={openFileFromDialog}
          onFileClick={openPath}
        />
        <div className="flex flex-col flex-1 min-w-0">
          <TabBar
            tabs={tabs}
            activeId={activeId}
            onSelect={setActiveId}
            onClose={closeTab}
          />
          <div className="flex flex-1 min-h-0">
            <div className="flex flex-col flex-1 min-w-0">
              <EditorPane
                tab={activeTab}
                onChange={updateActiveContent}
                onSave={saveActive}
              />
              <RunOutputPanel
                path={activeTab?.path ?? null}
                language={activeTab?.language ?? ""}
                visible={runPanelOpen}
                onClose={() => setRunPanelOpen(false)}
              />
            </div>
            {/* Learning Mode 사이드 패널 — 현재 탭이 museedit:// 로 들어온 경우에만. */}
            {activeTab?.lesson && <LearningModePanel lesson={activeTab.lesson} />}
          </div>
        </div>
      </div>
      <StatusBar
        tab={activeTab}
        runnable={!!activeTab && RUNNABLE_LANGS.has(activeTab.language)}
        onRun={() => setRunPanelOpen(true)}
      />
    </div>
  );
}
