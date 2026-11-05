use std::process::{Child, Command, Stdio};

use crate::error::{Error, Result};
use crate::manifest::InstalledApp;

pub struct Launcher;

impl Launcher {
    pub fn launch(app: &InstalledApp, extra_args: &[&str]) -> Result<Child> {
        if !app.is_valid() {
            return Err(Error::ExecutableNotFound(app.executable_path.clone()));
        }

        let mut cmd = Command::new(&app.executable_path);
        cmd.current_dir(&app.install_dir);
        cmd.args(extra_args);

        // Detach stdio so launcher doesn't block or clutter with child stdout/stderr
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        #[cfg(unix)]
        {
            // Ensure permissions
            use std::fs;
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = fs::metadata(&app.executable_path) {
                let mut perms = metadata.permissions();
                if perms.mode() & 0o111 == 0 {
                    perms.set_mode(0o755);
                    let _ = fs::set_permissions(&app.executable_path, perms);
                }
            }
        }

        cmd.spawn().map_err(|e| {
            Error::Launch(format!(
                "failed to spawn {}: {e}",
                app.executable_path.display()
            ))
        })
    }
}
