//! Hidden end-to-end runs of the shell binaries (a hidden window, no audio device, no real input). They need a GPU
//! adapter like the binaries themselves.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("terraria-app-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Runs a binary hidden and muted with a fixed step; returns (exit code, stdout). Killed after 120 s.
fn run(exe: &str, envs: &[(&str, &Path)], flags: &[(&str, &str)]) -> (i32, String) {
    let mut cmd = Command::new(exe);
    cmd.env("TERRARIA_HIDDEN", "1").env("TERRARIA_MUTE", "1").env("TERRARIA_FIXED_DT", "1").stdout(Stdio::piped()).stderr(Stdio::null());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    for (k, v) in flags {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().expect("spawn");
    let start = Instant::now();
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if start.elapsed() > Duration::from_secs(120) {
            let _ = child.kill();
            panic!("{exe} did not finish in 120 s");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let out = child.wait_with_output().unwrap();
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned())
}

/// `GetData` on render targets mid-frame returns exactly what was drawn, and a screenshot of the same draws does not
/// depend on whether the reads happened.
#[test]
fn readback_is_exact_and_does_not_disturb_rendering() {
    let dir = scratch("readback");
    let (a, b) = (dir.join("read.png"), dir.join("noread.png"));
    let exe = env!("CARGO_BIN_EXE_readback_demo");
    let (code, out) = run(exe, &[("TERRARIA_SCREENSHOT", &a)], &[("TERRARIA_SCREENSHOT_FRAME", "20")]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("8 checks, 0 failed"), "{out}");
    let (code, _) = run(exe, &[("TERRARIA_SCREENSHOT", &b)], &[("TERRARIA_SCREENSHOT_FRAME", "20"), ("READBACK_DEMO_SKIP", "1")]);
    assert_eq!(code, 0);
    let (pa, pb) = (std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    assert!(pa.len() > 100 && pa == pb, "screenshots differ with and without the reads");
}

/// The scripted stand-in game: keys, mouse, text, resize, F11, F12 (game-side capture), screenshot, quit.
#[test]
fn scripted_shell_demo_runs_and_captures() {
    let dir = scratch("shell_demo");
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts").join("shell_demo.input");
    let shot = dir.join("shot.png");
    let save = dir.join("save");
    let (code, out) = run(env!("CARGO_BIN_EXE_shell_demo"), &[("TERRARIA_INPUT_SCRIPT", &script), ("TERRARIA_SCREENSHOT", &shot), ("TERRARIA_SAVE_DIR", &save)], &[]);
    assert_eq!(code, 0, "{out}");
    for needle in ["click at (400, 300): SoundEffect.Play -> true", "F11: borderless", "F11: windowed 1024x576", "F12: 1024x576 written", "UnloadContent"] {
        assert!(out.contains(needle), "missing {needle:?} in:\n{out}");
    }
    let capture = std::fs::read(save.join("Captures").join("ShellDemo_001.png")).expect("the game-side capture");
    assert_eq!(&capture[1..4], b"PNG");
    assert_eq!(std::fs::read(&shot).unwrap()[1..4], *b"PNG");
}
