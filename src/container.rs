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

  fn create(&self, verbose: bool) -> Result<(), String> {
    if self.exists(verbose) {
      return Ok(());
    }

    info!("Creating container {}...", self.container_name().bold());
    self.create_container_base(verbose)?;

    if let Some(init) = &self.config.init {
      self.run_init_script(init, verbose)?;
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

    let shell = self.config.shell.as_deref().unwrap_or("sh");
    let args = ["exec", "-it", &self.container_name(), shell];

    debug!("{} {}", &self.runtime, args.join(" "));
    Command::new(&self.runtime)
      .args(args)
      .status()
      .map_err(|e| format!("Failed to enter container: {}", e))
  }

  pub fn exec(&self, verbose: bool, command: &[String]) -> Result<(), String> {
    self.ensure_running(verbose)?;

    let args = ["exec", "-it", &self.container_name()];
    let full_args: Vec<String> = args
      .iter()
      .map(|s| s.to_string())
      .chain(command.iter().cloned())
      .collect();
    debug!("{} {}", &self.runtime, full_args.join(" "));

    let status = Command::new(&self.runtime)
      .args(args)
      .args(command)
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
    self.check_status(status, "remove container")
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
