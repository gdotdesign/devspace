use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::env;
use std::fs;

/// RAII wrapper for temporary script files that ensures cleanup on drop.
pub struct TempScript {
  path: PathBuf,
}

impl TempScript {
  pub fn new(content: &str) -> Result<Self, String> {
    let path =
      env::temp_dir().join(format!("devspace-init-{}.sh", std::process::id()));

    fs::write(&path, format!("#!/bin/sh\n{}\n", content))
      .map_err(|e| format!("Failed to write init script: {}", e))?;

    #[cfg(unix)]
    {
      fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("Failed to set init script permissions: {}", e))?;
    }

    Ok(Self { path })
  }

  pub fn path(&self) -> &Path {
    &self.path
  }
}

impl Drop for TempScript {
  fn drop(&mut self) {
    let _ = fs::remove_file(&self.path);
  }
}
