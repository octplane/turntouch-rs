mod config;
mod event;
mod executor;
mod protocol;

use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Manager, Peripheral};
use clap::Parser;
use futures::StreamExt;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time;
use tracing::{error, info, warn};

use crate::config::Config;
use crate::event::EventDetector;
use crate::protocol::*;

#[derive(Parser)]
#[command(name = "turntouch", about = "Turn Touch BLE remote control CLI")]
struct Cli {
    /// Path to config file (default: ~/.config/turntouch/config.toml)
    #[arg(short, long)]
    config: Option<String>,

    /// Scan timeout in seconds
    #[arg(short, long, default_value = "30")]
    timeout: u64,

    /// Create/update the .app bundle for Bluetooth permissions and exit
    #[arg(long)]
    install: bool,
}

fn app_bundle_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("TurnTouch.app")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "turntouch=info".into()),
        )
        .init();

    let cli = Cli::parse();

    if cli.install {
        install_app_bundle()?;
        return Ok(());
    }

    // On macOS, check Bluetooth authorization before proceeding.
    // Without permission, CoreBluetooth will SIGABRT the process.
    #[cfg(target_os = "macos")]
    {
        use objc2_core_bluetooth::{CBManager, CBManagerAuthorization};
        let auth = unsafe { CBManager::authorization_class() };
        if auth != CBManagerAuthorization::AllowedAlways {
            let reason = match auth {
                CBManagerAuthorization::NotDetermined => "not yet granted",
                CBManagerAuthorization::Denied => "denied",
                CBManagerAuthorization::Restricted => "restricted",
                _ => "unavailable",
            };
            error!(
                "Bluetooth permission {reason}.\n\
                 \n\
                 Option 1: Grant Bluetooth to your terminal app:\n  \
                 System Settings > Privacy & Security > Bluetooth\n\
                 \n\
                 Option 2: Run via the .app bundle:\n  \
                 turntouch --install\n  \
                 open \"{}\"",
                app_bundle_dir().display()
            );
            std::process::exit(1);
        }
    }

    let config = if let Some(path) = &cli.config {
        let content = std::fs::read_to_string(path)?;
        toml::from_str(&content)?
    } else {
        Config::load()
    };

    info!("Config path: {}", Config::config_path().display());

    let manager = Manager::new().await?;
    let adapters = manager.adapters().await?;
    let adapter = adapters
        .into_iter()
        .next()
        .ok_or("No Bluetooth adapter found")?;

    info!("Using adapter: {:?}", adapter.adapter_info().await?);

    // Outer loop: scan → connect → listen, reconnect on disconnect
    loop {
        info!("Scanning for Turn Touch remote...");

        let mut events = adapter.events().await?;
        adapter.start_scan(ScanFilter::default()).await?;

        let peripheral = loop {
            let timeout_dur = Duration::from_secs(cli.timeout);
            match time::timeout(timeout_dur, find_turn_touch(&adapter, &mut events)).await {
                Ok(Some(p)) => break p,
                Ok(None) => {
                    warn!("Event stream ended, retrying scan...");
                    continue;
                }
                Err(_) => {
                    warn!("Scan timed out after {}s, retrying...", cli.timeout);
                    // Restart scan
                    let _ = adapter.stop_scan().await;
                    adapter.start_scan(ScanFilter::default()).await?;
                    events = adapter.events().await?;
                    continue;
                }
            }
        };

        adapter.stop_scan().await?;

        let name = peripheral
            .properties()
            .await?
            .and_then(|p| p.local_name)
            .unwrap_or_else(|| "Unknown".into());
        info!("Found: {name}");

        info!("Connecting...");
        if let Err(e) = peripheral.connect().await {
            warn!("Failed to connect: {e}. Retrying...");
            time::sleep(Duration::from_secs(2)).await;
            continue;
        }
        info!("Connected. Discovering services...");
        if let Err(e) = peripheral.discover_services().await {
            warn!("Failed to discover services: {e}. Retrying...");
            let _ = peripheral.disconnect().await;
            time::sleep(Duration::from_secs(2)).await;
            continue;
        }

        let button_char = match peripheral
            .characteristics()
            .into_iter()
            .find(|c| c.uuid == BUTTON_STATUS_V2 || c.uuid == BUTTON_STATUS_V1)
        {
            Some(c) => c,
            None => {
                warn!("Button characteristic not found. Retrying...");
                let _ = peripheral.disconnect().await;
                time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        };

        let firmware = if button_char.uuid == BUTTON_STATUS_V2 {
            "V2"
        } else {
            "V1"
        };
        info!("Firmware: {firmware}. Subscribing to button notifications...");

        if let Err(e) = peripheral.subscribe(&button_char).await {
            warn!("Failed to subscribe: {e}. Retrying...");
            let _ = peripheral.disconnect().await;
            time::sleep(Duration::from_secs(2)).await;
            continue;
        }
        info!("Ready! Waiting for button presses...");

        let mut detector = EventDetector::new();
        let mut notification_stream = peripheral.notifications().await?;

        // Also listen for adapter-level disconnect events
        let mut adapter_events = adapter.events().await?;

        // Periodic connectivity check — after macOS sleep/wake, BLE streams
        // can silently hang without delivering disconnect events.
        let mut check_interval = time::interval(Duration::from_secs(10));
        check_interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

        let disconnect_reason;

        loop {
            tokio::select! {
                notification = notification_stream.next() => {
                    match notification {
                        Some(n) if n.uuid == button_char.uuid => {
                            let button_events = detector.process(&n.value);
                            for evt in button_events {
                                executor::run_command(&config, &evt);
                            }
                        }
                        Some(_) => {}
                        None => {
                            disconnect_reason = "notification stream ended";
                            break;
                        }
                    }
                }
                event = adapter_events.next() => {
                    if let Some(CentralEvent::DeviceDisconnected(id)) = event {
                        if let Ok(p) = adapter.peripheral(&id).await {
                            if let Ok(Some(props)) = p.properties().await {
                                if let Some(ref pname) = props.local_name {
                                    if pname.contains(DEVICE_NAME_PREFIX) {
                                        disconnect_reason = "disconnect event received";
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                _ = check_interval.tick() => {
                    if !peripheral.is_connected().await.unwrap_or(false) {
                        disconnect_reason = "connectivity check failed";
                        break;
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    info!("Shutting down...");
                    let _ = peripheral.disconnect().await;
                    return Ok(());
                }
            }
        }

        warn!("Connection lost ({disconnect_reason}). Reconnecting...");
        let _ = peripheral.disconnect().await;
        time::sleep(Duration::from_secs(3)).await;
    }
}

fn install_app_bundle() -> Result<(), Box<dyn std::error::Error>> {
    let exe = std::env::current_exe()?.canonicalize()?;
    let app_dir = app_bundle_dir();
    let contents = app_dir.join("Contents");
    let macos = contents.join("MacOS");

    std::fs::create_dir_all(&macos)?;

    std::fs::write(
        contents.join("Info.plist"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>Turn Touch</string>
    <key>CFBundleIdentifier</key>
    <string>com.turntouch.cli</string>
    <key>CFBundleExecutable</key>
    <string>turntouch</string>
    <key>CFBundleVersion</key>
    <string>0.1.0</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSBluetoothAlwaysUsageDescription</key>
    <string>Turn Touch needs Bluetooth to connect to your remote control.</string>
</dict>
</plist>
"#,
    )?;

    let dest = macos.join("turntouch");
    std::fs::copy(&exe, &dest)?;

    // Fix nix store dylib paths so the binary works outside nix develop.
    #[cfg(target_os = "macos")]
    {
        let otool_output = std::process::Command::new("otool")
            .args(["-L", dest.to_str().unwrap()])
            .output()?;
        let output = String::from_utf8_lossy(&otool_output.stdout);
        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("/nix/store/") {
                let nix_path = trimmed.split(' ').next().unwrap_or("");
                let lib_name = std::path::Path::new(nix_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                let system_path = format!("/usr/lib/{lib_name}");
                info!("Rewriting {nix_path} -> {system_path}");
                std::process::Command::new("install_name_tool")
                    .args(["-change", nix_path, &system_path, dest.to_str().unwrap()])
                    .status()?;
            }
        }
        // Re-sign after install_name_tool modifies the binary
        std::process::Command::new("codesign")
            .args(["--force", "-s", "-", dest.to_str().unwrap()])
            .status()?;
    }

    println!("App bundle created at: {}", app_dir.display());
    println!();
    println!("First run (to grant Bluetooth permission):");
    println!("  open \"{}\"", app_dir.display());
    println!();
    println!("After granting permission, you can also run directly:");
    println!("  {}/Contents/MacOS/turntouch", app_dir.display());

    Ok(())
}

async fn find_turn_touch(
    adapter: &btleplug::platform::Adapter,
    events: &mut (impl StreamExt<Item = CentralEvent> + Unpin),
) -> Option<Peripheral> {
    while let Some(event) = events.next().await {
        if let CentralEvent::DeviceDiscovered(id) | CentralEvent::DeviceUpdated(id) = event {
            if let Ok(peripheral) = adapter.peripheral(&id).await {
                if let Ok(Some(props)) = peripheral.properties().await {
                    if let Some(ref name) = props.local_name {
                        if name.contains(DEVICE_NAME_PREFIX) {
                            return Some(peripheral);
                        }
                    }
                }
            }
        }
    }
    None
}
