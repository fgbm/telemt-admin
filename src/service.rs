//! Управление systemd-сервисом telemt.

use std::process::Command;

#[derive(Debug, Clone)]
pub struct ServiceController {
    service_name: String,
}

#[derive(Debug)]
pub struct ServiceResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

impl ServiceController {
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
        }
    }

    fn run_systemctl(&self, action: &str) -> ServiceResult {
        tracing::info!(
            action = action,
            service = %self.service_name,
            "Running systemctl command"
        );
        let output = Command::new("systemctl")
            .arg(action)
            .arg(&self.service_name)
            .output();

        match output {
            Ok(o) => {
                let result = ServiceResult {
                    success: o.status.success(),
                    stdout: String::from_utf8_lossy(&o.stdout).trim().to_string(),
                    stderr: String::from_utf8_lossy(&o.stderr).trim().to_string(),
                };
                if result.success {
                    tracing::info!(
                        action = action,
                        service = %self.service_name,
                        "systemctl finished successfully"
                    );
                } else {
                    tracing::warn!(
                        action = action,
                        service = %self.service_name,
                        stderr = %result.stderr,
                        "systemctl returned non-zero status"
                    );
                }
                result
            }
            Err(e) => ServiceResult {
                success: false,
                stdout: String::new(),
                stderr: {
                    tracing::error!(
                        action = action,
                        service = %self.service_name,
                        error = %e,
                        "Failed to execute systemctl"
                    );
                    format!("Ошибка запуска systemctl: {}", e)
                },
            },
        }
    }

    pub fn start(&self) -> ServiceResult {
        self.run_systemctl("start")
    }

    pub fn stop(&self) -> ServiceResult {
        self.run_systemctl("stop")
    }

    pub fn restart(&self) -> ServiceResult {
        self.run_systemctl("restart")
    }

    pub fn reload(&self) -> ServiceResult {
        self.run_systemctl("reload")
    }

    pub fn status(&self) -> ServiceResult {
        self.run_systemctl("status")
    }

    pub fn format_result(&self, action: &str, r: &ServiceResult) -> String {
        let status = if r.success { "OK" } else { "Ошибка" };
        let mut out = format!("{} telemt: {}\n", action, status);
        if !r.stdout.is_empty() {
            out.push_str(&r.stdout);
            out.push('\n');
        }
        if !r.stderr.is_empty() {
            out.push_str(&r.stderr);
        }
        out.trim().to_string()
    }
}
