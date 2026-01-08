mod discovery;
mod server;
mod service_mgmt;

use clap::{Parser, Subcommand};
use discovery::{start_discovery, DiscoveryState};
use tracing::info;
use tracing_subscriber::fmt::format::FmtSpan;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the discovery and HTTP server (default)
    Run {
        #[arg(short, long, default_value_t = 5380)]
        port: u16,

        /// Run as a Windows Service (internal use only)
        #[arg(long, hide = true)]
        service: bool,
    },
    /// Install as a system service
    Install,
    /// Uninstall the system service
    Uninstall,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_span_events(FmtSpan::CLOSE)
        .init();

    let cli = Cli::parse();

    let command = cli.command.unwrap_or(Commands::Run {
        port: 5380,
        service: false,
    });

    match command {
        Commands::Run { port, service } => {
            if service {
                #[cfg(target_os = "windows")]
                {
                    run_service(port)?;
                }
                #[cfg(not(target_os = "windows"))]
                {}
            } else {
                run_console(port)?;
            }
        }
        Commands::Install => {
            service_mgmt::install_service()?;
        }
        Commands::Uninstall => {
            service_mgmt::uninstall_service()?;
        }
    }

    Ok(())
}

fn run_console(port: u16) -> anyhow::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        info!("Starting HTTP Discovery Service (Console)...");
        run_app(port).await
    })
}

async fn run_app(port: u16) -> anyhow::Result<()> {
    let state = DiscoveryState::new();
    // Start background discovery
    start_discovery(state.clone());
    // Start HTTP server
    server::run_server(state, port).await?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn run_service(port: u16) -> anyhow::Result<()> {
    use std::sync::mpsc;
    use windows_service::{
        define_windows_service,
        service::ServiceControl,
        service_control_handler::{self, ServiceControlHandlerResult},
        service_dispatcher,
    };

    static mut SERVICE_PORT: u16 = 5380;
    unsafe {
        SERVICE_PORT = port;
    }

    define_windows_service!(ffi_service_main, my_service_main);

    fn my_service_main(_arguments: Vec<std::ffi::OsString>) {
        let (tx, rx) = mpsc::channel();

        let event_handler = move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                ServiceControl::Stop | ServiceControl::Interrogate => {
                    let _ = tx.send(());
                    ServiceControlHandlerResult::NoError
                }
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };

        let status_handle =
            match service_control_handler::register("http-discovery-service", event_handler) {
                Ok(h) => h,
                Err(e) => {
                    tracing::error!("Failed to register service control handler: {}", e);
                    return;
                }
            };

        let next_status = windows_service::service::ServiceStatus {
            service_type: windows_service::service::ServiceType::OWN_PROCESS,
            current_state: windows_service::service::ServiceState::Running,
            controls_accepted: windows_service::service::ServiceControlAccept::STOP,
            exit_code: windows_service::service::ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: std::time::Duration::default(),
            process_id: None,
        };

        if let Err(e) = status_handle.set_service_status(next_status) {
            tracing::error!("Failed to set service status: {}", e);
            return;
        }

        let port = unsafe { SERVICE_PORT };
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                tracing::error!("Failed to create runtime: {}", e);
                return;
            }
        };

        rt.block_on(async {
            info!("Starting HTTP Discovery Service (Windows Service mode)...");

            tokio::spawn(async move {
                if let Err(e) = run_app(port).await {
                    tracing::error!("App error: {}", e);
                }
            });

            let _ = tokio::task::spawn_blocking(move || {
                let _ = rx.recv();
            })
            .await;
        });

        let stop_status = windows_service::service::ServiceStatus {
            service_type: windows_service::service::ServiceType::OWN_PROCESS,
            current_state: windows_service::service::ServiceState::Stopped,
            controls_accepted: windows_service::service::ServiceControlAccept::empty(),
            exit_code: windows_service::service::ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: std::time::Duration::default(),
            process_id: None,
        };
        let _ = status_handle.set_service_status(stop_status);
    }

    service_dispatcher::start("http-discovery-service", ffi_service_main)
        .map_err(|e| anyhow::anyhow!("Service dispatcher failed: {}", e))
}
