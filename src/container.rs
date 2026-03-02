use crate::temp_script::TempScript;
use crate::config::Config;
use crate::gui;

use std::process::{Command, ExitStatus, Stdio};
use log::{debug, info};
use colored::Colorize;
use std::env;

const CONTAINER_INIT_SCRIPT: &str = "/tmp/devspace-init.sh";
const CONTAINER_WORKSPACE: &str = "/workspace";
const STOP_TIMEOUT_SECONDS: &str = "0";

fn detect_runtime() -> Result<String, String> {
  for cmd in &["podman", "docker"] {
    if Command::new(cmd)
      .arg("--version")
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .status()
      .is_ok()
    {
      return Ok(cmd.to_string());
    }
  }

  Err("Neither podman nor docker found in PATH".to_string())
}

pub struct Container<'a> {
  config: &'a Config,
  workdir: String,
  runtime: String,
}

impl<'a> Container<'a> {
  pub fn new(config: &'a Config) -> Result<Self, String> {
    let workdir = env::current_dir()
      .map_err(|e| format!("Failed to get current directory: {}", e))?
      .to_string_lossy()
      .to_string();

    let runtime = detect_runtime()?;

    Ok(Self {
      config,
      workdir,
      runtime,
    })
  }

  fn container_name(&self) -> String {
    format!("devspace-{}", self.config.name)
  }

  fn persist_volume_name(&self) -> String {
    format!("{}-persist", self.container_name())
  }

  fn run_command(
    &self,
    args: &[&str],
    verbose: bool,
  ) -> Result<ExitStatus, String> {
    debug!("{} {}", &self.runtime, args.join(" "));

    let stdout = if verbose {
      Stdio::inherit()
    } else {
      Stdio::null()
    };

    let stderr = if verbose {
      Stdio::inherit()
    } else {
      Stdio::null()
    };

    Command::new(&self.runtime)
      .args(args)
      .stdout(stdout)
      .stderr(stderr)
      .status()
      .map_err(|e| format!("Failed to {}: {}", args[0], e))
  }

  fn check_status(
    &self,
    status: ExitStatus,
    operation: &str,
  ) -> Result<(), String> {
    if status.success() {
      Ok(())
    } else {
      Err(format!("Failed to {}", operation))
    }
  }

  pub fn exists(&self, verbose: bool) -> bool {
    self
      .run_command(&["container", "inspect", &self.container_name()], verbose)
      .map(|s| s.success())
      .unwrap_or(false)
  }

  pub fn is_running(&self, verbose: bool) -> bool {
    let args = ["ps", "-q", "-f", &format!("name={}", self.container_name())];

    debug!("{} {}", &self.runtime, args.join(" "));

    let stderr = if verbose {
      Stdio::inherit()
    } else {
      Stdio::null()
    };

    let output = Command::new(&self.runtime)
      .args(args)
      .stderr(stderr)
      .output()
      .expect("Failed to check container status");

    !output.stdout.is_empty()
  }

  fn docker_user_args(&self) -> Vec<String> {
    if self.runtime != "docker" || !self.config.user_mapping {
      return vec![];
    }

    let uid = Command::new("id")
      .arg("-u")
      .output()
      .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
      .unwrap_or_default();

    let gid = Command::new("id")
      .arg("-g")
      .output()
      .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
      .unwrap_or_default();

    if uid.is_empty() || gid.is_empty() {
      return vec![];
    }

    let mut args = vec!["--user".to_string(), format!("{}:{}", uid, gid)];

    if let Ok(user) = env::var("USER") {
      args.extend(["-e".to_string(), format!("USER={}", user)]);
      args.extend(["-e".to_string(), format!("LOGNAME={}", user)]);
    }

    if let Ok(home) = env::var("HOME") {
      args.extend(["-e".to_string(), format!("HOME={}", home)]);
    }
    args
  }

  fn build_container_args(&self) -> Vec<String> {
    let mut args = vec![
      "create".to_string(),
      "-it".to_string(),
      "--name".to_string(),
      self.container_name(),
      "-v".to_string(),
      if self.runtime == "podman" {
        format!("{}:{}:Z", self.workdir, CONTAINER_WORKSPACE)
      } else {
        format!("{}:{}", self.workdir, CONTAINER_WORKSPACE)
      },
      "-w".to_string(),
      CONTAINER_WORKSPACE.to_string(),
    ];

    let container_home = if self.config.user_mapping {
      env::var("HOME").unwrap_or("/root".to_string())
    } else {
      "/root".to_string()
    };
    args.extend([
      "-v".to_string(),
      format!("{}:{}", self.persist_volume_name(), container_home),
    ]);

    if let Ok(term) = env::var("TERM") {
      args.extend(["-e".to_string(), format!("TERM={}", term)]);
    }

    if self.config.gui {
      gui::allow_local_connections();
      args.extend(gui::get_container_args());
    }

    if self.config.privileged {
      args.push("--privileged".to_string());
    }

    for port in &self.config.ports {
      args.extend(["-p".to_string(), port.clone()]);
    }

    if self.runtime == "podman" && self.config.user_mapping {
      args.push("--userns=keep-id".to_string());
    }

    args
  }

  fn create_container_base(&self, verbose: bool) -> Result<(), String> {
    let mut args = self.build_container_args();

    args.extend([
      self.config.image.clone(),
      "sleep".to_string(),
      "infinity".to_string(),
    ]);

    // Convert Vec<String> to Vec<&str> for command
    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    let status = self.run_command(&args_refs, verbose)?;
    self.check_status(status, "create container")
  }

  fn run_init_script(&self, init: &str, verbose: bool) -> Result<(), String> {
    let init = init.trim();
    let temp_script = TempScript::new(init)?;

    // Copy script to container
    let cp_status = self.run_command(
      &[
        "cp",
        &temp_script.path().to_string_lossy(),
        &format!("{}:{}", self.container_name(), CONTAINER_INIT_SCRIPT),
      ],
      verbose,
    )?;

    self.check_status(cp_status, "copy init script to container")?;

    // Run init script (start, run init, stop)
    info!("Running init script...");
    debug!("Init script contents:\n{}", init);

    let start_status =
      self.run_command(&["start", &self.container_name()], verbose)?;

    self.check_status(start_status, "start container")?;

    let init_status = self.run_command(
      &["exec", &self.container_name(), CONTAINER_INIT_SCRIPT],
      verbose,
    )?;

    if !init_status.success() {
      // Stop container on init failure
      let _ = self.run_command(
        &["stop", "-t", STOP_TIMEOUT_SECONDS, &self.container_name()],
        verbose,
      );
      return Err("Failed to run init script".to_string());
    }

    let stop_status = self.run_command(
      &["stop", "-t", STOP_TIMEOUT_SECONDS, &self.container_name()],
      verbose,
    )?;
    self.check_status(stop_status, "stop container")
  }

  fn home_setup_script(&self) -> Option<String> {
    if self.runtime != "docker" || !self.config.user_mapping {
      return None;
    }
    let uid = Command::new("id")
      .arg("-u")
      .output()
      .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
      .unwrap_or_default();
    let gid = Command::new("id")
      .arg("-g")
      .output()
      .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
      .unwrap_or_default();
    let user = env::var("USER").unwrap_or_default();
    let home = env::var("HOME").unwrap_or_default();

    if uid.is_empty() || gid.is_empty() || user.is_empty() || home.is_empty() {
      return None;
    }

    Some(format!(
      r#"mkdir -p '{home}'
grep -qF ':{uid}:' /etc/passwd || echo '{user}:x:{uid}:{gid}::{home}:/bin/sh' >> /etc/passwd
grep -qF ':{gid}:' /etc/group  || echo '{user}:x:{gid}:'                       >> /etc/group
chown {uid}:{gid} '{home}'"#,
      home = home,
      uid = uid,
      gid = gid,
      user = user,
    ))
  }

  fn chown_dirs_script(&self) -> Option<String> {
    if self.runtime != "docker"
      || !self.config.user_mapping
      || self.config.chown_dirs.is_empty()
    {
      return None;
    }
    let uid = Command::new("id")
      .arg("-u")
      .output()
      .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
      .unwrap_or_default();
    let gid = Command::new("id")
      .arg("-g")
      .output()
      .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
      .unwrap_or_default();
    if uid.is_empty() || gid.is_empty() {
      return None;
    }
    let lines: Vec<String> = self
      .config
      .chown_dirs
      .iter()
      .map(|dir| format!("mkdir -p '{dir}' && chown -R {uid}:{gid} '{dir}'"))
      .collect();
    Some(lines.join("\n"))
  }

  fn workspace_chown_script(&self) -> Option<String> {
    if self.runtime != "docker" || !self.config.user_mapping {
      return None;
    }
    let uid = Command::new("id")
      .arg("-u")
      .output()
      .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
      .unwrap_or_default();
    let gid = Command::new("id")
      .arg("-g")
      .output()
      .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
      .unwrap_or_default();
    if uid.is_empty() || gid.is_empty() {
      return None;
    }
    Some(format!(
      "chown -R {uid}:{gid} '{workspace}'",
      uid = uid,
      gid = gid,
      workspace = CONTAINER_WORKSPACE,
    ))
  }

  fn create(&self, verbose: bool) -> Result<(), String> {
    if self.exists(verbose) {
      return Ok(());
    }

    info!("Creating container {}...", self.container_name().bold());
    self.create_container_base(verbose)?;

    let preamble = self.home_setup_script();
    let extra_chowns = self.chown_dirs_script();
    let postamble = self.workspace_chown_script();

    let init = match (&preamble, &extra_chowns, &self.config.init, &postamble) {
      (None, None, None, None) => None,
      _ => {
        let parts: Vec<&str> = [
          preamble.as_deref(),
          self.config.init.as_deref(),
          extra_chowns.as_deref(),
          postamble.as_deref(),
        ]
        .into_iter()
        .flatten()
        .collect();
        Some(parts.join("\n"))
      }
    };

    if let Some(script) = init {
      self.run_init_script(&script, verbose)?;
    }

    Ok(())
  }

  fn ensure_running(&self, verbose: bool) -> Result<(), String> {
    self.create(verbose)?;

    if self.is_running(verbose) {
      return Ok(());
    }

    let status =
      self.run_command(&["start", &self.container_name()], verbose)?;

    self.check_status(status, "start container")
  }

  pub fn enter(&self, verbose: bool) -> Result<ExitStatus, String> {
    self.ensure_running(verbose)?;

    info!("Entering {}...", self.container_name().bold());

    let mut args = vec!["exec".to_string(), "-it".to_string()];
    let shell = self.config.shell.as_deref().unwrap_or("sh");
    let container_name = self.container_name();
    let user_args = self.docker_user_args();

    args.extend(user_args);
    args.extend([container_name.clone(), shell.to_string()]);

    debug!("{} {}", &self.runtime, args.join(" "));

    let status = Command::new(&self.runtime)
      .args(args)
      .status()
      .map_err(|e| format!("Failed to enter container: {}", e));

    self.stop(verbose)?;

    status
  }

  pub fn exec(
    &self,
    verbose: bool,
    interactive: bool,
    command: &[String],
  ) -> Result<(), String> {
    self.ensure_running(verbose)?;

    let shell = self.config.shell.as_deref().unwrap_or("sh");
    let container_name = self.container_name();
    let user_args = self.docker_user_args();
    let mut args = vec!["exec".to_string()];
    let command_str = command.join(" ");

    if interactive {
      args.push("-it".to_string());
    } else {
      args.push("-t".to_string());
    }

    args.extend(user_args);
    args.extend([
      container_name.clone(),
      shell.to_string(),
      "-i".to_string(),
      "-c".to_string(),
      command_str,
    ]);

    debug!("{} {}", &self.runtime, args.join(" "));

    let status = Command::new(&self.runtime)
      .args(&args)
      .status()
      .map_err(|e| format!("Failed to exec command: {}", e))?;

    if status.success() {
      Ok(())
    } else {
      std::process::exit(status.code().unwrap_or(1));
    }
  }

  pub fn stop(&self, verbose: bool) -> Result<(), String> {
    if !self.exists(verbose) {
      info!("Container {} does not exist", self.container_name().bold());
      return Ok(());
    }

    if !self.is_running(verbose) {
      info!("Container {} is not running", self.container_name().bold());
      return Ok(());
    }

    info!("Stopping {}...", self.container_name().bold());

    let status = self.run_command(
      &["stop", "-t", STOP_TIMEOUT_SECONDS, &self.container_name()],
      verbose,
    )?;
    self.check_status(status, "stop container")
  }

  pub fn remove(&self, verbose: bool) -> Result<(), String> {
    if !self.exists(verbose) {
      info!("Container {} does not exist", self.container_name().bold());
      return Ok(());
    }

    self.stop(verbose)?;

    info!("Removing {}...", self.container_name().bold());

    let status = self.run_command(&["rm", &self.container_name()], verbose)?;
    self.check_status(status, "remove container")?;

    info!("Removing volume {}...", self.persist_volume_name().bold());
    let _ = self.run_command(&["volume", "rm", &self.persist_volume_name()], verbose);

    Ok(())
  }

  pub fn status(&self, verbose: bool) {
    let name = self.container_name();

    if !self.exists(verbose) {
      info!("Container {} does not exist", name.bold());
    } else if self.is_running(verbose) {
      info!("Container {} is running", name.bold().green());
    } else {
      info!("Container {} is not running", name.bold().red());
    }
  }
}
