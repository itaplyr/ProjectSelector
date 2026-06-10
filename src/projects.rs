use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;

#[derive(Clone, Debug)]
pub struct Commit {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub date: String,
}

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub status: String,
    pub file: String,
}

#[derive(Clone, Debug)]
pub struct GitInfo {
    pub connected: bool,
    pub branch: String,
    pub last_commit: Option<Commit>,
    pub lines_changed: i32,
    pub files_changed: i32,
    pub ahead: i32,
    pub behind: i32,
    pub modified_files: Vec<FileEntry>,
}

#[derive(Clone, Debug)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
    pub last_modified: SystemTime,
    pub git_info: GitInfo,
}

fn run_git(args: &[&str], cwd: &std::path::Path) -> Result<String, String> {
    Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| e.to_string())
        .and_then(|o| {
            if o.status.success() {
                Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                Err(String::from_utf8_lossy(&o.stderr).trim().to_string())
            }
        })
}

fn get_git_info(project_path: &std::path::Path) -> GitInfo {
    let git_dir = project_path.join(".git");
    if !git_dir.exists() {
        return GitInfo {
            connected: false,
            branch: String::new(),
            last_commit: None,
            lines_changed: 0,
            files_changed: 0,
            ahead: 0,
            behind: 0,
            modified_files: Vec::new(),
        };
    }

    let branch = run_git(&["rev-parse", "--abbrev-ref", "HEAD"], project_path).unwrap_or_default();

    let last_commit = run_git(
        &["log", "-1", "--format=%H|%s|%an|%ai"],
        project_path,
    )
    .ok()
    .and_then(|s| {
        let parts: Vec<&str> = s.splitn(4, '|').collect();
        if parts.len() == 4 {
            Some(Commit {
                hash: parts[0].to_string(),
                message: parts[1].to_string(),
                author: parts[2].to_string(),
                date: parts[3].to_string(),
            })
        } else {
            None
        }
    });

    let (lines_changed, files_changed) = {
        let mut lc = 0i32;
        let mut fc = 0i32;

        for args in [&["diff", "--shortstat"][..], &["diff", "--cached", "--shortstat"]] {
            if let Ok(s) = run_git(args, project_path) {
                if s.is_empty() {
                    continue;
                }
                if let Some(n) = s.split(',').nth(0).and_then(|p| {
                    p.trim()
                        .split(' ')
                        .next()
                        .and_then(|w| w.parse::<i32>().ok())
                }) {
                    fc += n;
                }
                for part in s.split(',') {
                    let part = part.trim();
                    if part.contains("insertion") {
                        if let Some(n) = part.split(' ').next().and_then(|w| w.parse::<i32>().ok()) {
                            lc += n;
                        }
                    }
                    if part.contains("deletion") {
                        if let Some(n) = part.split(' ').next().and_then(|w| w.parse::<i32>().ok()) {
                            lc += n;
                        }
                    }
                }
            }
        }
        (lc, fc)
    };

    let (ahead, behind) = run_git(&["rev-list", "--left-right", "--count", "HEAD...@{upstream}"], project_path)
        .ok()
        .and_then(|s| {
            let parts: Vec<&str> = s.split('\t').collect();
            if parts.len() == 2 {
                Some((
                    parts[1].trim().parse::<i32>().unwrap_or(0),
                    parts[0].trim().parse::<i32>().unwrap_or(0),
                ))
            } else {
                None
            }
        })
        .unwrap_or((0, 0));

    let modified_files = run_git(&["status", "--porcelain"], project_path)
        .ok()
        .map(|s| {
            s.lines()
                .filter(|l| !l.is_empty())
                .map(|l| FileEntry {
                    status: l[..2].trim().to_string(),
                    file: l[3..].to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    GitInfo {
        connected: true,
        branch,
        last_commit,
        lines_changed,
        files_changed,
        ahead,
        behind,
        modified_files,
    }
}

pub fn get_projects(projects_dir: &std::path::Path) -> Vec<Project> {
    let entries = match std::fs::read_dir(projects_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut projects: Vec<Project> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| {
            let path = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            let last_modified = std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .unwrap_or_else(|_| std::time::SystemTime::UNIX_EPOCH);
            let git_info = get_git_info(&path);
            Project {
                name,
                path,
                last_modified,
                git_info,
            }
        })
        .collect();

    projects.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));
    projects
}
