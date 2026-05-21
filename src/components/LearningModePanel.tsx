import { useEffect, useState } from "react";
import type { LessonInfo } from "../types";

const SITE = "https://llmstudy-production.up.railway.app";

/// MuseEdit Mac 의 LearningModeSidePanel 의 React 포트. 같은 contract:
///   - Header: "Learning Mode" + lesson title + Open lesson 링크
///   - API KEYS NEEDED: lessonInfo.requires 워크 + ✓/✗ + copy export
///   - FROM TEXTBOOK: regionStart-regionEnd 표시
///   - TIPS: 사용법 안내
///
/// Mac 과 다른 점:
///   - 환경변수 체크는 native env 직접 못 봐서 안내문만 (실제 set 여부 알 수 없음).
///     RunOutputPanel 의 진단이 실행 후 stderr 매칭으로 보완.
///   - Open lesson 링크는 https:// 로만 (Windows 의 llmstudy:// app 미설치 가정).
export function LearningModePanel({ lesson }: { lesson: LessonInfo }) {
  const isKo = lesson.lang === "ko";
  const L = (ko: string, en: string) => (isKo ? ko : en);
  const [copied, setCopied] = useState<string | null>(null);

  function copyExport(k: string) {
    navigator.clipboard?.writeText(`export ${k}=...`);
    setCopied(k);
    setTimeout(() => setCopied(null), 1500);
  }

  const lessonUrl = `${SITE}/lesson/${lesson.slug}${lesson.lang ? `?lang=${lesson.lang}` : ""}`;

  return (
    <aside className="w-[280px] shrink-0 border-l border-zinc-800 bg-zinc-950/50 p-3 overflow-auto text-zinc-200">
      {/* Header */}
      <div className="mb-3">
        <div className="text-[10px] font-semibold uppercase tracking-wider text-blue-400 mb-0.5">
          🎓 {L("학습 모드", "Learning Mode")}
        </div>
        <div className="text-xs font-semibold leading-tight">
          {lesson.title || lesson.slug}
        </div>
        <a
          href={lessonUrl}
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex items-center gap-1 mt-1 text-[11px] text-blue-400 hover:underline"
        >
          ↗ {L("레슨 열기", "Open lesson")}
        </a>
      </div>

      <div className="h-px bg-zinc-800 my-3" />

      {/* API Keys */}
      <section>
        <div className="text-[10px] font-bold uppercase tracking-wider text-zinc-500 mb-1.5">
          {L("필요한 API 키", "API KEYS NEEDED")}
        </div>
        {lesson.requires.length === 0 ? (
          <div className="text-[11px] text-zinc-400">
            {L("이 스니펫은 API 키가 필요하지 않습니다.", "This snippet doesn't require any API keys.")}
          </div>
        ) : (
          <ul className="space-y-2">
            {lesson.requires.map((k) => (
              <li key={k} className="text-[11px]">
                <div className="flex items-center gap-2">
                  <span className="text-amber-400">●</span>
                  <code className="font-mono">{k}</code>
                  <button
                    onClick={() => copyExport(k)}
                    className="ml-auto text-[10px] text-blue-400 hover:underline"
                  >
                    {copied === k ? L("복사됨!", "Copied!") : L("export 복사", "Copy export")}
                  </button>
                </div>
                <div className="text-[10px] text-zinc-500 mt-0.5 ml-4">
                  {L("키 받기: ", "Get a key: ")}
                  {k === "ANTHROPIC_API_KEY" ? "console.anthropic.com"
                    : k === "OPENAI_API_KEY" ? "platform.openai.com"
                    : L("벤더 콘솔", "your vendor console")}
                </div>
              </li>
            ))}
          </ul>
        )}
        <p className="text-[10px] text-zinc-500 mt-2">
          {L(
            "※ Windows 빌드는 env 자동 체크 미구현. 실행 후 stderr 진단을 보세요.",
            "※ Auto env check not yet on Windows. See run-time stderr diagnosis.",
          )}
        </p>
      </section>

      <div className="h-px bg-zinc-800 my-3" />

      {/* From textbook */}
      <section>
        <div className="text-[10px] font-bold uppercase tracking-wider text-zinc-500 mb-1.5">
          {L("교재 발췌 구간", "FROM TEXTBOOK")}
        </div>
        <div className="flex items-center gap-1.5 text-[11px]">
          <span className="text-blue-400">📖</span>
          <code className="font-mono">
            {L(`${lesson.regionStart}–${lesson.regionEnd} 행`, `Lines ${lesson.regionStart}–${lesson.regionEnd}`)}
          </code>
        </div>
        <p className="text-[10px] text-zinc-500 mt-1.5 leading-relaxed">
          {L(
            "이 범위 밖의 줄은 스니펫이 end-to-end 로 실행되도록 자동 추가됨 (imports · 샘플 입력 · auto-print).",
            "Lines outside this range were auto-added so the snippet runs end-to-end (imports, sample inputs, auto-print).",
          )}
        </p>
      </section>

      <div className="h-px bg-zinc-800 my-3" />

      {/* Tips */}
      <section>
        <div className="text-[10px] font-bold uppercase tracking-wider text-zinc-500 mb-1.5">
          {L("팁", "TIPS")}
        </div>
        <ul className="space-y-1 text-[11px] text-zinc-400">
          <li>
            {L("• 아무 줄이나 수정하세요. Ctrl-R 로 전체 파일 실행.",
               "• Edit any line. Ctrl-R runs the whole file.")}
          </li>
          <li>
            {L("• 결과는 아래쪽 Output 패널에 표시됩니다.",
               "• Output appears in the bottom Output panel.")}
          </li>
          <li>
            {L("• 상단의 샘플 입력값을 바꿔서 다른 케이스도 시험해 보세요.",
               "• Replace sample inputs near the top to test other cases.")}
          </li>
        </ul>
      </section>
    </aside>
  );
}

/// 컴포넌트가 사용할 수 있는 작은 헬퍼 — useEffect 안에서 첫 마운트 시 보임 처리.
export function useLessonAnnounce(lesson: LessonInfo | undefined) {
  useEffect(() => {
    if (!lesson) return;
    document.title = `${lesson.title || lesson.slug} — MuseStudio`;
  }, [lesson]);
}
