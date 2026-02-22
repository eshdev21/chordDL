//! Network operations for dependencies (downloading and version checking)

use futures_util::StreamExt;
use reqwest::Client;
use serde_json;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

/// Generic helper to get latest tag from a GitHub repo
async fn get_latest_github_release_tag(repo: &str) -> Result<String, String> {
    let client = Client::new();
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo);
    let resp = client
        .get(url)
        .header("User-Agent", "Chord-App")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    json["tag_name"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format!("Failed to parse version for {}", repo))
}

/// Get latest yt-dlp version from GitHub
pub async fn get_latest_ytdlp_version() -> Result<String, String> {
    get_latest_github_release_tag("yt-dlp/yt-dlp").await
}

/// Get latest Deno version from GitHub
pub async fn get_latest_deno_version() -> Result<String, String> {
    get_latest_github_release_tag("denoland/deno").await
}

/// Get download URL for yt-dlp based on platform
pub fn get_ytdlp_download_url() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
    }
    #[cfg(target_os = "macos")]
    {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos"
    }
    #[cfg(target_os = "linux")]
    {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp"
    }
}

/// Get download URL for ffmpeg based on platform
pub fn get_ffmpeg_download_url() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip"
    }
    #[cfg(target_os = "macos")]
    {
        "https://evermeet.cx/ffmpeg/getrelease/zip"
    }
    #[cfg(target_os = "linux")]
    {
        "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linux64-gpl.tar.xz"
    }
}

/// Get download URL for Deno based on platform
pub fn get_deno_download_url() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip"
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "https://github.com/denoland/deno/releases/latest/download/deno-aarch64-apple-darwin.zip"
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-apple-darwin.zip"
    }
    #[cfg(target_os = "linux")]
    {
        "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-unknown-linux-gnu.zip"
    }
}

/// Download a file with progress reporting via Event Bus
pub async fn download_file(
    url: &str,
    dest: &PathBuf,
    name: &str,
    event_bus: &crate::events::EventBus,
) -> Result<(), String> {
    let client = Client::new();
    let resp = client
        .get(url)
        .header("User-Agent", "Chord-App")
        .send()
        .await
        .map_err(|e| format!("Download failed: {}", e))?;

    let total_size = resp.content_length().unwrap_or(0);

    // Create a temporary file for download to avoid corrupting existing valid files
    // or leaving partial files in the final location
    let temp_dest = dest.with_extension("partial");

    let mut file = tokio::fs::File::create(&temp_dest)
        .await
        .map_err(|e| format!("Failed to create temporary file: {}", e))?;

    let mut downloaded: u64 = 0;
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(c) => c,
            Err(e) => {
                let _ = tokio::fs::remove_file(&temp_dest).await;
                return Err(format!("Download error: {}", e));
            }
        };

        if let Err(e) = file.write_all(&chunk).await {
            let _ = tokio::fs::remove_file(&temp_dest).await;
            return Err(format!("Write error: {}", e));
        }

        downloaded += chunk.len() as u64;

        let progress = if total_size > 0 {
            (downloaded as f64 / total_size as f64) * 100.0
        } else {
            0.0
        };

        event_bus.emit(crate::events::AppEvent::DependencyStatus {
            target: name.to_string(),
            status: "downloading".to_string(),
            progress,
            total_size: Some(total_size),
            downloaded: Some(downloaded),
        });
    }

    // Emit finalizing status before flush and rename to avoid the 100% stuck UI look
    event_bus.emit(crate::events::AppEvent::DependencyStatus {
        target: name.to_string(),
        status: "finalizing".to_string(),
        progress: 100.0,
        total_size: Some(total_size),
        downloaded: Some(downloaded),
    });

    if let Err(e) = file.flush().await {
        let _ = tokio::fs::remove_file(&temp_dest).await;
        return Err(format!("Flush error: {}", e));
    }

    // Drop file handle before renaming (Windows requirement)
    drop(file);

    // Verify size if Content-Length was provided
    if total_size > 0 && downloaded != total_size {
        let _ = tokio::fs::remove_file(&temp_dest).await;
        return Err(format!(
            "Download incomplete: expected {} bytes, got {}",
            total_size, downloaded
        ));
    }

    // Rename partial file to final destination
    if let Err(e) = tokio::fs::rename(&temp_dest, dest).await {
        let _ = tokio::fs::remove_file(&temp_dest).await;
        return Err(format!("Failed to finalize download file: {}", e));
    }

    Ok(())
}
