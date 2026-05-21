import type { Tab } from "../types";

type Props = {
  tab: Tab | null;
  runnable?: boolean;
  onRun?: () => void;
};

export function StatusBar({ tab, runnable, onRun }: Props) {
  return (
    <footer className="bg-panel border-t border-border text-xs text-muted h-6 flex items-center px-3 gap-4 shrink-0">
      <span>
        {tab ? (
          <>
            <span className="text-fg">{tab.language}</span>
            <span className="mx-2">·</span>
            <span>{tab.content.split("\n").length} lines</span>
            <span className="mx-2">·</span>
            <span>{tab.content.length.toLocaleString()} chars</span>
            {tab.dirty && (
              <>
                <span className="mx-2">·</span>
                <span className="text-accent">unsaved</span>
              </>
            )}
          </>
        ) : (
          <span>Ready</span>
        )}
      </span>
      {runnable && onRun && (
        <button
          onClick={onRun}
          className="px-2 py-0.5 rounded bg-blue-600 hover:bg-blue-500 text-white text-[10px] font-medium"
          title="Run (Ctrl/⌘+R)"
        >
          ▶ Run
        </button>
      )}
      <span className="ml-auto">UTF-8</span>
      <span>MuseStudio v0.3</span>
    </footer>
  );
}
