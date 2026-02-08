// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! System Information State Module
//! 
//! Provides real-time system metrics using the `sysinfo` crate.
//! Maintains a persistent state for accurate CPU usage calculations.

use parking_lot::Mutex;
use serde::Serialize;
use sysinfo::System;

/// System information state - keeps the sysinfo::System alive for accurate metrics
pub struct SysInfoState {
    /// The core System struct that holds all data - wrapped in Mutex for thread safety
    pub system: Mutex<System>,
}

impl SysInfoState {
    /// Create a new SysInfoState with initial system data
    pub fn new() -> Self {
        let mut sys = System::new_all();
        // Initial refresh to populate data
        sys.refresh_all();
        
        Self {
            system: Mutex::new(sys),
        }
    }
}

impl Default for SysInfoState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Response Structs - Serializable for JSON transport to frontend
// ============================================================================

/// Complete system information response
#[derive(Debug, Serialize, Clone)]
pub struct AllSystemInfo {
    pub hostname: Option<String>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub kernel_version: Option<String>,
    pub arch: Option<String>,
    pub cpu: CpuInfo,
    pub memory: MemoryInfo,
    pub swap: SwapInfo,
}

/// CPU information
#[derive(Debug, Serialize, Clone)]
pub struct CpuInfo {
    /// Number of physical CPU cores
    pub core_count: usize,
    /// CPU brand/model name
    pub brand: String,
    /// CPU vendor ID
    pub vendor_id: String,
    /// Current CPU frequency in MHz
    pub frequency_mhz: u64,
    /// Global CPU usage percentage (0-100)
    pub usage_percent: f32,
    /// Per-core usage percentages
    pub per_core_usage: Vec<f32>,
}

/// Memory information
#[derive(Debug, Serialize, Clone)]
pub struct MemoryInfo {
    /// Total physical memory in bytes
    pub total_bytes: u64,
    /// Used memory in bytes
    pub used_bytes: u64,
    /// Available/free memory in bytes
    pub available_bytes: u64,
    /// Memory usage percentage (0-100)
    pub usage_percent: f32,
    /// Total memory in GB (for display)
    pub total_gb: f64,
    /// Used memory in GB (for display)
    pub used_gb: f64,
    /// Available memory in GB (for display)
    pub available_gb: f64,
}

/// Swap/virtual memory information
#[derive(Debug, Serialize, Clone)]
pub struct SwapInfo {
    /// Total swap space in bytes
    pub total_bytes: u64,
    /// Used swap space in bytes
    pub used_bytes: u64,
    /// Free swap space in bytes
    pub free_bytes: u64,
}

// ============================================================================
// Tauri Commands - Exposed to frontend
// ============================================================================

/// Get complete system information - CPU, memory, OS details
#[tauri::command]
pub fn get_all_system_info(state: tauri::State<SysInfoState>) -> AllSystemInfo {
    tracing::info!("[sysinfo] get_all_system_info called");
    
    // Lock the mutex to get exclusive access
    let mut sys = state.system.lock();
    
    // Refresh all data to get current values
    sys.refresh_all();
    
    // Small delay for CPU usage calculation accuracy
    std::thread::sleep(std::time::Duration::from_millis(100));
    sys.refresh_cpu_usage();
    
    // Extract CPU info
    let cpus = sys.cpus();
    let cpu_info = CpuInfo {
        core_count: cpus.len(),
        brand: cpus.first().map(|c| c.brand().to_string()).unwrap_or_default(),
        vendor_id: cpus.first().map(|c| c.vendor_id().to_string()).unwrap_or_default(),
        frequency_mhz: cpus.first().map(|c| c.frequency()).unwrap_or(0),
        usage_percent: sys.global_cpu_usage(),
        per_core_usage: cpus.iter().map(|c| c.cpu_usage()).collect(),
    };
    
    // Extract memory info
    let total_mem = sys.total_memory();
    let used_mem = sys.used_memory();
    // On macOS, available_memory() may return 0, so calculate it from total - used
    let raw_available = sys.available_memory();
    let available_mem = if raw_available == 0 && total_mem > used_mem {
        total_mem - used_mem
    } else {
        raw_available
    };
    let usage_percent = if total_mem > 0 {
        (used_mem as f32 / total_mem as f32) * 100.0
    } else {
        0.0
    };
    
    let memory_info = MemoryInfo {
        total_bytes: total_mem,
        used_bytes: used_mem,
        available_bytes: available_mem,
        usage_percent,
        total_gb: total_mem as f64 / 1_073_741_824.0, // 1024^3
        used_gb: used_mem as f64 / 1_073_741_824.0,
        available_gb: available_mem as f64 / 1_073_741_824.0,
    };
    
    // Extract swap info
    let swap_info = SwapInfo {
        total_bytes: sys.total_swap(),
        used_bytes: sys.used_swap(),
        free_bytes: sys.free_swap(),
    };
    
    let result = AllSystemInfo {
        hostname: System::host_name(),
        os_name: System::name(),
        os_version: System::os_version(),
        kernel_version: System::kernel_version(),
        arch: std::env::consts::ARCH.to_string().into(),
        cpu: cpu_info,
        memory: memory_info,
        swap: swap_info,
    };
    
    tracing::info!("[sysinfo] Returning: memory={} GB, cpu_cores={}, cpu_brand={:?}",
        result.memory.total_gb,
        result.cpu.core_count,
        result.cpu.brand
    );
    
    result
}

/// Get only memory information (lighter weight)
#[tauri::command]
pub fn get_memory_info(state: tauri::State<SysInfoState>) -> MemoryInfo {
    let mut sys = state.system.lock();
    
    // Only refresh memory
    sys.refresh_memory();
    
    let total_mem = sys.total_memory();
    let used_mem = sys.used_memory();
    // On macOS, available_memory() may return 0, so calculate it from total - used
    let raw_available = sys.available_memory();
    let available_mem = if raw_available == 0 && total_mem > used_mem {
        total_mem - used_mem
    } else {
        raw_available
    };
    let usage_percent = if total_mem > 0 {
        (used_mem as f32 / total_mem as f32) * 100.0
    } else {
        0.0
    };
    
    MemoryInfo {
        total_bytes: total_mem,
        used_bytes: used_mem,
        available_bytes: available_mem,
        usage_percent,
        total_gb: total_mem as f64 / 1_073_741_824.0,
        used_gb: used_mem as f64 / 1_073_741_824.0,
        available_gb: available_mem as f64 / 1_073_741_824.0,
    }
}

/// Get only CPU information
#[tauri::command]
pub fn get_cpu_info(state: tauri::State<SysInfoState>) -> CpuInfo {
    let mut sys = state.system.lock();
    
    // Refresh CPU - need two refreshes with delay for accurate usage
    sys.refresh_cpu_usage();
    std::thread::sleep(std::time::Duration::from_millis(100));
    sys.refresh_cpu_usage();
    
    let cpus = sys.cpus();
    
    CpuInfo {
        core_count: cpus.len(),
        brand: cpus.first().map(|c| c.brand().to_string()).unwrap_or_default(),
        vendor_id: cpus.first().map(|c| c.vendor_id().to_string()).unwrap_or_default(),
        frequency_mhz: cpus.first().map(|c| c.frequency()).unwrap_or(0),
        usage_percent: sys.global_cpu_usage(),
        per_core_usage: cpus.iter().map(|c| c.cpu_usage()).collect(),
    }
}

/// Get static system information (doesn't change, no refresh needed)
#[derive(Debug, Serialize)]
pub struct StaticSystemInfo {
    pub hostname: Option<String>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub kernel_version: Option<String>,
    pub arch: String,
    pub cpu_brand: String,
    pub cpu_core_count: usize,
    pub total_memory_gb: f64,
}

#[tauri::command]
pub fn get_static_system_info(state: tauri::State<SysInfoState>) -> StaticSystemInfo {
    let sys = state.system.lock();
    let cpus = sys.cpus();
    
    StaticSystemInfo {
        hostname: System::host_name(),
        os_name: System::name(),
        os_version: System::os_version(),
        kernel_version: System::kernel_version(),
        arch: std::env::consts::ARCH.to_string(),
        cpu_brand: cpus.first().map(|c| c.brand().to_string()).unwrap_or_default(),
        cpu_core_count: cpus.len(),
        total_memory_gb: sys.total_memory() as f64 / 1_073_741_824.0,
    }
}
