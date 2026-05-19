import Editor from "@monaco-editor/react";
import type { Tab } from "../types";

type Props = {
  tab: Tab | null;
  onChange: (next: string) => void;
  onSave: () => void;
};

export function EditorPane({ tab, onChange }: Props) {
  if (!tab) {
    return (
      <div className="flex-1 flex items-center justify-center text-muted text-sm">
        <div className="text-center">
          <div className="text-2xl font-semibold mb-2 text-fg">MuseStudio</div>
          <div>
            Open a folder (<kbd className="px-1.5 py-0.5 bg-panel rounded">⌘/Ctrl+Shift+O</kbd>)
            or a file (<kbd className="px-1.5 py-0.5 bg-panel rounded">⌘/Ctrl+O</kbd>) to start.
          </div>
        </div>
      </div>
    );
  }
  return (
    <div className="flex-1 min-h-0">
      <Editor
        height="100%"
        theme="vs-dark"
        language={tab.language}
        value={tab.content}
        onChange={(v) => onChange(v ?? "")}
        options={{
          fontSize: 13,
          minimap: { enabled: true },
          wordWrap: "on",
          renderWhitespace: "selection",
          smoothScrolling: true,
          scrollBeyondLastLine: false,
          fontFamily: "Menlo, Consolas, 'Liberation Mono', monospace",
        }}
      />
    </div>
  );
}
