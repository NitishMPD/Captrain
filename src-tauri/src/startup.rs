use crate::library::{err, Result};
use windows::{
    core::w,
    Win32::{Foundation::ERROR_FILE_NOT_FOUND, System::Registry::*},
};

const KEY: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const NAME: windows::core::PCWSTR = w!("Captrain");

fn command() -> Result<String> {
    Ok(format!(
        "\"{}\"",
        std::env::current_exe().map_err(err)?.display()
    ))
}

#[tauri::command]
pub fn get_startup(window: tauri::WebviewWindow) -> Result<bool> {
    crate::shelf_only(&window)?;
    let mut value = [0u16; 32768];
    let mut size = (value.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            KEY,
            NAME,
            RRF_RT_REG_SZ,
            None,
            Some(value.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(false);
    }
    status.ok().map_err(err)?;
    let value = String::from_utf16_lossy(&value[..(size as usize / 2).saturating_sub(1)]);
    Ok(value == command()?)
}

#[tauri::command]
pub fn set_startup(window: tauri::WebviewWindow, enabled: bool) -> Result<bool> {
    crate::shelf_only(&window)?;
    let status = if enabled {
        let value: Vec<u16> = command()?.encode_utf16().chain(Some(0)).collect();
        unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                KEY,
                NAME,
                REG_SZ.0,
                Some(value.as_ptr().cast()),
                (value.len() * 2) as u32,
            )
        }
    } else {
        unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, KEY, NAME) }
    };
    if status != ERROR_FILE_NOT_FOUND {
        status.ok().map_err(err)?;
    }
    get_startup(window)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_command_quotes_executable_without_shell() {
        let value = command().unwrap();
        assert!(value.starts_with('"') && value.ends_with('"'));
        assert_eq!(
            &value[1..value.len() - 1],
            std::env::current_exe().unwrap().to_string_lossy()
        );
    }
}
