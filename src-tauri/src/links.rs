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
                CoCreateInstance, CoInitializeEx, CoUninitialize, IServiceProvider,
                CLSCTX_INPROC_SERVER, CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED,
                COINIT_MULTITHREADED,
            },
            Threading::{
                OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION,
            },
            Variant::VARIANT,
        },
        UI::{
            Accessibility::*,
            Shell::{
                IShellBrowser, IShellWindows, IWebBrowser2, SID_STopLevelBrowser, ShellExecuteW,
                ShellWindows,
            },
            WindowsAndMessaging::{
                FindWindowExW, GetWindowThreadProcessId, IsWindowVisible, SW_SHOWNORMAL,
            },
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

pub fn validate_link(value: &str) -> Result<String> {
    if !value.starts_with("file:") {
        return validate_url(value);
    }
    if value.len() > 32768 || value.chars().any(char::is_control) {
        return Err("This Windows path is invalid.".into());
    }
    // Reject malformed file URLs before normalization can discard a device host.
    let invalid = std::cell::Cell::new(false);
    let url = tauri::Url::options()
        .syntax_violation_callback(Some(&|_| invalid.set(true)))
        .parse(value)
        .map_err(err)?;
    if invalid.get() {
        return Err("This Windows path is invalid.".into());
    }
    let path = url
        .to_file_path()
        .map_err(|_| "Choose an absolute Windows folder path.")?;
    use std::path::{Component, Prefix};
    if !path.is_absolute()
        || !matches!(path.components().next(), Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_) | Prefix::UNC(_, _)))
        || matches!(url.host_str(), Some("." | "?"))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || path.to_string_lossy().chars().any(char::is_control)
    {
        return Err("Only absolute drive and network paths can be saved.".into());
    }
    Ok(url.to_string())
}

pub fn link_text(value: &str) -> Result<String> {
    let value = validate_link(value)?;
    let url = tauri::Url::parse(&value).map_err(err)?;
    if url.scheme() == "file" {
        return url
            .to_file_path()
            .map_err(|_| "This Windows path is invalid.")?
            .into_os_string()
            .into_string()
            .map_err(|_| "This Windows path cannot be displayed.".into());
    }
    Ok(value)
}

// Read the active Explorer tab through Shell automation, without changing focus or clipboard.
unsafe fn explorer_address(hwnd: HWND) -> Result<String> {
    let windows: IShellWindows =
        CoCreateInstance(&ShellWindows, None, CLSCTX_LOCAL_SERVER).map_err(err)?;
    let mut active_tab = None;
    let mut previous = None;
    while let Ok(tab) = FindWindowExW(Some(hwnd), previous, w!("ShellTabWindowClass"), None) {
        if IsWindowVisible(tab).as_bool() {
            active_tab = Some(tab);
            break;
        }
        previous = Some(tab);
    }
    let mut candidates = Vec::new();
    let count = windows.Count().map_err(err)?;
    for index in 0..count {
        let Ok(dispatch) = windows.Item(&VARIANT::from(index)) else {
            continue;
        };
        let Ok(browser) = dispatch.cast::<IWebBrowser2>() else {
            continue;
        };
        if browser.HWND().map_err(err)?.0 != hwnd.0 as isize {
            continue;
        }
        let tab = browser
            .cast::<IServiceProvider>()
            .and_then(|provider| provider.QueryService::<IShellBrowser>(&SID_STopLevelBrowser))
            .and_then(|browser| browser.GetWindow())
            .ok();
        if active_tab.is_some() && tab != active_tab {
            continue;
        }
        let address = browser.LocationURL().map_err(err)?.to_string();
        let url = tauri::Url::parse(&validate_link(&address)?).map_err(err)?;
        if url.scheme() != "file"
            || !url
                .to_file_path()
                .map_err(|_| "This Explorer location has no folder path.")?
                .is_dir()
        {
            return Err(
                "Open a filesystem folder in Explorer; Home and This PC cannot be saved.".into(),
            );
        }
        candidates.push(url.to_string());
    }
    if candidates.len() != 1 {
        return Err(
            "Could not read the active Explorer folder. Select a folder tab and retry Link.".into(),
        );
    }
    Ok(candidates.remove(0))
}

fn source_is_explorer(hwnd: HWND) -> Result<bool> {
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
        match path.rsplit('\\').next() {
            Some("explorer.exe") => Ok(true),
            Some("chrome.exe" | "msedge.exe" | "brave.exe") => Ok(false),
            _ => Err("Open a page in Chrome, Edge or Brave, or a folder in File Explorer, then press the Link shortcut.".into()),
        }
    }
}

fn read_address(hwnd: HWND) -> Result<(String, Option<String>)> {
    let explorer = source_is_explorer(hwnd)?;
    unsafe {
        CoInitializeEx(
            None,
            if explorer {
                COINIT_APARTMENTTHREADED
            } else {
                COINIT_MULTITHREADED
            },
        )
        .ok()
        .map_err(err)?;
        // All UI Automation interfaces are dropped before the COM apartment is released.
        let result = (|| -> Result<(String, Option<String>)> {
            if explorer {
                return explorer_address(hwnd).map(|url| (url, None));
            }
            let automation: IUIAutomation =
                CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER).map_err(err)?;
            let timeouts: IUIAutomation2 = automation.cast().map_err(err)?;
            timeouts.SetConnectionTimeout(2000).map_err(err)?;
            timeouts.SetTransactionTimeout(2000).map_err(err)?;
            let root = automation.ElementFromHandle(hwnd).map_err(err)?;
            let window_title = root.CurrentName().ok().map(|name| {
                let name = name.to_string();
                name.strip_suffix(" - Google Chrome")
                    .or_else(|| name.strip_suffix(" - Microsoft Edge"))
                    .or_else(|| name.strip_suffix(" - Brave"))
                    .unwrap_or(&name)
                    .to_string()
            });
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
                                let title = element.CurrentName().ok().map(|name| name.to_string());
                                documents.push((url, title));
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
            Ok((urls.remove(0), window_title))
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
        let result = read_address(HWND(target as *mut _)).and_then(|(url, title)| {
            let settings = state.settings.lock().unwrap().value.clone();
            let mut library = state.library.lock().unwrap();
            let item = library.commit_url(
                &url,
                title.as_deref(),
                settings.folder_for(crate::library::Kind::Url),
            )?;
            let mut warning = None;
            let export = library.export_dir(
                item.folder_id.as_deref(),
                settings.capture_folder.as_deref(),
            );
            let item = if let Some(folder) = export {
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
        let shortcuts_only = state.settings.lock().unwrap().value.shortcuts_only;
        state
            .shelf
            .lock()
            .unwrap()
            .finish(committed, error, Instant::now(), shortcuts_only);
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
pub fn rename_link(window: WebviewWindow, app: AppHandle, id: String, title: String) -> Result<()> {
    crate::desktop::content_only(&window)?;
    app.state::<Arc<AppState>>()
        .library
        .lock()
        .unwrap()
        .rename_link(&id, &title)?;
    crate::desktop::changed(&app);
    Ok(())
}

#[tauri::command]
pub fn open_link(
    window: WebviewWindow,
    state: tauri::State<Arc<AppState>>,
    id: String,
) -> Result<()> {
    crate::desktop::content_only(&window)?;
    let url = state.library.lock().unwrap().url(&id)?;
    let target = link_text(&url)?;
    let wide: Vec<u16> = target.encode_utf16().chain(Some(0)).collect();
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
        return Err("Windows could not open this link. Check that the target exists or its default app is available.".into());
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
        let (url, _) = read_address(HWND(target as *mut _)).unwrap();
        assert!(validate_url(&url).is_ok());
    }
    #[test]
    fn folder_links_validate_and_copy_as_windows_paths() {
        assert_eq!(
            link_text("file:///D:/My%20Projects/wise-warrior").unwrap(),
            r"D:\My Projects\wise-warrior"
        );
        assert_eq!(
            link_text("file://server/share/My%20Projects").unwrap(),
            r"\\server\share\My Projects"
        );
        assert_eq!(
            link_text("file://localhost/C:/folder").unwrap(),
            r"C:\folder"
        );
        for invalid in [
            "file:///relative",
            "file:///C:/folder?query=1",
            "file:///C:/folder#fragment",
            "file:///C:/folder%0Aevil",
            "file://./C:/folder",
            "file://%2e/C:/folder",
            "file://server/C:/folder",
            r"file://.\C:/folder",
            "javascript:alert(1)",
        ] {
            assert!(validate_link(invalid).is_err(), "{invalid}");
        }
        assert!(validate_url("file:///C:/folder").is_err());
    }
    #[test]
    #[ignore = "Requires a live Explorer folder and CAPTRAIN_EXPLORER_HWND"]
    fn live_explorer_folder_is_readable() {
        let target: isize = std::env::var("CAPTRAIN_EXPLORER_HWND")
            .unwrap()
            .parse()
            .unwrap();
        let (url, _) = read_address(HWND(target as *mut _)).unwrap();
        assert!(url.starts_with("file:"));
        assert!(validate_link(&url).is_ok());
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
