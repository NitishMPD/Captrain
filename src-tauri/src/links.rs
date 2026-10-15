use crate::{
    library::{err, Result},
    AppState,
};
use std::{
    sync::{atomic::Ordering, Arc},
    time::Instant,
};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use windows::{
    core::{w, Interface, PCWSTR, PWSTR},
    Win32::{
        Foundation::{CloseHandle, HWND},
        System::{
            Com::{
                CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
                COINIT_MULTITHREADED,
            },
            Threading::{
                OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
        UI::{
            Accessibility::*,
            Shell::ShellExecuteW,
            WindowsAndMessaging::{GetWindowThreadProcessId, SW_SHOWNORMAL},
        },
    },
};

pub fn validate_url(value: &str) -> Result<String> {
    if value.is_empty()
        || value.len() > 32768
        || value.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return Err("The browser address is not a valid web URL.".into());
    }
    let url = tauri::Url::parse(value)
        .map_err(|_| "Only complete http:// or https:// web URLs can be saved.")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Only HTTP and HTTPS links without embedded credentials can be saved.".into());
    }
    Ok(url.to_string())
}

fn browser(hwnd: HWND) -> Result<()> {
    unsafe {
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).map_err(err)?;
        let mut path = [0u16; 32768];
        let mut len = path.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(path.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(handle);
        result.map_err(err)?;
        let path = String::from_utf16_lossy(&path[..len as usize]).to_lowercase();
        if !matches!(
            path.rsplit('\\').next(),
            Some("chrome.exe" | "msedge.exe" | "brave.exe")
        ) {
            return Err(
                "Open a page in Chrome, Edge or Brave, then press the Save link shortcut.".into(),
            );
        }
    }
    Ok(())
}

fn read_address(hwnd: HWND) -> Result<String> {
    browser(hwnd)?;
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()
            .map_err(err)?;
        // All UI Automation interfaces are dropped before the COM apartment is released.
        let result = (|| -> Result<String> {
            let automation: IUIAutomation =
                CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER).map_err(err)?;
            let timeouts: IUIAutomation2 = automation.cast().map_err(err)?;
            timeouts.SetConnectionTimeout(2000).map_err(err)?;
            timeouts.SetTransactionTimeout(2000).map_err(err)?;
            let root = automation.ElementFromHandle(hwnd).map_err(err)?;
            let walker = automation.ControlViewWalker().map_err(err)?;
            let mut pending = vec![(root, false)];
            let mut urls = Vec::new();
            let mut documents = Vec::new();
            let mut visited = 0;
            while let Some((element, in_toolbar)) = pending.pop() {
                visited += 1;
                if visited > 600 {
                    return Err("Browser controls could not be read safely. Focus the address bar and retry.".into());
                }
                let kind = element.CurrentControlType().map_err(err)?;

                // Read only document metadata; never descend into page content or editable fields.
                if kind == UIA_DocumentControlTypeId {
                    if element.CurrentIsOffscreen().map_err(err)?.as_bool() {
                        continue;
                    }
                    if let Ok(pattern) = element
                        .GetCurrentPatternAs::<IUIAutomationLegacyIAccessiblePattern>(
                            UIA_LegacyIAccessiblePatternId,
                        )
                    {
                        if let Ok(value) = pattern.CurrentValue() {
                            if let Ok(url) = validate_url(&value.to_string()) {
                                documents.push(url);
                            }
                        }
                    }
                    continue;
                }
                let in_toolbar = in_toolbar || kind == UIA_ToolBarControlTypeId;
                if kind == UIA_EditControlTypeId
                    && !element.CurrentIsPassword().map_err(err)?.as_bool()
                {
                    let id = element.CurrentAutomationId().map_err(err)?.to_string();
                    if in_toolbar
                        || matches!(id.as_str(), "addressEditBox" | "address and search bar")
                    {
                        if let Ok(pattern) = element
                            .GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
                        {
                            if let Ok(value) = pattern.CurrentValue() {
                                if let Ok(url) = validate_url(&value.to_string()) {
                                    urls.push(url);
                                }
                            }
                        }
                    }
                }
                if let Ok(mut child) = walker.GetFirstChildElement(&element) {
                    loop {
                        let next = walker.GetNextSiblingElement(&child);
                        pending.push((child, in_toolbar));
                        if pending.len() > 600 {
                            return Err(
                                "Too many browser controls. Close extra browser panels and retry."
                                    .into(),
                            );
                        }
                        match next {
                            Ok(sibling) => child = sibling,
                            Err(_) => break,
                        }
                    }
                }
            }
            if documents.len() == 1 {
                return Ok(documents.remove(0));
            }
            if documents.len() > 1 {
                return Err("Multiple browser pages are visible. Close browser side panels and retry Save link.".into());
            }
            if urls.len() != 1 {
                return Err("Could not read a unique web address. Finish loading the page, focus its address bar (Ctrl+L), then retry Save link. Internal browser pages are not supported.".into());
            }
            Ok(urls.remove(0))
        })();
        CoUninitialize();
        result
    }
}

pub fn start(app: &AppHandle) -> Result<()> {
    let state = app.state::<Arc<AppState>>().inner().clone();
    let target = crate::foreground::target();
    state
        .busy
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| "Finish the current capture, viewer or update before saving a link.")?;
    let app = app.clone();
    state.shelf.lock().unwrap().begin();
    state.timer.notify_one();
    std::thread::spawn(move || {
        let result = read_address(HWND(target as *mut _)).and_then(|url| {
            let settings = state.settings.lock().unwrap().value.clone();
            let mut library = state.library.lock().unwrap();
            let item = library.commit_url(&url, settings.active_folder.as_deref())?;
            let mut warning = None;
            let item = if let Some(folder) = settings.capture_folder {
                match library.auto_export(&item.id, &folder) {
                    Ok(exported) => exported,
                    Err(e) => {
                        warning = Some(format!(
                            "Link is saved in Captrain, but automatic save failed: {e}"
                        ));
                        item
                    }
                }
            } else {
                item
            };
            Ok((item, warning))
        });
        let (committed, error) = match result {
            Ok((item, warning)) => {
                let _ = app.emit("capture-created", &item);
                (true, warning)
            }
            Err(e) => (false, Some(e)),
        };
        state
            .shelf
            .lock()
            .unwrap()
            .finish(committed, error, Instant::now());
        state.busy.store(false, Ordering::Release);
        state.timer.notify_one();
        crate::shelf_window::apply(&app, false);
    });
    Ok(())
}

#[tauri::command]
pub fn save_link(window: WebviewWindow, app: AppHandle) -> Result<()> {
    crate::shelf_only(&window)?;
    start(&app)
}

#[tauri::command]
pub fn open_link(
    window: WebviewWindow,
    state: tauri::State<Arc<AppState>>,
    id: String,
) -> Result<()> {
    crate::desktop::content_only(&window)?;
    let url = state.library.lock().unwrap().url(&id)?;
    let wide: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(wide.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize <= 32 {
        return Err("Windows could not open this link. Check your default browser.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Requires a live Chrome/Edge/Brave page and CAPTRAIN_BROWSER_HWND"]
    fn live_browser_address_is_readable() {
        let target: isize = std::env::var("CAPTRAIN_BROWSER_HWND")
            .unwrap()
            .parse()
            .unwrap();
        let url = read_address(HWND(target as *mut _)).unwrap();
        assert!(validate_url(&url).is_ok());
    }
    #[test]
    fn accepts_only_web_urls_without_control_characters_or_credentials() {
        assert_eq!(
            validate_url("https://example.com/a?q=1#two").unwrap(),
            "https://example.com/a?q=1#two"
        );
        assert!(validate_url("http://localhost:3000/").is_ok());
        for value in [
            "example.com",
            "file:///C:/secret",
            "javascript:alert(1)",
            "https://user:pass@example.com",
            "https://example.com/\r\nIconFile=evil",
            "",
            "chrome://settings",
            "https://",
        ] {
            assert!(validate_url(value).is_err(), "{value}");
        }
    }
}
