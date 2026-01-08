use service_manager::{
    ServiceInstallCtx, ServiceLabel, ServiceManager, ServiceStartCtx, ServiceStopCtx,
    ServiceUninstallCtx,
};
use std::ffi::OsString;
use std::path::PathBuf;
use tracing::info;

fn get_install_path() -> anyhow::Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let program_data =
            std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
        let mut path = PathBuf::from(program_data);
        path.push("HTTPDiscovery");
        if !path.exists() {
            std::fs::create_dir_all(&path)?;
        }
        path.push("http-discovery-service.exe");
        Ok(path)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(std::env::current_exe()?)
    }
}

pub fn install_service() -> anyhow::Result<()> {
    let label: ServiceLabel = "http-discovery-service".parse()?;
    let manager = <dyn ServiceManager>::native()?;

    let current_exe = std::env::current_exe()?;

    // Attempt copy logic
    let target_path = match get_install_path() {
        Ok(p) => {
            if p != current_exe {
                let _ = manager.stop(ServiceStopCtx {
                    label: label.clone(),
                });

                // Give a moment for lock release
                std::thread::sleep(std::time::Duration::from_millis(500));

                match std::fs::copy(&current_exe, &p) {
                    Ok(_) => info!("Copied binary to {:?}", p),
                    Err(e) => {
                        #[cfg(target_os = "windows")]
                        if let Some(32) = e.raw_os_error() {
                            info!("Service executable is locked (running). Service is already installed.");
                            return Ok(());
                        }

                        return Err(anyhow::anyhow!(
                            "Failed to copy binary to install location: {}",
                            e
                        ));
                    }
                }
                p
            } else {
                current_exe
            }
        }
        Err(_) => current_exe,
    };

    let args: Vec<OsString> = vec!["run".into(), "--service".into()];

    let result = manager.install(ServiceInstallCtx {
        label: label.clone(),
        program: target_path,
        args,
        contents: None,
        username: None,
        working_directory: None,
        environment: None,
        autostart: true,
    });

    match result {
        Ok(_) => {
            manager.start(ServiceStartCtx {
                label: label.clone(),
            })?;
            info!("Service 'http-discovery-service' installed and started successfully.");
            Ok(())
        }
        Err(e) => {
            let err_string = e.to_string();

            if err_string.contains("already exists") || err_string.contains("AlreadyExists") {
                info!("Service 'http-discovery-service' is already installed.");
                return Ok(());
            }

            if err_string.contains("Access is denied")
                || err_string.contains("privileged")
                || err_string.contains("Failed to copy")
            {
                #[cfg(target_os = "windows")]
                {
                    info!("Access denied. Attempting to elevate privileges...");
                    let current_exe = std::env::current_exe()?;
                    let output = std::process::Command::new("powershell")
                        .arg("-NoProfile")
                        .arg("Start-Process")
                        .arg(current_exe)
                        .arg("-ArgumentList")
                        .arg("'install'")
                        .arg("-Verb")
                        .arg("RunAs")
                        .arg("-WindowStyle")
                        .arg("Hidden")
                        .arg("-Wait")
                        .output()?;

                    if output.status.success() {
                        info!("Elevated installation process completed.");

                        Ok(())
                    } else {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        Err(anyhow::anyhow!("Elevated process failed: {}", stderr))
                    }
                }

                #[cfg(not(target_os = "windows"))]
                {
                    Err(anyhow::anyhow!("Failed to install service: Access denied. Please run this command as Administrator.\nError details: {}", e))
                }
            } else {
                Err(anyhow::Error::new(e))
            }
        }
    }
}

pub fn uninstall_service() -> anyhow::Result<()> {
    let label: ServiceLabel = "http-discovery-service".parse()?;
    let manager = <dyn ServiceManager>::native()?;

    let stop_result = manager.stop(ServiceStopCtx {
        label: label.clone(),
    });

    let uninstall_result = manager.uninstall(ServiceUninstallCtx {
        label: label.clone(),
    });

    let needs_elevation = match (&stop_result, &uninstall_result) {
        (Err(e), _) if e.to_string().contains("Access is denied") => true,
        (_, Err(e)) if e.to_string().contains("Access is denied") => true,
        _ => false,
    };

    if needs_elevation {
        #[cfg(target_os = "windows")]
        {
            info!("Access denied. Attempting to elevate privileges to uninstall...");
            let current_exe = std::env::current_exe()?;
            let output = std::process::Command::new("powershell")
                .arg("-NoProfile")
                .arg("Start-Process")
                .arg(current_exe)
                .arg("-ArgumentList")
                .arg("'uninstall'")
                .arg("-Verb")
                .arg("RunAs")
                .arg("-WindowStyle")
                .arg("Hidden")
                .arg("-Wait")
                .output()?;

            if output.status.success() {
                info!("Elevated uninstallation process completed.");
                return Ok(());
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("Elevated process failed: {}", stderr));
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            if let Err(e) = stop_result {
                return Err(anyhow::Error::new(e));
            }
            if let Err(e) = uninstall_result {
                return Err(anyhow::Error::new(e));
            }
        }
    }

    uninstall_result.map_err(anyhow::Error::new)?;

    #[cfg(target_os = "windows")]
    if let Ok(path) = get_install_path() {
        let _ = std::fs::remove_file(path);
    }

    info!("Service 'http-discovery-service' uninstalled successfully.");
    Ok(())
}
