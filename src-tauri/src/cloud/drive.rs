use super::auth::{checked, http};
use crate::library::{err, BackupItem, Kind, Result};
use reqwest::{
    blocking::{Client, Response},
    header,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

const FILES: &str = "https://www.googleapis.com/drive/v3/files";
const UPLOAD: &str = "https://www.googleapis.com/upload/drive/v3/files";
const CHUNK: usize = 8 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
pub struct Record {
    pub version: u8,
    pub backup: BackupItem,
    pub sha256: String,
}
impl Record {
    pub fn validate(&self) -> Result<()> {
        if self.version != 1
            || self.sha256.len() != 64
            || !self.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("Unsupported or damaged cloud backup record.".into());
        }
        self.backup.validate()
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFile {
    pub id: String,
    description: Option<String>,
    pub modified_time: String,
    size: Option<String>,
}
impl RemoteFile {
    pub fn record(&self) -> Result<Record> {
        if self.id.is_empty()
            || !self
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err("Invalid Google Drive file ID.".into());
        }
        let record: Record = serde_json::from_str(
            self.description
                .as_deref()
                .ok_or("Missing cloud metadata.")?,
        )
        .map_err(|_| "Damaged cloud item metadata.")?;
        record.validate()?;
        Ok(record)
    }
}

fn json<T: serde::de::DeserializeOwned>(response: Response) -> Result<T> {
    let mut bytes = Vec::new();
    checked(response)?
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(err)?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("Google Drive returned too much metadata.".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "Google Drive returned invalid metadata.".into())
}

pub struct Drive {
    client: Client,
    token: String,
}
impl Drive {
    pub fn new(token: String) -> Result<Self> {
        Ok(Self {
            client: http()?,
            token,
        })
    }

    pub fn list(&self) -> Result<Vec<RemoteFile>> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Page {
            files: Vec<RemoteFile>,
            next_page_token: Option<String>,
        }
        let mut files = Vec::new();
        let mut page = String::new();
        loop {
            let response = self.client.get(FILES).bearer_auth(&self.token).query(&[
                ("spaces", "appDataFolder"), ("q", "trashed = false and appProperties has { key='captrainBackup' and value='1' }"),
                ("fields", "nextPageToken,files(id,description,modifiedTime,size)"), ("pageSize", "1000"), ("pageToken", page.as_str()),
            ]).send().map_err(|_| "Could not reach Google Drive. Check your connection and retry.".to_string())?;
            let result: Page = json(response)?;
            files.extend(result.files);
            if files.len() > 100_000 {
                return Err(
                    "Cloud backup contains too many items for this version of Captrain.".into(),
                );
            }
            match result.next_page_token {
                Some(next) if next != page => page = next,
                Some(_) => return Err("Google Drive repeated a page token.".into()),
                None => break,
            }
        }
        // Concurrent first uploads can create duplicates; newest metadata wins during additive restore.
        files.sort_by(|a, b| {
            b.modified_time
                .cmp(&a.modified_time)
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(files)
    }

    pub fn update_metadata(&self, id: &str, record: &Record) -> Result<()> {
        checked(self.client.patch(format!("{FILES}/{id}")).bearer_auth(&self.token)
            .json(&serde_json::json!({ "description": serde_json::to_string(record).map_err(err)? }))
            .send().map_err(|_| "Could not update cloud metadata. Backup will retry.".to_string())?)?;
        Ok(())
    }

    pub fn upload(&self, record: &Record, file: &mut File, cancel: &AtomicBool) -> Result<()> {
        let length = file.metadata().map_err(err)?.len();
        if length == 0 {
            return Err("Cannot back up an empty file.".into());
        }
        let mime = match record.backup.item.kind {
            Kind::Url => "application/internet-shortcut",
            Kind::Video => "video/mp4",
            _ => "image/png",
        };
        let response = checked(self.client.post(UPLOAD).query(&[("uploadType", "resumable")]).bearer_auth(&self.token)
            .header("X-Upload-Content-Type", mime).header("X-Upload-Content-Length", length)
            .json(&serde_json::json!({ "name": record.backup.item.filename, "parents": ["appDataFolder"],
                "description": serde_json::to_string(record).map_err(err)?, "appProperties": { "captrainBackup": "1" } }))
            .send().map_err(|_| "Could not start the cloud upload. Backup will retry.".to_string())?)?;
        let session = response
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .ok_or("Google Drive did not return an upload session.")?;
        validate_session(session)?;
        self.send_chunks(session, file, length, mime, cancel)
    }

    fn send_chunks(
        &self,
        session: &str,
        file: &mut File,
        length: u64,
        mime: &str,
        cancel: &AtomicBool,
    ) -> Result<()> {
        let mut offset = 0u64;
        let mut stalled = 0;
        while offset < length {
            if cancel.load(Ordering::Acquire) {
                return Err(
                    "Cloud upload cancelled. Local files and completed cloud copies are safe."
                        .into(),
                );
            }
            let count = (length - offset).min(CHUNK as u64) as usize;
            let mut chunk = vec![0; count];
            file.seek(SeekFrom::Start(offset)).map_err(err)?;
            file.read_exact(&mut chunk).map_err(err)?;
            let sent = self
                .client
                .put(session)
                .bearer_auth(&self.token)
                .header(header::CONTENT_TYPE, mime)
                .header(
                    header::CONTENT_RANGE,
                    format!("bytes {offset}-{}/{length}", offset + count as u64 - 1),
                )
                .body(chunk)
                .send();
            let response = match sent {
                Ok(response)
                    if response.status().is_success() || response.status().as_u16() == 308 =>
                {
                    response
                }
                Ok(response) if !response.status().is_server_error() => {
                    checked(response)?;
                    return Err("Unexpected Google upload response.".into());
                }
                _ => {
                    stalled += 1;
                    if stalled > 3 {
                        return Err("Cloud upload was interrupted. Backup will retry; local files are safe.".into());
                    }
                    self.client
                        .put(session)
                        .bearer_auth(&self.token)
                        .header(header::CONTENT_LENGTH, 0)
                        .header(header::CONTENT_RANGE, format!("bytes */{length}"))
                        .send()
                        .map_err(|_| "Upload could not resume. Backup will retry.".to_string())?
                }
            };
            if response.status().is_success() {
                return Ok(());
            }
            if response.status().as_u16() != 308 {
                checked(response)?;
                return Err("Unexpected resumable upload response.".into());
            }
            let next = resume_offset(
                response
                    .headers()
                    .get(header::RANGE)
                    .and_then(|v| v.to_str().ok()),
                length,
            )?;
            if next <= offset {
                stalled += 1;
            } else {
                stalled = 0;
            }
            if stalled > 3 {
                return Err("Cloud upload stopped making progress. Backup will retry.".into());
            }
            offset = next;
        }
        Err("Google Drive did not confirm the completed upload.".into())
    }

    pub fn download(&self, remote: &RemoteFile, destination: &Path) -> Result<()> {
        remote.record()?;
        let length: u64 = remote
            .size
            .as_deref()
            .ok_or("Cloud file size is missing.")?
            .parse()
            .map_err(err)?;
        if length == 0 {
            return Err("Cloud file is empty.".into());
        }
        let response = checked(
            self.client
                .get(format!("{FILES}/{}", remote.id))
                .query(&[("alt", "media")])
                .bearer_auth(&self.token)
                .send()
                .map_err(|_| "Cloud download was interrupted. Retry Restore.".to_string())?,
        )?;
        let mut file = File::options()
            .create_new(true)
            .write(true)
            .open(destination)
            .map_err(err)?;
        let bytes =
            std::io::copy(&mut response.take(length.saturating_add(1)), &mut file).map_err(err)?;
        if bytes != length {
            return Err("Cloud download size does not match. Nothing was imported.".into());
        }
        file.flush().and_then(|_| file.sync_all()).map_err(err)?;
        Ok(())
    }
}

pub fn digest(file: &mut File) -> Result<String> {
    file.seek(SeekFrom::Start(0)).map_err(err)?;
    let mut hasher = Sha256::new();
    let mut chunk = [0; 64 * 1024];
    loop {
        let n = file.read(&mut chunk).map_err(err)?;
        if n == 0 {
            break;
        }
        hasher.update(&chunk[..n]);
    }
    file.seek(SeekFrom::Start(0)).map_err(err)?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn validate_session(session: &str) -> Result<()> {
    let url = tauri::Url::parse(session).map_err(err)?;
    if url.scheme() != "https"
        || url.host_str() != Some("www.googleapis.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || !url.path().starts_with("/upload/drive/v3/files")
    {
        return Err("Google returned an unexpected upload destination.".into());
    }
    Ok(())
}
fn resume_offset(range: Option<&str>, length: u64) -> Result<u64> {
    let Some(range) = range else {
        return Ok(0);
    };
    let end: u64 = range
        .strip_prefix("bytes=0-")
        .ok_or("Invalid upload range.")?
        .parse()
        .map_err(err)?;
    let offset = end.checked_add(1).ok_or("Invalid upload offset.")?;
    if offset > length {
        return Err("Google returned an invalid upload offset.".into());
    }
    Ok(offset)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interrupted_upload_queries_server_and_resumes_from_confirmed_offset() {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for step in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut headers = Vec::new();
                let mut byte = [0];
                while !headers.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    headers.push(byte[0]);
                }
                let headers = String::from_utf8(headers).unwrap().to_lowercase();
                let length: usize = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length: "))
                    .unwrap_or("0")
                    .parse()
                    .unwrap();
                let mut body = vec![0; length];
                stream.read_exact(&mut body).unwrap();
                match step {
                    0 => {
                        assert!(headers.contains("content-range: bytes 0-9/10"));
                        assert_eq!(body, b"0123456789");
                    }
                    1 => {
                        assert!(headers.contains("content-range: bytes */10"));
                        assert!(body.is_empty());
                        stream.write_all(b"HTTP/1.1 308 Resume Incomplete\r\nRange: bytes=0-4\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                    }
                    _ => {
                        assert!(headers.contains("content-range: bytes 5-9/10"));
                        assert_eq!(body, b"56789");
                        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}").unwrap();
                    }
                }
            }
        });
        let path = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::write(&path, b"0123456789").unwrap();
        let drive = Drive {
            client: http().unwrap(),
            token: "test-token".into(),
        };
        drive
            .send_chunks(
                &format!("http://{address}/upload"),
                &mut File::open(&path).unwrap(),
                10,
                "video/mp4",
                &AtomicBool::new(false),
            )
            .unwrap();
        server.join().unwrap();
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn resumable_upload_validates_range_and_destination() {
        assert_eq!(resume_offset(None, 20).unwrap(), 0);
        assert_eq!(resume_offset(Some("bytes=0-7"), 20).unwrap(), 8);
        assert!(resume_offset(Some("bytes=0-20"), 20).is_err());
        assert!(resume_offset(Some("bytes=1-7"), 20).is_err());
        assert!(
            validate_session("https://www.googleapis.com/upload/drive/v3/files?upload_id=x")
                .is_ok()
        );
        for url in [
            "http://www.googleapis.com/upload/drive/v3/files",
            "https://evil.test/upload/drive/v3/files",
            "https://www.googleapis.com/other",
        ] {
            assert!(validate_session(url).is_err());
        }
    }
}
