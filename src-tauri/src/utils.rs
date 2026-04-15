use anyhow::Result;
use widestring::{U16CStr, U16CString};
use windows::Win32::{
    Devices::Display::{
        DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
        DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_ROTATION, DISPLAYCONFIG_ROTATION_IDENTITY,
        DISPLAYCONFIG_ROTATION_ROTATE90, DISPLAYCONFIG_ROTATION_ROTATE180,
        DISPLAYCONFIG_ROTATION_ROTATE270, DISPLAYCONFIG_SOURCE_DEVICE_NAME,
        DISPLAYCONFIG_TARGET_DEVICE_NAME, DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes,
        QDC_ONLY_ACTIVE_PATHS, QueryDisplayConfig,
    },
    Foundation::POINT,
    Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MONITORINFOEXW, MonitorFromPoint,
    },
    UI::WindowsAndMessaging::{
        GetCursorPos, SPI_SETMOUSETRAILS, SPIF_SENDCHANGE, SystemParametersInfoW,
    },
};

pub fn software_mouse() -> Result<()> {
    unsafe {
        SystemParametersInfoW(
            SPI_SETMOUSETRAILS,
            u32::MAX, /*-1*/
            None,
            SPIF_SENDCHANGE,
        )?;
    }
    Ok(())
}

pub fn hardware_mouse() -> Result<()> {
    unsafe {
        SystemParametersInfoW(SPI_SETMOUSETRAILS, 0, None, SPIF_SENDCHANGE)?;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum Orientation {
    #[serde(rename = "default")]
    Default,
    #[serde(rename = "rotate-left")]
    RotateLeft,
    #[serde(rename = "rotate-right")]
    RotateRight,
    #[serde(rename = "upside-down")]
    Rotate180,
}

impl From<DISPLAYCONFIG_ROTATION> for Orientation {
    fn from(value: DISPLAYCONFIG_ROTATION) -> Self {
        match value {
            DISPLAYCONFIG_ROTATION_IDENTITY => Self::Default,
            DISPLAYCONFIG_ROTATION_ROTATE90 => Self::RotateLeft,
            DISPLAYCONFIG_ROTATION_ROTATE270 => Self::RotateRight,
            DISPLAYCONFIG_ROTATION_ROTATE180 => Self::Rotate180,
            _ => Self::Default,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub path: U16CString,
    pub gdi_path: U16CString,
    pub name: String,
    pub orientation: Orientation,
    pub refresh_rate: f32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct MonitorInfoDto {
    pub path: String,
    pub name: String,
    pub orientation: Orientation,
    pub refresh_rate: f32,
}

impl From<MonitorInfo> for MonitorInfoDto {
    fn from(value: MonitorInfo) -> Self {
        Self {
            path: value.path.to_string_lossy(),
            name: value.name,
            orientation: value.orientation,
            refresh_rate: value.refresh_rate,
        }
    }
}

pub fn get_all_monitors() -> Result<Vec<MonitorInfo>> {
    unsafe {
        let mut monitors = Vec::new();

        let mut patharray_size = 0;
        let mut mode_info_size = 0;

        let ret = GetDisplayConfigBufferSizes(
            QDC_ONLY_ACTIVE_PATHS,
            &mut patharray_size,
            &mut mode_info_size,
        );
        if ret.is_err() {
            anyhow::bail!("GetDisplayConfigBufferSizes failed: {ret:?}");
        }
        if patharray_size == 0 {
            anyhow::bail!("GetDisplayConfigBufferSizes returned no active display paths");
        }

        let mut patharray = vec![std::mem::zeroed(); patharray_size as usize];
        let mut mode_info = vec![std::mem::zeroed(); mode_info_size as usize];

        let ret = QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut patharray_size,
            patharray.as_mut_ptr(),
            &mut mode_info_size,
            mode_info.as_mut_ptr(),
            None,
        );
        if ret.is_err() {
            anyhow::bail!("QueryDisplayConfig failed: {ret:?}");
        }

        for item in patharray.iter() {
            let target_info = item.targetInfo;
            let source_info = item.sourceInfo;

            let refresh_rate = target_info.refreshRate.Numerator as f32
                / target_info.refreshRate.Denominator as f32;
            let orientation = target_info.rotation;

            let mut req = DISPLAYCONFIG_TARGET_DEVICE_NAME {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
                    size: std::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32,
                    adapterId: target_info.adapterId,
                    id: target_info.id,
                },
                ..Default::default()
            };
            if DisplayConfigGetDeviceInfo(&mut req.header) != 0 {
                anyhow::bail!("DisplayConfigGetDeviceInfo target name failed");
            }
            let name = match U16CStr::from_slice_truncate(&req.monitorFriendlyDeviceName) {
                Ok(v) => v.to_string_lossy(),
                Err(_) => anyhow::bail!("invalid target monitor friendly name"),
            };
            let path = match U16CStr::from_slice_truncate(&req.monitorDevicePath) {
                Ok(v) => v.to_ucstring(),
                Err(_) => anyhow::bail!("invalid target monitor path"),
            };

            let mut req = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                    size: std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                    adapterId: source_info.adapterId,
                    id: source_info.id,
                },
                ..Default::default()
            };
            if DisplayConfigGetDeviceInfo(&mut req.header) != 0 {
                anyhow::bail!("DisplayConfigGetDeviceInfo source name failed");
            }
            let gdi_path = match U16CStr::from_slice_truncate(&req.viewGdiDeviceName) {
                Ok(v) => v.to_ucstring(),
                Err(_) => anyhow::bail!("invalid source GDI path"),
            };

            monitors.push(MonitorInfo {
                path,
                gdi_path,
                name,
                orientation: orientation.into(),
                refresh_rate,
            });
        }

        Ok(monitors)
    }
}

pub fn get_mouse_monitor(monitors: &[MonitorInfo]) -> Option<MonitorInfo> {
    unsafe {
        let mut pos = POINT::default();
        if GetCursorPos(&mut pos).is_err() {
            return None;
        }
        let h_monitor = MonitorFromPoint(pos, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFOEXW::default();
        mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if !GetMonitorInfoW(h_monitor, &mut mi.monitorInfo as *mut MONITORINFO).as_bool() {
            return None;
        }
        let gdi_path = match U16CStr::from_slice_truncate(&mi.szDevice) {
            Ok(v) => v,
            Err(_) => return None,
        };
        monitors
            .iter()
            .find(|item| item.gdi_path == gdi_path)
            .cloned()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_get_all_displays() {
        let monitors = get_all_monitors().unwrap();
        dbg!(&monitors);
        assert_ne!(monitors.len(), 0);
    }

    #[test]
    fn test_get_mouse_monitor() {
        let monitors = get_all_monitors().unwrap();
        let monitor = get_mouse_monitor(&monitors);
        dbg!(&monitor);
        assert!(monitor.is_some());
    }

    #[test]
    #[ignore = "changes global mouse system setting; run manually"]
    fn test_software_mouse() {
        software_mouse().unwrap();
    }

    #[test]
    #[ignore = "changes global mouse system setting; run manually"]
    fn test_hardware_mouse() {
        hardware_mouse().unwrap();
    }
}
