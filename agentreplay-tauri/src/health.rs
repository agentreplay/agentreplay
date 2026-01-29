// Copyright 2025 Sushanth (https://github.com/sushanthpy)
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Background service health monitoring (Task 15)
//!
//! Monitors the health of background workers (retention, compaction, ingestion)
//! and reports status to the UI via Tauri events.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use parking_lot::RwLock;
use tauri::{AppHandle, Emitter};

/// Health status of a background service
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceHealth {
    /// Service is running normally
    Healthy,
    /// Service is degraded but functional
    Degraded,
    /// Service has failed or stopped unexpectedly
    Unhealthy,
    /// Service is not running (disabled or not started)
    Stopped,
}

/// Status report for a background service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub name: String,
    pub health: ServiceHealth,
    pub last_heartbeat_ms: Option<u64>,
    pub last_run_duration_ms: Option<u64>,
    pub error_count: u32,
    pub last_error: Option<String>,
    pub runs_completed: u64,
    pub started_at: Option<u64>,
}

impl Default for ServiceStatus {
    fn default() -> Self {
        Self {
            name: String::new(),
            health: ServiceHealth::Stopped,
            last_heartbeat_ms: None,
            last_run_duration_ms: None,
            error_count: 0,
            last_error: None,
            runs_completed: 0,
            started_at: None,
        }
    }
}

/// Overall system health report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub overall: ServiceHealth,
    pub services: HashMap<String, ServiceStatus>,
    pub timestamp_ms: u64,
}

/// Service health monitor that tracks all background services
#[derive(Clone)]
pub struct HealthMonitor {
    services: Arc<RwLock<HashMap<String, ServiceStatus>>>,
    app_handle: Option<AppHandle>,
    /// Heartbeat timeout - if no heartbeat received within this duration, service is degraded
    heartbeat_timeout: Duration,
    /// Critical timeout - if no heartbeat received within this duration, service is unhealthy
    critical_timeout: Duration,
}

impl HealthMonitor {
    /// Create a new health monitor
    pub fn new(app_handle: Option<AppHandle>) -> Self {
        Self {
            services: Arc::new(RwLock::new(HashMap::new())),
            app_handle,
            heartbeat_timeout: Duration::from_secs(120),  // 2 minutes
            critical_timeout: Duration::from_secs(300),   // 5 minutes
        }
    }

    /// Register a new service for monitoring
    pub fn register_service(&self, name: &str) {
        let mut services = self.services.write();
        let now_ms = Self::now_ms();
        
        services.insert(name.to_string(), ServiceStatus {
            name: name.to_string(),
            health: ServiceHealth::Healthy,
            started_at: Some(now_ms),
            ..Default::default()
        });
        
        tracing::info!(service = name, "Registered service for health monitoring");
    }

    /// Record a heartbeat from a service
    pub fn heartbeat(&self, service_name: &str) {
        let mut services = self.services.write();
        
        if let Some(status) = services.get_mut(service_name) {
            status.last_heartbeat_ms = Some(Self::now_ms());
            status.health = ServiceHealth::Healthy;
        } else {
            // Auto-register if not found
            drop(services);
            self.register_service(service_name);
        }
    }

    /// Record successful completion of a service run
    pub fn record_success(&self, service_name: &str, duration: Duration) {
        let mut services = self.services.write();
        
        if let Some(status) = services.get_mut(service_name) {
            status.last_heartbeat_ms = Some(Self::now_ms());
            status.last_run_duration_ms = Some(duration.as_millis() as u64);
            status.runs_completed += 1;
            status.health = ServiceHealth::Healthy;
        }
    }

    /// Record a service error
    pub fn record_error(&self, service_name: &str, error: &str) {
        let mut services = self.services.write();
        
        if let Some(status) = services.get_mut(service_name) {
            status.error_count += 1;
            status.last_error = Some(error.to_string());
            status.last_heartbeat_ms = Some(Self::now_ms());
            
            // Degrade health after errors
            if status.error_count >= 3 {
                status.health = ServiceHealth::Unhealthy;
            } else {
                status.health = ServiceHealth::Degraded;
            }
        }
        
        // Emit error event to UI
        self.emit_health_event();
    }

    /// Mark a service as stopped
    pub fn mark_stopped(&self, service_name: &str) {
        let mut services = self.services.write();
        
        if let Some(status) = services.get_mut(service_name) {
            status.health = ServiceHealth::Stopped;
            status.last_heartbeat_ms = Some(Self::now_ms());
        }
        
        self.emit_health_event();
    }

    /// Get the current system health report
    pub fn get_health(&self) -> SystemHealth {
        let now = Self::now_ms();
        let services = self.services.read();
        
        let mut statuses = services.clone();
        
        // Update health based on heartbeat timeouts
        for status in statuses.values_mut() {
            if status.health == ServiceHealth::Healthy || status.health == ServiceHealth::Degraded {
                if let Some(last_hb) = status.last_heartbeat_ms {
                    let elapsed = Duration::from_millis(now.saturating_sub(last_hb));
                    
                    if elapsed > self.critical_timeout {
                        status.health = ServiceHealth::Unhealthy;
                    } else if elapsed > self.heartbeat_timeout {
                        status.health = ServiceHealth::Degraded;
                    }
                }
            }
        }
        
        // Determine overall health
        let overall = if statuses.values().any(|s| s.health == ServiceHealth::Unhealthy) {
            ServiceHealth::Unhealthy
        } else if statuses.values().any(|s| s.health == ServiceHealth::Degraded) {
            ServiceHealth::Degraded
        } else {
            ServiceHealth::Healthy
        };
        
        SystemHealth {
            overall,
            services: statuses,
            timestamp_ms: now,
        }
    }

    /// Emit health status event to the UI
    fn emit_health_event(&self) {
        if let Some(ref app_handle) = self.app_handle {
            let health = self.get_health();
            let _ = app_handle.emit("service-health", &health);
        }
    }

    /// Start background health check loop
    pub fn start_monitoring(self: Arc<Self>) {
        tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                let health = self.get_health();
                
                // Log any unhealthy services
                for (name, status) in &health.services {
                    match status.health {
                        ServiceHealth::Unhealthy => {
                            tracing::warn!(
                                service = name.as_str(),
                                error = status.last_error.as_deref(),
                                "Service is unhealthy"
                            );
                        }
                        ServiceHealth::Degraded => {
                            tracing::info!(
                                service = name.as_str(),
                                "Service is degraded"
                            );
                        }
                        _ => {}
                    }
                }
                
                // Emit periodic health event
                self.emit_health_event();
            }
        });
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

/// Macro for easy service heartbeat with duration tracking
#[macro_export]
macro_rules! with_health_tracking {
    ($monitor:expr, $service:expr, $body:expr) => {{
        let start = std::time::Instant::now();
        let result = $body;
        match &result {
            Ok(_) => $monitor.record_success($service, start.elapsed()),
            Err(e) => $monitor.record_error($service, &e.to_string()),
        }
        result
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_monitoring() {
        let monitor = HealthMonitor::new(None);
        
        monitor.register_service("test-service");
        monitor.heartbeat("test-service");
        
        let health = monitor.get_health();
        assert_eq!(health.overall, ServiceHealth::Healthy);
        assert_eq!(health.services.get("test-service").unwrap().health, ServiceHealth::Healthy);
    }

    #[test]
    fn test_error_handling() {
        let monitor = HealthMonitor::new(None);
        
        monitor.register_service("test-service");
        monitor.record_error("test-service", "Test error");
        
        let health = monitor.get_health();
        assert_eq!(health.services.get("test-service").unwrap().health, ServiceHealth::Degraded);
        assert_eq!(health.services.get("test-service").unwrap().error_count, 1);
    }
}
