//! Synthetic mock fixtures that depend on the machine in real use: the harness roster (detection
//! needs real binaries) and the recent projects.

use harness_business::harness::Harness;
use harness_business::harnesses::{Claude, Omp, OpenCode, Pi};
use harness_business::model::{Detection, HarnessSummary, ProjectInfo, VersionSupport};

use crate::common::{DEMO_HOME, DEMO_PROJECT};

/// OMP installed and supported; PI not installed (its config left behind), with an install hint;
/// OpenCode installed by its install script and supported; Claude Code not installed and never
/// used (no config folder), with its install hint.
pub fn harness_list() -> Vec<HarnessSummary> {
    let omp = HarnessSummary {
        info: Omp.info(),
        capabilities: Omp.capabilities(),
        detection: Detection {
            installed: true,
            binary_path: Some(format!("{DEMO_HOME}/.bun/bin/omp")),
            is_batch_script: false,
            version: Some("18.6.1".to_owned()),
            version_support: VersionSupport::Supported,
            config_dir: Some(format!("{DEMO_HOME}/.omp")),
            config_dir_exists: true,
            install_hint: None,
            error: None,
        },
    };

    let pi = HarnessSummary {
        info: Pi.info(),
        capabilities: Pi.capabilities(),
        detection: Detection {
            installed: false,
            binary_path: None,
            is_batch_script: false,
            version: None,
            version_support: VersionSupport::Unknown,
            config_dir: Some(format!("{DEMO_HOME}/.pi")),
            config_dir_exists: true,
            install_hint: Some(Pi.install_hint()),
            error: None,
        },
    };

    let opencode = HarnessSummary {
        info: OpenCode::default().info(),
        capabilities: OpenCode::default().capabilities(),
        detection: Detection {
            installed: true,
            binary_path: Some(format!("{DEMO_HOME}/.opencode/bin/opencode")),
            is_batch_script: false,
            version: Some("1.19.3".to_owned()),
            version_support: VersionSupport::Supported,
            config_dir: Some(format!("{DEMO_HOME}/.config/opencode")),
            config_dir_exists: true,
            install_hint: None,
            error: None,
        },
    };

    let claude = HarnessSummary {
        info: Claude.info(),
        capabilities: Claude.capabilities(),
        detection: Detection {
            installed: false,
            binary_path: None,
            is_batch_script: false,
            version: None,
            version_support: VersionSupport::Unknown,
            config_dir: Some(format!("{DEMO_HOME}/.claude")),
            config_dir_exists: false,
            install_hint: Some(Claude.install_hint()),
            error: None,
        },
    };

    vec![omp, pi, opencode, claude]
}

/// Recent projects, most recent first: the demo repo, a plain folder and one that was deleted.
pub fn project_recent() -> Vec<ProjectInfo> {
    let recent = |path: &str, exists: bool, is_git_repo: bool| ProjectInfo {
        path: path.to_owned(),
        name: path.rsplit('/').next().unwrap_or(path).to_owned(),
        exists,
        is_git_repo,
    };

    vec![
        recent(DEMO_PROJECT, true, true),
        recent(&format!("{DEMO_HOME}/Notes/research"), true, false),
        recent(&format!("{DEMO_HOME}/Archive/legacy-api"), false, false),
    ]
}
