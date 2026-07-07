import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";

const PTY_ID = "main";

/// base64 → Uint8Array. PTY 출력이 raw bytes 로 오므로 xterm.write(Uint8Array)
/// 의 상태 유지 UTF-8 디코더에 그대로 넘긴다 (한글이 청크 경계에서 안 깨짐).
function b64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes;
}

/// 내장 터미널 — 실제 PTY 위의 로그인 셸 (zsh -l). pip install · python3 REPL ·
/// Ctrl+C · 방향키 히스토리가 전부 동작한다. 패널을 닫아도 unmount 하지 않고
/// display:none 으로 숨겨 세션을 유지한다 (App.tsx 쪽 책임).
export function TerminalPanel({
  visible,
  cwd,
  onClose,
  injected,
}: {
  visible: boolean;
  cwd: string | null;
  onClose: () => void;
  /// Output 패널의 "터미널에서 설치" 버튼이 넣는 명령. nonce 가 바뀔 때마다
  /// 셸 입력으로 주입해 사용자가 실행 과정을 그대로 보게 한다.
  injected: { cmd: string; nonce: number } | null;
}) {
  const holderRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  /// pty_spawn 완료를 기다리는 게이트 — 마운트 직후 injected 명령이 spawn 보다
  /// 먼저 pty_write 를 때리면 "pty not running" 으로 조용히 유실되는 레이스 방지.
  const spawnGateRef = useRef<Promise<void>>(Promise.resolve());

  useEffect(() => {
    const el = holderRef.current;
    if (!el) return;
    const term = new Terminal({
      fontSize: 12,
      fontFamily: "Menlo, Monaco, 'Courier New', monospace",
      cursorBlink: true,
      theme: { background: "#09090b", foreground: "#e4e4e7" },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(el);
    fit.fit();
    termRef.current = term;
    fitRef.current = fit;

    const unOut = listen<string>(`pty-output-${PTY_ID}`, (e) => {
      term.write(b64ToBytes(e.payload));
    });
    const unExit = listen(`pty-exit-${PTY_ID}`, () => {
      term.write(
        "\r\n\x1b[33m[셸 종료됨 — 패널을 닫았다 다시 열면 새 셸이 시작됩니다 / shell exited]\x1b[0m\r\n",
      );
    });

    spawnGateRef.current = invoke("pty_spawn", {
      id: PTY_ID,
      cols: term.cols,
      rows: term.rows,
      cwd,
    })
      .then(() => undefined)
      .catch((e) => {
        term.write(`\r\n\x1b[31mPTY spawn failed: ${e}\x1b[0m\r\n`);
      });
    const dataDisp = term.onData((d) => {
      invoke("pty_write", { id: PTY_ID, data: d }).catch(() => {});
    });
    const resizeDisp = term.onResize(({ cols, rows }) => {
      invoke("pty_resize", { id: PTY_ID, cols, rows }).catch(() => {});
    });
    const ro = new ResizeObserver(() => {
      // 숨김 (display:none) 상태에서 fit() 하면 0 크기로 리사이즈되므로 방어.
      if (el.offsetHeight > 0) fit.fit();
    });
    ro.observe(el);

    return () => {
      ro.disconnect();
      dataDisp.dispose();
      resizeDisp.dispose();
      unOut.then((f) => f());
      unExit.then((f) => f());
      invoke("pty_kill", { id: PTY_ID }).catch(() => {});
      term.dispose();
      termRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 보이게 될 때 크기 재계산 + 포커스 (키 입력이 바로 터미널로).
  useEffect(() => {
    if (visible) {
      requestAnimationFrame(() => {
        fitRef.current?.fit();
        termRef.current?.focus();
      });
    }
  }, [visible]);

  // Output 패널에서 주입된 명령 실행 (예: python3 -m pip install X).
  const lastNonce = useRef(0);
  useEffect(() => {
    if (!injected || injected.nonce === lastNonce.current) return;
    lastNonce.current = injected.nonce;
    (async () => {
      await spawnGateRef.current;
      try {
        await invoke("pty_write", { id: PTY_ID, data: injected.cmd + "\r" });
      } catch (e) {
        termRef.current?.write(`\r\n\x1b[31mcommand inject failed: ${e}\x1b[0m\r\n`);
      }
      requestAnimationFrame(() => termRef.current?.focus());
    })();
  }, [injected]);

  return (
    <div
      className="border-t border-zinc-800 bg-zinc-950 flex-col"
      style={{ height: 260, display: visible ? "flex" : "none" }}
    >
      <div className="flex items-center justify-between px-3 py-1.5 border-b border-zinc-800 text-xs shrink-0">
        <span className="text-zinc-400">Terminal</span>
        <button
          onClick={onClose}
          className="text-zinc-500 hover:text-zinc-200 text-sm leading-none px-1"
          aria-label="Close terminal"
        >
          ×
        </button>
      </div>
      <div ref={holderRef} className="flex-1 min-h-0 px-2 py-1" />
    </div>
  );
}
