use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelTier {
    Lightweight, // <= 8 GB RAM (1.5B models)
    Standard,    // 9–18 GB RAM (3B models)
    Power,       // >= 24 GB RAM (7B models)
}

impl std::fmt::Display for ModelTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lightweight => write!(f, "Lightweight (1.5B)"),
            Self::Standard => write!(f, "Standard (3B)"),
            Self::Power => write!(f, "Power (7B)"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HardwareProfile {
    pub chip_name: String,
    pub total_memory_bytes: u64,
    pub total_memory_gb: u32,
    pub is_apple_silicon: bool,
    pub recommended_tier: ModelTier,
}

impl HardwareProfile {
    pub fn new(chip_name: String, total_memory_bytes: u64) -> Self {
        let total_memory_gb =
            ((total_memory_bytes as f64) / (1024.0 * 1024.0 * 1024.0)).round() as u32;
        let is_apple_silicon = chip_name.to_lowercase().contains("apple")
            || cfg!(target_arch = "aarch64")
            || chip_name.starts_with('M');

        let recommended_tier = Self::recommended_tier_for_memory(total_memory_gb);

        Self {
            chip_name,
            total_memory_bytes,
            total_memory_gb,
            is_apple_silicon,
            recommended_tier,
        }
    }

    pub fn recommended_tier_for_memory(memory_gb: u32) -> ModelTier {
        if memory_gb <= 8 {
            ModelTier::Lightweight
        } else if memory_gb <= 18 {
            ModelTier::Standard
        } else {
            ModelTier::Power
        }
    }

    pub fn detect() -> Self {
        #[cfg(target_os = "macos")]
        {
            let mut mem_bytes: u64 = 0;
            let mut mem_size = std::mem::size_of::<u64>();
            let mem_key = std::ffi::CString::new("hw.memsize").unwrap();
            let res = unsafe {
                libc::sysctlbyname(
                    mem_key.as_ptr(),
                    &mut mem_bytes as *mut u64 as *mut libc::c_void,
                    &mut mem_size,
                    std::ptr::null_mut(),
                    0,
                )
            };

            if res != 0 || mem_bytes == 0 {
                mem_bytes = 16 * 1024 * 1024 * 1024; // 16GB fallback
            }

            let mut cpu_buf = vec![0u8; 256];
            let mut cpu_len = cpu_buf.len();
            let cpu_key = std::ffi::CString::new("machdep.cpu.brand_string").unwrap();
            let cpu_res = unsafe {
                libc::sysctlbyname(
                    cpu_key.as_ptr(),
                    cpu_buf.as_mut_ptr() as *mut libc::c_void,
                    &mut cpu_len,
                    std::ptr::null_mut(),
                    0,
                )
            };

            let chip_name = if cpu_res == 0 && cpu_len > 0 {
                String::from_utf8_lossy(&cpu_buf[..cpu_len.saturating_sub(1)])
                    .trim()
                    .to_string()
            } else {
                "Apple Silicon".to_string()
            };

            Self::new(chip_name, mem_bytes)
        }

        #[cfg(not(target_os = "macos"))]
        {
            Self::new("Generic Processor".to_string(), 16 * 1024 * 1024 * 1024)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardware_profiler() {
        let profile = HardwareProfile::detect();
        assert!(profile.total_memory_bytes > 0);
        assert!(profile.total_memory_gb > 0);
        assert!(!profile.chip_name.is_empty());

        // Test tier mapping
        assert_eq!(
            HardwareProfile::recommended_tier_for_memory(8),
            ModelTier::Lightweight
        );
        assert_eq!(
            HardwareProfile::recommended_tier_for_memory(16),
            ModelTier::Standard
        );
        assert_eq!(
            HardwareProfile::recommended_tier_for_memory(24),
            ModelTier::Power
        );
        assert_eq!(
            HardwareProfile::recommended_tier_for_memory(32),
            ModelTier::Power
        );
        assert_eq!(
            HardwareProfile::recommended_tier_for_memory(64),
            ModelTier::Power
        );
    }
}
