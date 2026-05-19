import type { Tab } from "../types";

type Props = {
  tab: Tab | null;
};

export function StatusBar({ tab }: Props) {
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
      <span className="ml-auto">UTF-8</span>
      <span>MuseStudio v0.1</span>
    </footer>
  );
}
