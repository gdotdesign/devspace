mod container;
mod config;
mod gui;
mod temp_script;

use clap::{Parser, Subcommand};
use container::Container;
use config::Config;
use log::error;

use std::path::Path;
use std::io::Write;
use std::fs;

const CONFIG_FILE: &str = ".devspace.toml";

#[derive(Parser)]
#[command(
  about = "Sandboxed development environments with UI passthrough",
  version = env!("CARGO_PKG_VERSION"),
  name = "devspace",
)]
struct Cli {
  #[command(subcommand)]
  command: Commands,
}

#[derive(Subcommand)]
enum Commands {
  /// Run a command in the container without entering
  Exec {
    /// Command to run
    #[arg(trailing_var_arg = true, required = true)]
    command: Vec<String>,

    /// Toggles verbose output (runtime commands)
    #[arg(short, long)]
    verbose: bool,
  },
  /// Remove the container
  Remove {
    #[arg(short, long)]
    verbose: bool,
  },
  /// Show the container status
  Status {
    #[arg(short, long)]
    verbose: bool,
  },
  /// Create and enter the container
  Enter {
    #[arg(short, long)]
    verbose: bool,
  },
  /// Stop the running container
  Stop {
    #[arg(short, long)]
    verbose: bool,
  },
  /// Show version information
  Version,
  /// Create a sample .devspace.toml config file
  Init,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let cli = Cli::parse();

  if let Commands::Init = cli.command {
    init_config()?;
    return Ok(());
  }

  if let Commands::Version = cli.command {
    println!("devspace {}", env!("CARGO_PKG_VERSION"));
    return Ok(());
  }

  let verbose = match &cli.command {
    Commands::Init | Commands::Version => false,
    Commands::Exec { verbose, .. } => *verbose,
    Commands::Remove { verbose } => *verbose,
    Commands::Status { verbose } => *verbose,
    Commands::Enter { verbose } => *verbose,
    Commands::Stop { verbose } => *verbose,
  };

  env_logger::Builder::new()
    .filter_level(if verbose {
      log::LevelFilter::Debug
    } else {
      log::LevelFilter::Info
    })
    .format(|buf, record| {
      use colored::Colorize;
      let prefix = match record.level() {
        log::Level::Error => "[✗]".red().to_string(),
        log::Level::Debug => "[›]".dimmed().to_string(),
        _ => "[✓]".green().to_string(),
      };
      writeln!(buf, "{} {}", prefix, record.args())
    })
    .init();

  let config = Config::load().map_err(|e| {
    error!("{}", e);
    error!("Run 'devspace init' to create a sample config");
    e
  })?;

  let container = Container::new(&config)?;

  match cli.command {
    Commands::Stop { verbose } => container.stop(verbose).map_err(|e| e.into()),
    Commands::Init | Commands::Version => unreachable!(),

    Commands::Exec { verbose, command } => {
      container.exec(verbose, &command).map_err(|e| e.into())
    }

    Commands::Remove { verbose } => {
      container.remove(verbose).map_err(|e| e.into())
    }

    Commands::Enter { verbose } => {
      container.enter(verbose)?;
      Ok(())
    }

    Commands::Status { verbose } => {
      container.status(verbose);
      Ok(())
    }
  }
}

fn init_config() -> Result<(), Box<dyn std::error::Error>> {
  if Path::new(CONFIG_FILE).exists() {
    return Err(format!("{} already exists", CONFIG_FILE).into());
  }

  fs::write(CONFIG_FILE, get_sample_config())?;
  println!("Created {}", CONFIG_FILE);
  Ok(())
}

fn get_sample_config() -> &'static str {
  r#"name = "myproject"
image = "docker.io/library/alpine:latest"
shell = zsh
gui = true

init = """
apk add --no-cache zsh git
"""
"#
}
