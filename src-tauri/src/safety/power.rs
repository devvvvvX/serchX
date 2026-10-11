use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThermalState {
    Nominal,
    Fair,
    Serious,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerStatus {
    pub thermal_state: ThermalState,
    pub on_battery: bool,
    pub should_throttle: bool,
}

#[cfg(target_os = "macos")]
#[allow(deprecated, unexpected_cfgs)]
pub fn get_thermal_state() -> ThermalState {
    use cocoa::base::id;
    use objc::{msg_send, sel, sel_impl};
    unsafe {
        let process_info_class = objc::class!(NSProcessInfo);
        let process_info: id = msg_send![process_info_class, processInfo];
        let state: std::os::raw::c_long = msg_send![process_info, thermalState];
        match state {
            1 => ThermalState::Fair,
            2 => ThermalState::Serious,
            3 => ThermalState::Critical,
            _ => ThermalState::Nominal,
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn get_thermal_state() -> ThermalState {
    ThermalState::Nominal
}

pub fn check_power_conditions() -> PowerStatus {
    let thermal_state = get_thermal_state();
    let should_throttle = matches!(
        thermal_state,
        ThermalState::Serious | ThermalState::Critical
    );
    PowerStatus {
        thermal_state,
        on_battery: false,
        should_throttle,
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_thermal_state_query() {
        let state = get_thermal_state();
        assert!(matches!(
            state,
            ThermalState::Nominal
                | ThermalState::Fair
                | ThermalState::Serious
                | ThermalState::Critical
        ));
    }

    #[test]
    fn test_power_status_query() {
        let status = check_power_conditions();
        if status.thermal_state == ThermalState::Serious
            || status.thermal_state == ThermalState::Critical
        {
            assert!(status.should_throttle);
        } else {
            assert!(!status.should_throttle);
        }
    }
}
