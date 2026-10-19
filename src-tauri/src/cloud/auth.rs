use crate::library::{atomic_write, err, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{LocalFree, HLOCAL},
        Security::Cryptography::{
            CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
        UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
    },
};

const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const SCOPE: &str = "https://www.googleapis.com/auth/drive.appdata openid email";

#[derive(Clone, Deserialize, Serialize)]
pub struct DesktopClient {
    client_id: String,
    client_secret: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Credentials {
    pub client: DesktopClient,
    pub refresh_token: Option<String>,
    pub email: Option<String>,
}

#[derive(Deserialize)]
struct Tokens {
    access_token: String,
    refresh_token: Option<String>,
    scope: Option<String>,
}

pub fn parse_client(bytes: &[u8]) -> Result<DesktopClient> {
    #[derive(Deserialize)]
    struct Download {
        installed: DesktopClient,
    }
    let client = serde_json::from_slice::<Download>(bytes)
        .map_err(|_| "Choose the Desktop app OAuth JSON downloaded from Google Cloud.")?
        .installed;
    if !client.client_id.ends_with(".apps.googleusercontent.com")
        || client.client_id.len() > 256
        || client.client_secret.is_empty()
        || client.client_secret.len() > 256
        || client.client_id.chars().any(char::is_whitespace)
    {
        return Err("Invalid Google Desktop app OAuth configuration.".into());
    }
    Ok(client)
}

fn protect(bytes: &[u8], encrypt: bool) -> Result<Vec<u8>> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len().try_into().map_err(err)?,
        pbData: bytes.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        if encrypt {
            CryptProtectData(&input, w!("Captrain Google Drive"), None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut output)
        } else {
            CryptUnprotectData(&input, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut output)
        }.map_err(|_| "Windows could not unlock the cloud credentials. Reimport the OAuth configuration on this Windows account.".to_string())?;
        let result = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
        Ok(result)
    }
}

pub fn load(root: &Path) -> Result<Option<Credentials>> {
    match fs::read(root.join("cloud-auth.bin")) {
        Ok(bytes) => serde_json::from_slice(&protect(&bytes, false)?)
            .map(Some)
            .map_err(|_| "Cloud credentials are damaged. Reimport the OAuth configuration.".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(err(e)),
    }
}

pub fn store(root: &Path, credentials: &Credentials) -> Result<()> {
    atomic_write(
        &root.join("cloud-auth.bin"),
        &protect(&serde_json::to_vec(credentials).map_err(err)?, true)?,
    )
}

pub fn http() -> Result<Client> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(err)
}

pub fn checked(response: reqwest::blocking::Response) -> Result<reqwest::blocking::Response> {
    match response.status().as_u16() {
        200..=299 => Ok(response),
        401 => Err("Google authorization expired. Reconnect Google Drive.".into()),
        403 => Err("Google Drive refused this request. Check available storage, Drive API access and app permissions, then retry.".into()),
        429 => Err("Google Drive is busy. Backup will retry automatically.".into()),
        code => Err(format!("Google Drive request failed (HTTP {code}). Retry or reconnect your account.")),
    }
}

fn token_response(response: reqwest::blocking::Response) -> Result<Tokens> {
    if !response.status().is_success() {
        return Err("Google sign-in could not be renewed. Reconnect Google Drive; check that the app is published or your account is an OAuth test user.".into());
    }
    response
        .json()
        .map_err(|_| "Google returned an invalid sign-in response.".into())
}

pub fn access(client: &Client, credentials: &mut Credentials, root: &Path) -> Result<String> {
    let refresh = credentials
        .refresh_token
        .as_deref()
        .ok_or("Connect Google Drive first.")?;
    let token = token_response(
        client
            .post(TOKEN_URL)
            .form(&[
                ("client_id", credentials.client.client_id.as_str()),
                ("client_secret", credentials.client.client_secret.as_str()),
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh),
            ])
            .send()
            .map_err(|_| {
                "Could not reach Google. Check your connection; backup will retry.".to_string()
            })?,
    )?;
    if let Some(refresh) = token.refresh_token {
        credentials.refresh_token = Some(refresh);
        store(root, credentials)?;
    }
    Ok(token.access_token)
}

pub fn open_browser(url: &str) -> Result<()> {
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
        return Err("Windows could not open the sign-in browser.".into());
    }
    Ok(())
}

fn callback_code(target: &str, state: &str) -> Result<String> {
    let url = tauri::Url::parse(&format!("http://127.0.0.1{target}")).map_err(err)?;
    if url.path() != "/oauth/callback"
        || url.query_pairs().filter(|(k, _)| k == "state").count() != 1
        || !url.query_pairs().any(|(k, v)| k == "state" && v == state)
    {
        return Err("Invalid sign-in callback.".into());
    }
    if url.query_pairs().any(|(k, _)| k == "error") {
        return Err("Google sign-in was cancelled or denied.".into());
    }
    let codes: Vec<_> = url
        .query_pairs()
        .filter(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .collect();
    if codes.len() != 1 || codes[0].is_empty() {
        return Err("Google sign-in did not return a code.".into());
    }
    Ok(codes[0].clone())
}

pub fn connect(credentials: &mut Credentials, cancel: &AtomicBool) -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(err)?;
    listener.set_nonblocking(true).map_err(err)?;
    let redirect = format!(
        "http://127.0.0.1:{}/oauth/callback",
        listener.local_addr().map_err(err)?.port()
    );
    let state = uuid::Uuid::new_v4().to_string();
    let verifier = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let mut url = tauri::Url::parse("https://accounts.google.com/o/oauth2/v2/auth").map_err(err)?;
    url.query_pairs_mut().extend_pairs([
        ("client_id", credentials.client.client_id.as_str()),
        ("redirect_uri", &redirect),
        ("response_type", "code"),
        ("scope", SCOPE),
        ("state", &state),
        ("code_challenge", &challenge),
        ("code_challenge_method", "S256"),
        ("access_type", "offline"),
        ("prompt", "consent select_account"),
    ]);
    open_browser(url.as_str())?;
    let deadline = Instant::now() + Duration::from_secs(300);
    let code = loop {
        if cancel.load(Ordering::Acquire) {
            return Err("Google sign-in cancelled.".into());
        }
        if Instant::now() > deadline {
            return Err("Google sign-in timed out. Try connecting again.".into());
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .map_err(err)?;
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .map_err(err)?;
                let mut bytes = Vec::new();
                let mut byte = [0];
                while bytes.len() < 8192 && !bytes.ends_with(b"\r\n") {
                    if stream.read(&mut byte).unwrap_or(0) == 0 {
                        break;
                    }
                    bytes.push(byte[0]);
                }
                let line = String::from_utf8_lossy(&bytes);
                let target = line
                    .strip_prefix("GET ")
                    .and_then(|s| s.split_whitespace().next())
                    .unwrap_or("");
                let result = callback_code(target, &state);
                let valid_state = tauri::Url::parse(&format!("http://127.0.0.1{target}"))
                    .ok()
                    .is_some_and(|u| {
                        u.path() == "/oauth/callback"
                            && u.query_pairs().any(|(k, v)| k == "state" && v == state)
                    });
                let (status, body) = if result.is_ok() {
                    (
                        "200 OK",
                        "Sign-in received. Return to Captrain to finish connecting.",
                    )
                } else {
                    (
                        "400 Bad Request",
                        "Sign-in was not accepted. Return to Captrain.",
                    )
                };
                let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{body}", body.len());
                if valid_state {
                    break result?;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100))
            }
            Err(e) => return Err(err(e)),
        }
    };
    let client = http()?;
    let tokens = token_response(
        client
            .post(TOKEN_URL)
            .form(&[
                ("client_id", credentials.client.client_id.as_str()),
                ("client_secret", credentials.client.client_secret.as_str()),
                ("code", &code),
                ("redirect_uri", &redirect),
                ("code_verifier", &verifier),
                ("grant_type", "authorization_code"),
            ])
            .send()
            .map_err(|_| {
                "Could not finish Google sign-in. Check your connection and retry.".to_string()
            })?,
    )?;
    if !tokens.scope.as_deref().is_some_and(|s| {
        s.split_whitespace()
            .any(|v| v == "https://www.googleapis.com/auth/drive.appdata")
    }) {
        return Err(
            "Google Drive permission was not granted. Connect again and allow app storage access."
                .into(),
        );
    }
    let email = checked(
        client
            .get("https://www.googleapis.com/oauth2/v2/userinfo")
            .bearer_auth(&tokens.access_token)
            .send()
            .map_err(err)?,
    )?
    .json::<serde_json::Value>()
    .map_err(err)?
    .get("email")
    .and_then(|v| v.as_str())
    .filter(|s| !s.is_empty())
    .map(String::from)
    .ok_or("Google did not return the connected account's email.")?;
    credentials.refresh_token = Some(
        tokens
            .refresh_token
            .ok_or("Google did not grant offline access. Reconnect and grant consent.")?,
    );
    credentials.email = Some(email);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn oauth_callback_requires_exact_state_and_single_code() {
        assert_eq!(
            callback_code("/oauth/callback?state=abc&code=ok", "abc").unwrap(),
            "ok"
        );
        for target in [
            "/oauth/callback?code=x",
            "/oauth/callback?state=wrong&code=x",
            "/other?state=abc&code=x",
            "/oauth/callback?state=abc&code=x&code=y",
            "/oauth/callback?state=abc&state=abc&code=x",
            "/oauth/callback?state=abc&error=access_denied",
        ] {
            assert!(callback_code(target, "abc").is_err());
        }
    }
    #[test]
    fn credentials_are_encrypted_for_current_windows_user() {
        let secret = b"refresh-token-test";
        let encrypted = protect(secret, true).unwrap();
        assert!(!encrypted.windows(secret.len()).any(|w| w == secret));
        assert_eq!(protect(&encrypted, false).unwrap(), secret);
        assert!(parse_client(br#"{"web":{"client_id":"wrong"}}"#).is_err());
    }
}
