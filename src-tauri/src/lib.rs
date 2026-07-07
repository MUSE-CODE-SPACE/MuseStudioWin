// MuseStudio core — Tauri commands for file IO, directory listing, and code execution.
// All paths are absolute; the UI layer is responsible for path display normalization.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use walkdir::WalkDir;

#[derive(Serialize, Deserialize, Debug)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RunResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[tauri::command]
fn read_file(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| format!("read_file({}): {}", path, e))
}

#[tauri::command]
fn write_file(path: String, contents: String) -> Result<(), String> {
    if let Some(parent) = Path::new(&path).parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir({}): {}", parent.display(), e))?;
        }
    }
    fs::write(&path, contents).map_err(|e| format!("write_file({}): {}", path, e))
}

#[tauri::command]
fn list_dir(path: String) -> Result<Vec<DirEntry>, String> {
    let mut out: Vec<DirEntry> = Vec::new();
    let read =
        fs::read_dir(&path).map_err(|e| format!("list_dir({}): {}", path, e))?;
    for entry in read.flatten() {
        let p = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        // Skip dotfiles and node_modules / .git / target / dist by default
        if name.starts_with('.') || matches!(name.as_str(), "node_modules" | "target" | "dist") {
            continue;
        }
        out.push(DirEntry {
            name,
            path: p.to_string_lossy().to_string(),
            is_dir: p.is_dir(),
        });
    }
    out.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(out)
}

#[tauri::command]
fn search_files(root: String, query: String) -> Result<Vec<String>, String> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }
    let q = query.to_lowercase();
    let mut hits: Vec<String> = Vec::new();
    for entry in WalkDir::new(&root)
        .max_depth(8)
        .into_iter()
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            !(n.starts_with('.') || matches!(n.as_ref(), "node_modules" | "target" | "dist"))
        })
        .flatten()
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if name.contains(&q) {
            hits.push(entry.path().to_string_lossy().to_string());
            if hits.len() >= 200 {
                break;
            }
        }
    }
    Ok(hits)
}

/// GUI 로 실행된 앱 (Finder / Dock / deeplink) 은 로그인 셸 환경을 상속받지 못해
/// PATH 가 /usr/bin:/bin 수준이고 ANTHROPIC_API_KEY 같은 사용자 환경변수도 없다.
/// LLMStudy 레슨 코드는 API 키 환경변수를 전제하므로, 최초 1회 사용자의 로그인
/// 셸 (-i: ~/.zshrc 의 export 까지, -l: ~/.zprofile 의 PATH 까지) 을 돌려 env 를
/// 수집해 두었다가 run_code 자식 프로세스에 주입한다. (VS Code 와 같은 전략.)
#[cfg(unix)]
fn login_shell_env() -> &'static std::collections::HashMap<String, String> {
    use std::collections::HashMap;
    use std::process::Stdio;
    use std::sync::OnceLock;
    static CACHE: OnceLock<HashMap<String, String>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        let mut map: HashMap<String, String> = HashMap::new();
        // stdin 을 닫아 두면 rc 파일이 입력을 기다리다 멈추는 대신 즉시 실패한다.
        let out = Command::new(&shell)
            .args(["-ilc", "command env"])
            .stdin(Stdio::null())
            .output();
        if let Ok(o) = out {
            for line in String::from_utf8_lossy(&o.stdout).lines() {
                if let Some((k, v)) = line.split_once('=') {
                    // 멀티라인 값의 이어지는 줄 / 프롬프트 노이즈는 키 형식 검사로 걸러냄.
                    if !k.is_empty()
                        && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    {
                        map.insert(k.to_string(), v.to_string());
                    }
                }
            }
        }
        map
    })
}

/// Windows 는 python.org / MS Store 설치 모두 `python` 만 등록하고 `python3` 는
/// 보통 Store 리다이렉트 스텁이라 실제 실행이 안 된다. Unix 는 `python` 이
/// Python 2 를 가리킬 수 있어 `python3` 고정.
#[cfg(windows)]
const PYTHON: &str = "python";
#[cfg(not(windows))]
const PYTHON: &str = "python3";

/// 현재 실행 중인 run_code 자식 프로세스의 pid — Stop 버튼 (stop_run) 이 kill 대상
/// 을 찾기 위한 슬롯. 앱은 한 번에 하나의 실행만 허용한다 (UI 가 Run 버튼을 잠금).
static RUNNING_PID: Mutex<Option<u32>> = Mutex::new(None);

/// spawn 실패를 사용자용 메시지로 변환. 특히 NotFound (인터프리터 미설치 / PATH
/// 누락) 는 학습자가 가장 자주 만나는 케이스라 설치 안내까지 포함한다.
fn friendly_spawn_error(program: &str, e: &std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::NotFound {
        let hint = match program {
            "python3" | "python" => concat!(
                "Python 3 가 설치되어 있지 않거나 PATH 에서 찾을 수 없습니다. ",
                "https://www.python.org/downloads/ 에서 설치한 뒤 MuseStudio 를 재시작하세요.\n",
                "Python 3 is not installed or not on PATH. ",
                "Install it from python.org and restart MuseStudio."
            ),
            "node" | "npx" => concat!(
                "Node.js 가 설치되어 있지 않거나 PATH 에서 찾을 수 없습니다. ",
                "https://nodejs.org 에서 설치한 뒤 MuseStudio 를 재시작하세요.\n",
                "Node.js is not installed or not on PATH. ",
                "Install it from nodejs.org and restart MuseStudio."
            ),
            _ => concat!(
                "해당 언어의 실행기가 설치되어 있지 않거나 PATH 에서 찾을 수 없습니다. ",
                "설치 후 MuseStudio 를 재시작하세요.\n",
                "The interpreter is not installed or not on PATH. ",
                "Install it and restart MuseStudio."
            ),
        };
        format!("'{}' 을(를) 찾을 수 없습니다 / '{}' not found\n{}", program, program, hint)
    } else {
        format!("spawn {}: {}", program, e)
    }
}

fn run_code_impl(path: String, language: String) -> Result<RunResult, String> {
    let path_buf = PathBuf::from(&path);
    let workdir = path_buf
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let (program, args): (&str, Vec<String>) = match language.as_str() {
        "python" => (PYTHON, vec![path.clone()]),
        "javascript" => ("node", vec![path.clone()]),
        "typescript" => ("npx", vec!["tsx".into(), path.clone()]),
        "shell" | "bash" => ("bash", vec![path.clone()]),
        "ruby" => ("ruby", vec![path.clone()]),
        "go" => ("go", vec!["run".into(), path.clone()]),
        "rust" => ("cargo", vec!["run".into()]),
        other => return Err(format!("unsupported language: {}", other)),
    };

    let mut cmd = Command::new(program);
    cmd.args(&args)
        .current_dir(&workdir)
        // stdin 은 즉시 EOF — input() 류 대기 코드가 GUI 앱을 영원히 매달지 않고
        // EOFError 로 바로 끝나 stderr 에 원인이 보이게 한다.
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Unix: 로그인 셸 env (PATH + API 키) 를 병합. Rust 의 Command 는 자식 env 에
    // PATH 가 설정되어 있으면 그 PATH 로 program 을 찾는다.
    #[cfg(unix)]
    cmd.envs(login_shell_env());
    if language == "python" {
        // Windows 콘솔 기본 cp949 로 한글 print 가 깨지는 것 방지 + 파이프 버퍼링 해제.
        cmd.env("PYTHONIOENCODING", "utf-8").env("PYTHONUNBUFFERED", "1");
    }

    let child = cmd.spawn().map_err(|e| friendly_spawn_error(program, &e))?;
    *RUNNING_PID.lock().unwrap() = Some(child.id());
    let output = child.wait_with_output();
    *RUNNING_PID.lock().unwrap() = None;
    let output = output.map_err(|e| format!("wait {}: {}", program, e))?;

    Ok(RunResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        // 시그널로 죽은 경우 (Stop 버튼 = SIGKILL) code() 는 None → -1 로 보고.
        exit_code: output.status.code().unwrap_or(-1),
    })
}

/// Stop 버튼 — 실행 중인 자식 프로세스를 강제 종료한다. 실행 중인 것이 없으면
/// false. 종료된 프로세스의 run_code 호출은 exit_code -1 로 정상 반환된다.
fn stop_run_impl() -> Result<bool, String> {
    let pid = RUNNING_PID.lock().unwrap().take();
    let Some(pid) = pid else {
        return Ok(false);
    };
    #[cfg(unix)]
    let status = Command::new("kill")
        .args(["-9", &pid.to_string()])
        .stdin(Stdio::null())
        .status();
    #[cfg(windows)]
    let status = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status();
    status
        .map(|s| s.success())
        .map_err(|e| format!("stop_run kill({}): {}", pid, e))
}

#[tauri::command]
fn stop_run() -> Result<bool, String> {
    stop_run_impl()
}

/// async + spawn_blocking — Tauri 2 의 동기 command 는 메인 스레드에서 실행되므로
/// LLM API 호출이 포함된 레슨 코드 (수십 초) 를 돌리는 동안 UI 전체가 얼었다.
#[tauri::command]
async fn run_code(path: String, language: String) -> Result<RunResult, String> {
    tauri::async_runtime::spawn_blocking(move || run_code_impl(path, language))
        .await
        .map_err(|e| format!("run_code task failed: {}", e))?
}

// ---------------------------------------------------------------------------
// 내장 터미널 (xterm.js + PTY)
//
// pty_spawn 이 로그인 셸 (zsh -l 등) 을 실제 PTY 위에 띄우고, reader 스레드가
// 출력 바이트를 base64 로 인코딩해 `pty-output-<id>` 이벤트로 프론트에 흘린다.
// 입력 (pty_write) · 리사이즈 (pty_resize) · 종료 (pty_kill) 는 커맨드로 받는다.
// base64 인 이유: 문자열 이벤트로 보내면 UTF-8 멀티바이트 (한글) 가 read 청크
// 경계에서 잘릴 때 깨진다 — xterm.write(Uint8Array) 는 상태 유지 디코더라 안전.
// ---------------------------------------------------------------------------

struct PtySession {
    writer: Box<dyn std::io::Write + Send>,
    master: Box<dyn portable_pty::MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

static PTYS: std::sync::LazyLock<Mutex<std::collections::HashMap<String, PtySession>>> =
    std::sync::LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

#[tauri::command]
fn pty_spawn(
    app: tauri::AppHandle,
    id: String,
    cols: u16,
    rows: u16,
    cwd: Option<String>,
) -> Result<(), String> {
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};
    use tauri::Emitter;

    // 같은 id 로 이미 살아 있으면 그대로 재사용 (패널 재오픈 시 세션 유지).
    if PTYS.lock().unwrap().contains_key(&id) {
        return Ok(());
    }

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| format!("openpty: {}", e))?;

    #[cfg(unix)]
    let mut cmd = {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        let mut c = CommandBuilder::new(shell);
        // -l: 로그인 셸 — ~/.zprofile 의 PATH, ~/.zshrc 의 alias/export 로드.
        c.arg("-l");
        c
    };
    #[cfg(windows)]
    let mut cmd = CommandBuilder::new("powershell.exe");

    // run_code 와 같은 환경 — 로그인 셸 env (PATH + API 키) 주입. 터미널에서
    // pip install 한 패키지를 run_code 의 python3 가 바로 보는 것이 보장된다.
    #[cfg(unix)]
    {
        for (k, v) in login_shell_env() {
            cmd.env(k, v);
        }
        cmd.env("TERM", "xterm-256color");
        // 한글 입출력 — LANG 이 비어 있으면 C 로케일로 떨어져 IME 입력이 깨진다.
        if login_shell_env().get("LANG").map_or(true, |v| !v.contains("UTF-8")) {
            cmd.env("LANG", "en_US.UTF-8");
        }
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    cmd.cwd(cwd.unwrap_or(home));

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("pty spawn: {}", e))?;
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("pty reader: {}", e))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("pty writer: {}", e))?;

    PTYS.lock().unwrap().insert(
        id.clone(),
        PtySession { writer, master: pair.master, child },
    );

    std::thread::spawn(move || {
        use base64::Engine as _;
        use std::io::Read as _;
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&buf[..n]);
                    let _ = app.emit(&format!("pty-output-{}", id), b64);
                }
            }
        }
        PTYS.lock().unwrap().remove(&id);
        let _ = app.emit(&format!("pty-exit-{}", id), ());
    });
    Ok(())
}

#[tauri::command]
fn pty_write(id: String, data: String) -> Result<(), String> {
    let mut map = PTYS.lock().unwrap();
    let s = map.get_mut(&id).ok_or("pty not running")?;
    s.writer
        .write_all(data.as_bytes())
        .and_then(|_| s.writer.flush())
        .map_err(|e| format!("pty write: {}", e))
}

#[tauri::command]
fn pty_resize(id: String, cols: u16, rows: u16) -> Result<(), String> {
    use portable_pty::PtySize;
    let map = PTYS.lock().unwrap();
    let s = map.get(&id).ok_or("pty not running")?;
    s.master
        .resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| format!("pty resize: {}", e))
}

#[tauri::command]
fn pty_kill(id: String) -> Result<(), String> {
    if let Some(mut s) = PTYS.lock().unwrap().remove(&id) {
        let _ = s.child.kill();
    }
    Ok(())
}

#[derive(Serialize, Debug)]
pub struct ImportResult {
    /// 압축 해제된 번들 루트 (사이드바 rootDir 로 쓸 경로).
    pub root: String,
    /// 루트의 README.md 절대 경로 — 있으면 UI 가 첫 탭으로 연다.
    pub readme: Option<String>,
}

const MAX_BUNDLE_BYTES: u64 = 100 * 1024 * 1024; // 100 MB

fn sanitize_dir_name(s: &str) -> Option<String> {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn import_bundle_impl(
    url: String,
    name: Option<String>,
    slug: Option<String>,
    base_dir: PathBuf,
) -> Result<ImportResult, String> {
    // 1) Download
    let resp = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(120))
        .call()
        .map_err(|e| format!("download failed ({}): {}", url, e))?;
    let mut bytes: Vec<u8> = Vec::new();
    use std::io::Read as _;
    resp.into_reader()
        .take(MAX_BUNDLE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("download read failed: {}", e))?;
    if bytes.len() as u64 > MAX_BUNDLE_BYTES {
        return Err(format!("bundle exceeds {} MB limit", MAX_BUNDLE_BYTES / 1024 / 1024));
    }

    // 2) Destination — Documents/MuseStudio/<slug|name|url-stem>
    let dir_name = slug
        .as_deref()
        .and_then(sanitize_dir_name)
        .or_else(|| name.as_deref().and_then(sanitize_dir_name))
        .or_else(|| {
            url.rsplit('/')
                .next()
                .map(|f| f.trim_end_matches(".zip"))
                .and_then(sanitize_dir_name)
        })
        .unwrap_or_else(|| "bundle".to_string());
    let dest = base_dir.join("MuseStudio").join(&dir_name);
    fs::create_dir_all(&dest).map_err(|e| format!("mkdir({}): {}", dest.display(), e))?;

    // 3) Extract
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| format!("invalid zip: {}", e))?;
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("zip entry {}: {}", i, e))?;
        // llmstudy 번들 zip 은 UTF-8 파일명을 EFS 플래그 없이 저장 → zip crate 가
        // cp437 로 잘못 디코딩해 한글 파일명이 깨진다. raw bytes 를 UTF-8 로 먼저
        // 시도하고, 실패 시에만 crate 의 디코딩 결과를 쓴다.
        let raw_name: String = match std::str::from_utf8(file.name_raw()) {
            Ok(s) => s.to_string(),
            Err(_) => file.name().to_string(),
        };
        // zip-slip 방지 — 절대경로 · 드라이브 프리픽스 · ".." 컴포넌트 거부.
        let mut rel = PathBuf::new();
        let mut unsafe_path = false;
        for comp in Path::new(&raw_name).components() {
            match comp {
                std::path::Component::Normal(c) => rel.push(c),
                std::path::Component::CurDir => {}
                _ => {
                    unsafe_path = true;
                    break;
                }
            }
        }
        if unsafe_path || rel.as_os_str().is_empty() {
            continue;
        }
        let out_path = dest.join(rel);
        if raw_name.ends_with('/') {
            fs::create_dir_all(&out_path)
                .map_err(|e| format!("mkdir({}): {}", out_path.display(), e))?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir({}): {}", parent.display(), e))?;
        }
        let mut out_file = fs::File::create(&out_path)
            .map_err(|e| format!("create({}): {}", out_path.display(), e))?;
        std::io::copy(&mut file, &mut out_file)
            .map_err(|e| format!("write({}): {}", out_path.display(), e))?;
    }

    // 4) zip 이 단일 최상위 디렉토리 (예: "03-rag/") 를 담고 있으면 그 안쪽을 루트로.
    let mut root = dest.clone();
    if let Ok(entries) = fs::read_dir(&dest) {
        let visible: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .map(|n| !n.to_string_lossy().starts_with('.'))
                    .unwrap_or(false)
            })
            .collect();
        if visible.len() == 1 && visible[0].is_dir() {
            root = visible[0].clone();
        }
    }

    let readme = root.join("README.md");
    Ok(ImportResult {
        root: root.to_string_lossy().to_string(),
        readme: readme
            .is_file()
            .then(|| readme.to_string_lossy().to_string()),
    })
}

/// musestudio://import?url=<zip>&name=<title>&slug=<chapter> — LLMStudy 의
/// ChapterCodeBundleCard 가 보내는 챕터 코드 번들을 받아 Documents/MuseStudio/
/// 아래에 풀고 루트 경로를 돌려준다.
#[tauri::command]
async fn import_bundle(
    app: tauri::AppHandle,
    url: String,
    name: Option<String>,
    slug: Option<String>,
) -> Result<ImportResult, String> {
    if !(url.starts_with("https://")
        || url.starts_with("http://localhost")
        || url.starts_with("http://127.0.0.1"))
    {
        return Err("only https:// bundle URLs are allowed".to_string());
    }
    use tauri::Manager;
    let base_dir = app
        .path()
        .document_dir()
        .or_else(|_| app.path().home_dir())
        .map_err(|e| format!("cannot resolve documents dir: {}", e))?;
    tauri::async_runtime::spawn_blocking(move || {
        import_bundle_impl(url, name, slug, base_dir)
    })
    .await
    .map_err(|e| format!("import task failed: {}", e))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    // Windows / Linux 에서 deeplink 가 두 번째 인스턴스를 새로 띄우는 문제 방지 —
    // single-instance 가 URL 인자를 기존 인스턴스로 넘기고 (deep-link feature),
    // 기존 창에 포커스를 준다. 반드시 첫 번째로 등록해야 한다.
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
        use tauri::Manager;
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.show();
            let _ = w.set_focus();
        }
    }));
    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        // museedit:// + musestudio:// deeplink — installer 가 OS-level scheme 등록을
        // 처리하고 런타임에 들어오는 URL 은 onOpenUrl 이벤트로 JS 에 전달된다.
        .plugin(tauri_plugin_deep_link::init())
        .invoke_handler(tauri::generate_handler![
            read_file,
            write_file,
            list_dir,
            search_files,
            run_code,
            stop_run,
            import_bundle,
            pty_spawn,
            pty_write,
            pty_resize,
            pty_kill,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// run_code 시나리오 검증 — RUNNING_PID 가 전역 슬롯이므로 반드시
/// `cargo test -- --test-threads=1` 로 직렬 실행할 것.
#[cfg(test)]
mod tests {
    use super::*;

    fn write_script(name: &str, body: &str) -> String {
        let dir = std::env::temp_dir().join("musestudio_run_tests");
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        fs::write(&p, body).unwrap();
        p.to_string_lossy().to_string()
    }

    /// (a) 일반 python3 스크립트 — stdout + exit code 0.
    #[test]
    fn python_basic_stdout_and_exit_zero() {
        let p = write_script("basic.py", "print('hello musestudio')\n");
        let r = run_code_impl(p, "python".into()).unwrap();
        assert_eq!(r.exit_code, 0);
        assert!(r.stdout.contains("hello musestudio"));
        assert!(r.stderr.is_empty());
    }

    /// (a) 0 이 아닌 exit code 전달.
    #[test]
    fn python_nonzero_exit_code() {
        let p = write_script("exit3.py", "import sys\nsys.exit(3)\n");
        let r = run_code_impl(p, "python".into()).unwrap();
        assert_eq!(r.exit_code, 3);
    }

    /// (b) 인터프리터 NotFound → 설치 안내 포함한 사용자용 메시지.
    #[test]
    fn spawn_not_found_gives_friendly_message() {
        let e = std::io::Error::from(std::io::ErrorKind::NotFound);
        let msg = friendly_spawn_error(PYTHON, &e);
        assert!(msg.contains("찾을 수 없습니다"));
        assert!(msg.contains("python.org"));
        assert!(msg.contains("not installed or not on PATH"));
        // 실제 spawn 경로에서도 NotFound 가 같은 helper 를 타는지 — 존재하지 않는
        // 바이너리로 재현.
        let err = Command::new("musestudio-definitely-missing-binary")
            .stdin(Stdio::null())
            .spawn()
            .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    /// (c) 미설치 모듈 import — stderr 에 ModuleNotFoundError 가 그대로 담겨야 함.
    #[test]
    fn python_missing_module_stderr() {
        let p = write_script(
            "missing_mod.py",
            "import definitely_missing_module_musestudio_xyz\n",
        );
        let r = run_code_impl(p, "python".into()).unwrap();
        assert_ne!(r.exit_code, 0);
        assert!(
            r.stderr.contains("No module named"),
            "stderr was: {}",
            r.stderr
        );
    }

    /// (d) 무한 루프 스크립트 — stop_run 으로 중단 가능해야 하고, run_code 는
    /// exit_code -1 (SIGKILL) 로 돌아와야 한다.
    #[test]
    fn infinite_loop_can_be_stopped() {
        let p = write_script(
            "infinite.py",
            "import time\nwhile True:\n    time.sleep(0.05)\n",
        );
        let handle = std::thread::spawn(move || run_code_impl(p, "python".into()));
        // 자식 pid 가 등록될 때까지 대기 (최대 15초 — 첫 로그인 셸 env 수집 포함).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while RUNNING_PID.lock().unwrap().is_none() {
            assert!(std::time::Instant::now() < deadline, "child never started");
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        // 살짝 돌게 놔둔 뒤 중단.
        std::thread::sleep(std::time::Duration::from_millis(300));
        let stopped = stop_run_impl().unwrap();
        assert!(stopped, "stop_run reported nothing to stop");
        let r = handle.join().unwrap().unwrap();
        assert_eq!(r.exit_code, -1, "killed process should report -1");
    }

    /// (d) 실행 중이 아닐 때 stop_run 은 false.
    #[test]
    fn stop_run_without_running_process() {
        assert_eq!(stop_run_impl().unwrap(), false);
    }

    /// (e) input() 대기 — stdin 이 즉시 EOF 라 행이 걸리지 않고 EOFError 로 종료.
    #[test]
    fn python_input_does_not_hang() {
        let p = write_script("stdin.py", "name = input('your name? ')\nprint(name)\n");
        let start = std::time::Instant::now();
        let r = run_code_impl(p, "python".into()).unwrap();
        assert!(
            start.elapsed() < std::time::Duration::from_secs(20),
            "input() run took too long"
        );
        assert_ne!(r.exit_code, 0);
        assert!(r.stderr.contains("EOFError"), "stderr was: {}", r.stderr);
    }

    /// (f) 한글 stdout + 한글 파일명/경로.
    #[test]
    fn python_korean_output_and_path() {
        let dir = std::env::temp_dir()
            .join("musestudio_run_tests")
            .join("한글 폴더");
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("한글_출력.py");
        fs::write(&p, "print('안녕하세요, 뮤즈스튜디오! ✨')\n").unwrap();
        let r = run_code_impl(p.to_string_lossy().to_string(), "python".into()).unwrap();
        assert_eq!(r.exit_code, 0, "stderr: {}", r.stderr);
        assert!(
            r.stdout.contains("안녕하세요, 뮤즈스튜디오! ✨"),
            "stdout was: {:?}",
            r.stdout
        );
    }

    /// (b 보조) 지원하지 않는 언어는 명확한 에러.
    #[test]
    fn unsupported_language_error() {
        let p = write_script("x.zig", "");
        let err = run_code_impl(p, "zig".into()).unwrap_err();
        assert!(err.contains("unsupported language"));
    }
}
