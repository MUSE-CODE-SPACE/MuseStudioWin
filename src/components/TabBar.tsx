import type { Tab } from "../types";

type Props = {
  tabs: Tab[];
  activeId: string | null;
  onSelect: (id: string) => void;
  onClose: (id: string) => void;
};

export function TabBar({ tabs, activeId, onSelect, onClose }: Props) {
  if (tabs.length === 0) return null;
  return (
    <div className="flex bg-panel border-b border-border overflow-x-auto h-9 items-stretch">
      {tabs.map((t) => {
        const isActive = t.id === activeId;
        return (
          <div
            key={t.id}
            className={`flex items-center gap-2 pl-3 pr-1 border-r border-border text-sm cursor-pointer min-w-0 max-w-xs ${
              isActive ? "bg-bg text-fg" : "text-muted hover:text-fg"
            }`}
            onClick={() => onSelect(t.id)}
            title={t.path}
          >
            <span className="truncate">
              {t.name}
              {t.dirty && <span className="text-accent ml-1">●</span>}
            </span>
            <button
              onClick={(e) => {
                e.stopPropagation();
                onClose(t.id);
              }}
              className="px-1.5 py-0.5 hover:bg-border rounded text-xs"
            >
              ×
            </button>
          </div>
        );
      })}
    </div>
  );
}
