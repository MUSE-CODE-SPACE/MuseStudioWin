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

#[tauri::command]
fn run_code(path: String, language: String) -> Result<RunResult, String> {
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

    let output = Command::new(program)
        .args(&args)
        .current_dir(&workdir)
        .output()
        .map_err(|e| format!("spawn {}: {}", program, e))?;

    Ok(RunResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            read_file,
            write_file,
            list_dir,
            search_files,
            run_code,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
