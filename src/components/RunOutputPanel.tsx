import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type RunResult = {
  stdout: string;
  stderr: string;
  exit_code: number;
};

type Diagnosis = {
  title: string;
  hint: string;
  fixCmd?: string;
};

/// stderr 의 흔한 패턴을 카테고리 매칭. 못 찾으면 null.
/// MuseEdit Mac 의 LearningModeSidePanel.diagnose() 와 같은 카탈로그.
function diagnose(stderr: string): Diagnosis | null {
  if (!stderr) return null;
  const modMatch = /No module named ['"]?([\w.]+)['"]?/.exec(stderr);
  if (modMatch) {
    // 정규식이 [\w.]+ 만 허용하므로 셸 인젝션 불가. top-level 패키지명만 설치
    // (예: "a.b" import 실패 → "a" 설치). run_code 와 같은 python3 인터프리터의
    // pip 를 쓰도록 python3 -m pip — 설치/실행 인터프리터 불일치 방지.
    const pkg = modMatch[1].split(".")[0];
    return {
      title: "모듈 누락",
      hint: `'${modMatch[1]}' 를 찾을 수 없습니다. 아래 버튼으로 내장 터미널에서 바로 설치할 수 있어요.`,
      fixCmd: `python3 -m pip install ${pkg}`,
    };
  }
  if (stderr.includes("AuthenticationError") || stderr.includes(" 401") || stderr.includes("Unauthorized")) {
    return {
      title: "API 키 인증 실패",
      hint: "ANTHROPIC_API_KEY / OPENAI_API_KEY 값을 확인 (앞뒤 공백·따옴표 조심).",
    };
  }
  if (stderr.includes("RateLimit") || stderr.includes(" 429")) {
    return {
      title: "API 호출 한도 초과",
      hint: "10~30초 후 재시도하거나, 더 저렴한 모델 (haiku / gpt-4o-mini) 로.",
    };
  }
  if (stderr.includes("APIConnectionError") || stderr.includes("getaddrinfo") || stderr.includes("ENOTFOUND")) {
    return {
      title: "네트워크 연결 실패",
      hint: "VPN / 방화벽 / DNS 확인. 회사망이면 proxy 필요할 수 있어요.",
    };
  }
  if (stderr.includes("TimeoutError") || stderr.toLowerCase().includes("timed out")) {
    return {
      title: "호출 시간 초과",
      hint: "모델·프롬프트 크기 확인 또는 streaming 사용 고려.",
    };
  }
  if (stderr.includes("JSONDecodeError")) {
    return {
      title: "JSON 파싱 실패",
      hint: "응답이 JSON 이 아닙니다. response_format / structured output 설정 확인.",
    };
  }
  if (stderr.includes("EOFError")) {
    return {
      title: "키보드 입력(stdin) 미지원",
      hint: "MuseStudio 실행 환경에는 터미널 입력이 연결되지 않아 input() 이 즉시 EOFError 로 끝납니다. input() 대신 변수에 값을 직접 할당해 실행해 보세요.",
    };
  }
  return null;
}

export function RunOutputPanel({
  path,
  language,
  visible,
  onClose,
  onBeforeRun,
  onRunInTerminal,
}: {
  path: string | null;
  language: string;
  visible: boolean;
  onClose: () => void;
  /// 실행 직전 훅 — 에디터의 dirty 내용을 디스크에 저장. 없으면 편집 후 Run 시
  /// 저장 안 된 옛 파일이 실행된다 ("아무 줄이나 수정하고 Run" 학습 루프 필수).
  onBeforeRun?: () => void | Promise<void>;
  /// 진단의 fixCmd (pip install 등) 를 내장 터미널에 넣어 실행 — 사용자가 설치
  /// 과정을 눈으로 보고 배우게 한다.
  onRunInTerminal?: (cmd: string) => void;
}) {
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<RunResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [stopped, setStopped] = useState(false);

  useEffect(() => {
    if (!path) {
      setResult(null);
      setError(null);
      setStopped(false);
    }
  }, [path]);

  async function run() {
    if (!path || running) return;
    setRunning(true);
    setError(null);
    setResult(null);
    setStopped(false);
    try {
      await onBeforeRun?.();
      const r = await invoke<RunResult>("run_code", { path, language });
      setResult(r);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setRunning(false);
    }
  }

  /// 무한 루프 / 장시간 실행 중단 — Rust 쪽 stop_run 이 자식 프로세스를 kill 하면
  /// 위 run() 의 invoke 가 exit_code -1 로 정상 반환된다.
  async function stop() {
    try {
      setStopped(true);
      await invoke<boolean>("stop_run");
    } catch (e) {
      console.error("stop_run failed:", e);
    }
  }

  if (!visible) return null;

  const diag = result ? diagnose(result.stderr) : null;
  const hasOutput = !!(result?.stdout || result?.stderr);

  return (
    <div className="border-t border-zinc-800 bg-zinc-950 flex flex-col" style={{ height: 240 }}>
      <div className="flex items-center justify-between px-3 py-1.5 border-b border-zinc-800 text-xs">
        <div className="flex items-center gap-2">
          <span className="text-zinc-400">Output</span>
          {result && (
            <span className={`px-1.5 py-0.5 rounded text-[10px] ${result.exit_code === 0 ? "bg-emerald-700/40 text-emerald-300" : "bg-red-700/40 text-red-300"}`}>
              exit {result.exit_code}
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          {running && (
            <button
              onClick={stop}
              disabled={stopped}
              className="px-2 py-0.5 rounded bg-red-700 hover:bg-red-600 disabled:opacity-50 text-white text-[11px] font-medium"
            >
              {stopped ? "Stopping…" : "■ Stop"}
            </button>
          )}
          <button
            onClick={run}
            disabled={!path || running}
            className="px-2 py-0.5 rounded bg-blue-600 hover:bg-blue-500 disabled:opacity-50 text-white text-[11px] font-medium"
          >
            {running ? "Running…" : "▶ Run"}
          </button>
          <button
            onClick={onClose}
            className="text-zinc-500 hover:text-zinc-200 text-sm leading-none px-1"
            aria-label="Close output"
          >
            ×
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-auto font-mono text-[11px] leading-relaxed">
        {!hasOutput && !error && !running && !result && (
          <div className="p-3 text-zinc-500">
            {path ? "▶ Run 을 눌러 실행 / Press ▶ Run to execute" : "파일을 먼저 열어주세요 / Open a file first"}
          </div>
        )}
        {result && !hasOutput && !running && !stopped && (
          <div className="p-3 text-zinc-500">(출력 없음 / no output)</div>
        )}
        {running && <div className="p-3 text-zinc-400">실행 중… / Executing…</div>}
        {!running && stopped && result && (
          <div className="px-3 pt-3 text-amber-400">■ 사용자가 실행을 중단했습니다 / Stopped by user</div>
        )}
        {error && (
          <div className="p-3 text-red-400 whitespace-pre-wrap">실행 실패 / Failed to run:{"\n"}{error}</div>
        )}
        {result?.stdout && (
          <pre className="p-3 text-zinc-200 whitespace-pre-wrap">{result.stdout}</pre>
        )}
        {result?.stderr && (
          <pre className="px-3 pb-3 text-red-300 whitespace-pre-wrap">{result.stderr}</pre>
        )}
        {diag && (
          <div className="mx-3 mb-3 rounded border border-amber-700/50 bg-amber-900/20 p-2.5 text-[11px]">
            <div className="text-amber-300 font-semibold mb-1">⚠ {diag.title}</div>
            <div className="text-zinc-300">{diag.hint}</div>
            {diag.fixCmd && (
              <div className="mt-2 flex items-center gap-2 flex-wrap">
                <code className="px-1.5 py-0.5 rounded bg-zinc-800 text-zinc-200">{diag.fixCmd}</code>
                {onRunInTerminal && (
                  <button
                    onClick={() => onRunInTerminal(diag.fixCmd!)}
                    className="px-2 py-0.5 rounded bg-emerald-700 hover:bg-emerald-600 text-white text-[10px] font-medium"
                  >
                    ⌨ 터미널에서 설치 / Install in terminal
                  </button>
                )}
                <button
                  onClick={() => navigator.clipboard?.writeText(diag.fixCmd!)}
                  className="text-blue-400 hover:text-blue-300 text-[10px]"
                >
                  copy
                </button>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
