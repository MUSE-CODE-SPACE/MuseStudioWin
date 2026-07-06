import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { DirEntry } from "../types";

type Props = {
  rootDir: string | null;
  onOpenFolder: () => void;
  onOpenFile: () => void;
  onFileClick: (path: string) => void;
};

export function Sidebar({ rootDir, onOpenFolder, onOpenFile, onFileClick }: Props) {
  return (
    <aside className="w-64 shrink-0 bg-panel border-r border-border flex flex-col">
      <div className="px-3 py-2 border-b border-border flex items-center justify-between">
        <span className="text-xs uppercase tracking-wider text-muted">Explorer</span>
        <div className="flex gap-1">
          <button
            onClick={onOpenFile}
            className="text-xs px-2 py-0.5 rounded hover:bg-border"
            title="Open File (Ctrl/⌘+O)"
          >
            File
          </button>
          <button
            onClick={onOpenFolder}
            className="text-xs px-2 py-0.5 rounded hover:bg-border"
            title="Open Folder (Ctrl/⌘+Shift+O)"
          >
            Folder
          </button>
        </div>
      </div>
      <div className="flex-1 overflow-y-auto">
        {rootDir ? (
          // key=rootDir — 루트 폴더가 바뀔 때 (Open Folder / 번들 import) 서브트리를
          // 리마운트해 이전 폴더의 캐시된 entries 가 그대로 남던 문제 방지.
          <FolderNode key={rootDir} path={rootDir} depth={0} onFileClick={onFileClick} initiallyOpen />
        ) : (
          <p className="px-3 py-4 text-xs text-muted">
            No folder open. Click <b>Folder</b> above to start.
          </p>
        )}
      </div>
    </aside>
  );
}

function FolderNode({
  path,
  depth,
  onFileClick,
  initiallyOpen = false,
}: {
  path: string;
  depth: number;
  onFileClick: (path: string) => void;
  initiallyOpen?: boolean;
}) {
  const [open, setOpen] = useState(initiallyOpen);
  const [entries, setEntries] = useState<DirEntry[] | null>(null);
  const [loading, setLoading] = useState(false);
  const name = path.split("/").pop() || path;

  useEffect(() => {
    if (open && entries === null && !loading) {
      setLoading(true);
      invoke<DirEntry[]>("list_dir", { path })
        .then(setEntries)
        .catch((e) => {
          console.error(e);
          setEntries([]);
        })
        .finally(() => setLoading(false));
    }
  }, [open, entries, loading, path]);

  return (
    <div>
      <button
        onClick={() => setOpen((v) => !v)}
        className="w-full text-left text-sm hover:bg-border/60 truncate flex items-center"
        style={{ paddingLeft: 8 + depth * 12, paddingTop: 2, paddingBottom: 2 }}
      >
        <span className="text-muted mr-1 w-3 inline-block">{open ? "▾" : "▸"}</span>
        <span className="truncate">{depth === 0 ? path : name}</span>
      </button>
      {open && entries && (
        <div>
          {entries.length === 0 && (
            <div
              className="text-xs text-muted italic"
              style={{ paddingLeft: 8 + (depth + 1) * 12 }}
            >
              (empty)
            </div>
          )}
          {entries.map((e) =>
            e.is_dir ? (
              <FolderNode
                key={e.path}
                path={e.path}
                depth={depth + 1}
                onFileClick={onFileClick}
              />
            ) : (
              <button
                key={e.path}
                onClick={() => onFileClick(e.path)}
                className="w-full text-left text-sm hover:bg-border/60 truncate block"
                style={{
                  paddingLeft: 8 + (depth + 1) * 12 + 14,
                  paddingTop: 2,
                  paddingBottom: 2,
                }}
              >
                {e.name}
              </button>
            ),
          )}
        </div>
      )}
    </div>
  );
}
