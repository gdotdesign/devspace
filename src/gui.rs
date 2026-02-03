use std::process::Command;
use std::env;
use std::fs;

const CONTAINER_XDG_RUNTIME_DIR: &str = "/tmp";

const DEFAULT_XDG_RUNTIME_DIR: &str = "/run/user/1000";
const DEFAULT_WAYLAND_DISPLAY: &str = "wayland-0";
const DEFAULT_X11_DISPLAY: &str = ":0";
const DEFAULT_X11_HOME: &str = "/root";

pub fn allow_local_connections() {
  if env::var("DISPLAY").is_ok() {
    let _ = Command::new("xhost")
      .arg("+local:")
      .stdout(std::process::Stdio::null())
      .stderr(std::process::Stdio::null())
      .status();
  }
}

pub fn get_container_args() -> Vec<String> {
  let has_wayland = env::var("WAYLAND_DISPLAY").is_ok();
  let has_x11 = env::var("DISPLAY").is_ok();

  let mut args = vec![];

  if has_wayland {
    args.extend(wayland_args());
  }
  if has_x11 {
    args.extend(x11_args());
  }

  // Add GPU device access if available
  if std::path::Path::new("/dev/dri").exists() {
    args.extend(["--device".to_string(), "/dev/dri".to_string()]);
  }

  args
}

fn hostname() -> String {
  fs::read_to_string("/etc/hostname")
    .map(|s| s.trim().to_string())
    .unwrap_or_else(|_| "localhost".to_string())
}

fn x11_args() -> Vec<String> {
  let display =
    env::var("DISPLAY").unwrap_or_else(|_| DEFAULT_X11_DISPLAY.to_string());
  let home = env::var("HOME").unwrap_or_else(|_| DEFAULT_X11_HOME.to_string());
  let xauthority =
    env::var("XAUTHORITY").unwrap_or_else(|_| format!("{}/.Xauthority", home));

  vec![
    "--hostname".to_string(),
    hostname(),
    "--ipc=host".to_string(),
    "-e".to_string(),
    format!("DISPLAY={}", display),
    "-e".to_string(),
    "XAUTHORITY=/root/.Xauthority".to_string(),
    "-v".to_string(),
    "/tmp/.X11-unix:/tmp/.X11-unix:ro".to_string(),
    "-v".to_string(),
    format!("{}:/root/.Xauthority:ro", xauthority),
  ]
}

fn wayland_args() -> Vec<String> {
  let wayland_display = env::var("WAYLAND_DISPLAY")
    .unwrap_or_else(|_| DEFAULT_WAYLAND_DISPLAY.to_string());
  let xdg_runtime_dir = env::var("XDG_RUNTIME_DIR")
    .unwrap_or_else(|_| DEFAULT_XDG_RUNTIME_DIR.to_string());

  vec![
    "--ipc=host".to_string(),
    "-e".to_string(),
    format!("WAYLAND_DISPLAY={}", wayland_display),
    "-e".to_string(),
    format!("XDG_RUNTIME_DIR={}", CONTAINER_XDG_RUNTIME_DIR),
    "-v".to_string(),
    format!("{}:{}", xdg_runtime_dir, CONTAINER_XDG_RUNTIME_DIR),
  ]
}
