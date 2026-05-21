import type { LessonInfo } from "../types";

/// museedit:// URL 의 한 occurrence 를 파싱.
/// LLMStudy 의 RunInMuseEdit.tsx 가 만드는 contract 와 동일.
/// URL 형태:
///   museedit://open?lang=py&code=<base64>&run=1
///                 &learnSlug=...&learnTitle=...&learnRegion=N-M
///                 &learnRequires=A,B&learnLang=ko
export type MuseEditDeepLink = {
  /// LANG_TO_EXT 결과 (py / js / ts / sh / rb / php / swift / go / rs)
  ext: string;
  /// monaco-editor 가 인식하는 언어 이름 (python / javascript / ...)
  language: string;
  /// base64 디코딩 + UTF-8 변환된 본문 코드 (preamble + 본문 + postamble 모두 포함)
  code: string;
  /// run=1 또는 run=true 이면 import 직후 자동 실행
  autoRun: boolean;
  /// Learning Mode 메타 — 없으면 undefined (그냥 코드 snippet 으로 열기)
  lesson?: LessonInfo;
};

const EXT_TO_LANGUAGE: Record<string, string> = {
  py: "python",
  js: "javascript",
  ts: "typescript",
  sh: "shell",
  zsh: "shell",
  rb: "ruby",
  php: "php",
  swift: "swift",
  go: "go",
  rs: "rust",
};

function decodeUrlSafeBase64(s: string): string {
  // LLMStudy 가 표준 btoa 결과를 encodeURIComponent 한 뒤 보냄. URL 파싱 시 그 단계는
  // 자동 디코딩되어 표준 base64 ('+' 와 '/' 포함) 가 들어옴. 안전 위해 - / _ 도 변환.
  const normalized = s.replace(/-/g, "+").replace(/_/g, "/");
  const padding = normalized.length % 4 === 0 ? "" : "=".repeat(4 - (normalized.length % 4));
  const bin = atob(normalized + padding);
  // base64 가 디코드한 binary string → UTF-8 텍스트 복원
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return new TextDecoder().decode(bytes);
}

export function parseMuseEditUrl(raw: string): MuseEditDeepLink | null {
  if (!raw.startsWith("museedit://")) return null;
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    return null;
  }
  if (url.host && url.host !== "open" && url.pathname !== "//open") {
    // museedit://open?... — host 가 "open" 인 경우만 허용.
    // 다른 host 형태 (예: museedit://lesson/...) 는 향후 확장.
    if (url.host !== "open") return null;
  }
  const params = url.searchParams;
  const codeParam = params.get("code");
  if (!codeParam) return null;
  let code: string;
  try {
    code = decodeUrlSafeBase64(codeParam);
  } catch {
    return null;
  }
  const ext = (params.get("lang") || "").toLowerCase();
  const language = EXT_TO_LANGUAGE[ext] || "plaintext";
  const run = params.get("run");
  const autoRun = run === "1" || run === "true";

  // Learning Mode 파라미터 — learnSlug 있으면 LessonInfo 박음.
  const slug = params.get("learnSlug");
  let lesson: LessonInfo | undefined;
  if (slug) {
    const region = params.get("learnRegion") || "";
    const [s, e] = region.split("-").map((x) => Number(x) || 0);
    const requires = (params.get("learnRequires") || "")
      .split(",").map((x) => x.trim()).filter(Boolean);
    const langRaw = (params.get("learnLang") || "").toLowerCase();
    const lang = langRaw === "ko" ? "ko" : "en";
    lesson = {
      slug,
      title: params.get("learnTitle") || undefined,
      regionStart: s || 1,
      regionEnd: e || 1,
      requires,
      lang,
    };
  }

  return { ext, language, code, autoRun, lesson };
}
