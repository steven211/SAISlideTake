use crate::app_container::AppContainer;
use std::{process::Command, sync::Weak, thread, time::Duration};

pub fn start(container: Weak<AppContainer>, deck_id: String, generation: u64) {
    #[cfg(target_os = "macos")]
    thread::spawn(move || watch_macos(container, deck_id, generation));
    #[cfg(target_os = "windows")]
    thread::spawn(move || watch_windows(container, deck_id, generation));
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = (container, deck_id, generation);
}

#[cfg(target_os = "macos")]
fn watch_macos(container: Weak<AppContainer>, deck_id: String, generation: u64) {
    const SCRIPT: &str = r#"if application "Microsoft PowerPoint" is running then
tell application "Microsoft PowerPoint" to get slide index of slide of slide show view of slide show window 1
else
return 0
end if"#;
    while let Some(app) = container.upgrade() {
        if !app.watcher_is_current(&deck_id, generation) {
            break;
        }
        if let Ok(output) = Command::new("osascript").args(["-e", SCRIPT]).output() {
            if output.status.success() {
                if let Ok(position) = String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .parse::<usize>()
                {
                    if position > 0 {
                        let _ = app.set_deck_active_slide(&deck_id, position - 1);
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(350));
    }
}

#[cfg(target_os = "windows")]
fn watch_windows(container: Weak<AppContainer>, deck_id: String, generation: u64) {
    const SCRIPT: &str = r#"$ErrorActionPreference='SilentlyContinue'
while ($true) {
  try {
    $app = [Runtime.InteropServices.Marshal]::GetActiveObject('PowerPoint.Application')
    if ($app.SlideShowWindows.Count -gt 0) {
      $position = [int]$app.SlideShowWindows.Item(1).View.Slide.SlideIndex
      if ($position -gt 0) { [Console]::WriteLine($position); [Console]::Out.Flush() }
    }
  } catch {}
  Start-Sleep -Milliseconds 300
}"#;
    let mut command = Command::new("powershell.exe");
    command
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            SCRIPT,
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let Ok(mut child) = command.spawn() else {
        return;
    };
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        return;
    };
    let mut reader = std::io::BufReader::new(stdout);
    let mut line = String::new();
    while let Some(app) = container.upgrade() {
        if !app.watcher_is_current(&deck_id, generation) {
            break;
        }
        line.clear();
        match std::io::BufRead::read_line(&mut reader, &mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                if let Ok(position) = line.trim().parse::<usize>() {
                    if position > 0 {
                        let _ = app.set_deck_active_slide(&deck_id, position - 1);
                    }
                }
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}
