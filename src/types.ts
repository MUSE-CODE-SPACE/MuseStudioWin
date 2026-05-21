export type Tab = {
  id: string;
  path: string;
  name: string;
  language: string;
  content: string;
  dirty: boolean;
  /// LLMStudy 의 museedit:// 로 들어온 탭에만 들어가는 메타데이터.
  /// 사이드 패널 (LearningModePanel) 이 이걸 보고 사이드바 표시 + 진단 띄움.
  lesson?: LessonInfo;
};

export type LessonInfo = {
  slug: string;
  title?: string;
  /// 원본 강의 코드의 시작/끝 라인 (1-based). preamble/postamble 바깥의 본문 범위.
  regionStart: number;
  regionEnd: number;
  /// 환경변수 이름들. e.g. ["ANTHROPIC_API_KEY"]
  requires: string[];
  /// UI 언어. "ko" 또는 "en", 없으면 "en" default.
  lang?: "ko" | "en";
};

export type DirEntry = {
  name: string;
  path: string;
  is_dir: boolean;
};
