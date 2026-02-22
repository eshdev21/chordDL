//! Extraction logic for dependency archives (zip and tar.xz)

use crate::events::EventBus;
use crate::state::AppState;
use tauri::{AppHandle, Manager};

/// Extract multiple binaries from a ZIP archive
async fn extract_binaries_from_zip(
    zip_path: &std::path::Path,
    bin_dir: &std::path::Path,
    name: &str,
    binary_mappings: &[(&str, &str)], // (archive_suffix, target_filename)
    event_bus: &EventBus,
) -> Result<(), String> {
    // Notify UI about extraction start via Bus
    event_bus.emit(crate::events::AppEvent::DependencyStatus {
        target: name.to_string(),
        status: "extracting".to_string(),
        progress: 0.0,
        total_size: None,
        downloaded: None,
    });

    let zip_path_owned = zip_path.to_path_buf();
    let bin_dir_owned = bin_dir.to_path_buf();
    let mappings_owned: Vec<(String, String)> = binary_mappings
        .iter()
        .map(|(a, t)| (a.to_string(), t.to_string()))
        .collect();
    let name_owned = name.to_string();

    let mappings_len = mappings_owned.len();
    let event_bus_clone = event_bus.clone();

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&zip_path_owned)
            .map_err(|e| format!("Failed to open zip: {}", e))?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("Invalid zip archive: {}", e))?;

        let mut extracted_count = 0;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
            let entry_name = file.name().to_string();

            for (suffix, target_name) in &mappings_owned {
                if entry_name.ends_with(suffix) || entry_name == *suffix {
                    let dest_path = bin_dir_owned.join(target_name);
                    let mut outfile = std::fs::File::create(&dest_path).map_err(|e| {
                        format!("Failed to create output file {}: {}", target_name, e)
                    })?;
                    std::io::copy(&mut file, &mut outfile)
                        .map_err(|e| format!("Extraction failed for {}: {}", target_name, e))?;

                    extracted_count += 1;

                    // Emit progress for each successfully extracted file
                    let progress = (extracted_count as f64 / mappings_len as f64) * 100.0;
                    event_bus_clone.emit(crate::events::AppEvent::DependencyStatus {
                        target: name_owned.clone(),
                        status: "extracting".to_string(),
                        progress,
                        total_size: None,
                        downloaded: None,
                    });

                    break;
                }
            }

            if extracted_count == mappings_len {
                break;
            }
        }

        if extracted_count < mappings_len {
            return Err(format!(
                "Could only find {}/{} binaries in the archive for {}",
                extracted_count, mappings_len, name_owned
            ));
        }

        Ok(())
    })
    .await
    .map_err(|e| format!("Task panicked: {}", e))??;

    // Notify UI about completion via Bus
    event_bus.emit(crate::events::AppEvent::DependencyStatus {
        target: name.to_string(),
        status: "extracting".to_string(),
        progress: 100.0,
        total_size: None,
        downloaded: None,
    });

    Ok(())
}

/// Extract ffmpeg and ffprobe from zip file
pub async fn extract_ffmpeg_from_zip(
    app: &AppHandle,
    zip_path: &std::path::Path,
    bin_dir: &std::path::Path,
) -> Result<(), String> {
    #[cfg(windows)]
    let mappings = &[("ffmpeg.exe", "ffmpeg.exe"), ("ffprobe.exe", "ffprobe.exe")];
    #[cfg(not(windows))]
    let mappings = &[("ffmpeg", "ffmpeg"), ("ffprobe", "ffprobe")];

    let event_bus = &app.state::<AppState>().event_bus;
    extract_binaries_from_zip(zip_path, bin_dir, "ffmpeg", mappings, event_bus).await
}

/// Extract deno from zip file
pub async fn extract_deno_from_zip(
    app: &AppHandle,
    zip_path: &std::path::Path,
    bin_dir: &std::path::Path,
) -> Result<(), String> {
    #[cfg(windows)]
    let mappings = &[("deno.exe", "deno.exe")];
    #[cfg(not(windows))]
    let mappings = &[("deno", "deno")];

    let event_bus = &app.state::<AppState>().event_bus;
    extract_binaries_from_zip(zip_path, bin_dir, "deno", mappings, event_bus).await
}

/// Extract ffmpeg and ffprobe from tar.xz file (BtbN Linux build)
#[cfg(target_os = "linux")]
pub async fn extract_ffmpeg_from_tar_xz(
    app: &AppHandle,
    tar_path: &std::path::Path,
    bin_dir: &std::path::Path,
) -> Result<(), String> {
    let event_bus = &app.state::<AppState>().event_bus;
    // Notify UI about extraction start via Bus
    event_bus.emit(crate::events::AppEvent::DependencyStatus {
        target: "ffmpeg".to_string(),
        status: "extracting".to_string(),
        progress: 0.0,
        total_size: None,
        downloaded: None,
    });

    let tar_path_owned = tar_path.to_path_buf();
    let bin_dir_owned = bin_dir.to_path_buf();
    let event_bus_clone = event_bus.clone();

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&tar_path_owned)
            .map_err(|e| format!("Failed to open tar.xz: {}", e))?;
        let decompressor = xz2::read::XzDecoder::new(file);
        let mut archive = tar::Archive::new(decompressor);

        let mut ffmpeg_found = false;
        let mut ffprobe_found = false;

        for entry in archive
            .entries()
            .map_err(|e| format!("Failed to read tar entries: {}", e))?
        {
            let mut entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path().map_err(|e| e.to_string())?;

            if path.ends_with("bin/ffmpeg") {
                let dest = bin_dir_owned.join("ffmpeg");
                let mut outfile = std::fs::File::create(&dest)
                    .map_err(|e| format!("Failed to create ffmpeg: {}", e))?;
                std::io::copy(&mut entry, &mut outfile)
                    .map_err(|e| format!("Extraction failed for ffmpeg: {}", e))?;
                ffmpeg_found = true;
            } else if path.ends_with("bin/ffprobe") {
                let dest = bin_dir_owned.join("ffprobe");
                let mut outfile = std::fs::File::create(&dest)
                    .map_err(|e| format!("Failed to create ffprobe: {}", e))?;
                std::io::copy(&mut entry, &mut outfile)
                    .map_err(|e| format!("Extraction failed for ffprobe: {}", e))?;
                ffprobe_found = true;
            }

            if ffmpeg_found || ffprobe_found {
                let found_count = if ffmpeg_found && ffprobe_found { 2 } else { 1 };
                let progress = (found_count as f64 / 2.0) * 100.0;

                event_bus_clone.emit(crate::events::AppEvent::DependencyStatus {
                    target: "ffmpeg".to_string(),
                    status: "extracting".to_string(),
                    progress,
                    total_size: None,
                    downloaded: None,
                });
            }

            if ffmpeg_found && ffprobe_found {
                break;
            }
        }

        if !ffmpeg_found || !ffprobe_found {
            return Err(
                "Could not find ffmpeg or ffprobe binaries inside the tar.xz archive".to_string(),
            );
        }
        Ok(())
    })
    .await
    .map_err(|e| format!("Task panicked: {}", e))??;

    // Notify UI about extraction completion via Bus
    event_bus.emit(crate::events::AppEvent::DependencyStatus {
        target: "ffmpeg".to_string(),
        status: "extracting".to_string(),
        progress: 100.0,
        total_size: None,
        downloaded: None,
    });

    Ok(())
}
