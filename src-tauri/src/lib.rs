// MuseStudio core — Tauri commands for file IO, directory listing, and code execution.
// All paths are absolute; the UI layer is responsible for path display normalization.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
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

fn run_code_impl(path: String, language: String) -> Result<RunResult, String> {
    let path_buf = PathBuf::from(&path);
    let workdir = path_buf
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let (program, args): (&str, Vec<String>) = match language.as_str() {
        "python" => ("python3", vec![path.clone()]),
        "javascript" => ("node", vec![path.clone()]),
        "typescript" => ("npx", vec!["tsx".into(), path.clone()]),
        "shell" | "bash" => ("bash", vec![path.clone()]),
        "ruby" => ("ruby", vec![path.clone()]),
        "go" => ("go", vec!["run".into(), path.clone()]),
        "rust" => ("cargo", vec!["run".into()]),
        other => return Err(format!("unsupported language: {}", other)),
    };

    let mut cmd = Command::new(program);
    cmd.args(&args).current_dir(&workdir);
    // Unix: 로그인 셸 env (PATH + API 키) 를 병합. Rust 의 Command 는 자식 env 에
    // PATH 가 설정되어 있으면 그 PATH 로 program 을 찾는다.
    #[cfg(unix)]
    cmd.envs(login_shell_env());

    let output = cmd
        .output()
        .map_err(|e| format!("spawn {}: {}", program, e))?;

    Ok(RunResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
    })
}

/// async + spawn_blocking — Tauri 2 의 동기 command 는 메인 스레드에서 실행되므로
/// LLM API 호출이 포함된 레슨 코드 (수십 초) 를 돌리는 동안 UI 전체가 얼었다.
#[tauri::command]
async fn run_code(path: String, language: String) -> Result<RunResult, String> {
    tauri::async_runtime::spawn_blocking(move || run_code_impl(path, language))
        .await
        .map_err(|e| format!("run_code task failed: {}", e))?
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
            import_bundle,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
