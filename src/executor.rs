use std::process::Command;

use crate::config::Config;
use crate::protocol::ButtonEvent;

pub fn run_command(config: &Config, event: &ButtonEvent) {
    let env = Config::to_env_map(event);

    match config.command_for(event) {
        Some(cmd) => {
            tracing::info!("Event {event} -> running: {cmd}");
            match Command::new("sh").arg("-c").arg(cmd).envs(&env).spawn() {
                Ok(mut child) => {
                    // Fire and forget — don't block the BLE event loop.
                    // Log exit status in background.
                    std::thread::spawn(move || match child.wait() {
                        Ok(status) if !status.success() => {
                            tracing::warn!("Command exited with {status}");
                        }
                        Err(e) => tracing::error!("Failed to wait on command: {e}"),
                        _ => {}
                    });
                }
                Err(e) => tracing::error!("Failed to spawn command: {e}"),
            }
        }
        None => {
            tracing::info!("Event {event} (no command configured)");
        }
    }
}
