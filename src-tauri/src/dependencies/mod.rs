//! Dependencies Manager - Handles yt-dlp, ffmpeg, and Deno installation/updates

pub mod archive;
pub mod fetch;
pub mod paths;
pub mod setup;

// Re-export public API
pub use paths::{get_bin_dir, get_deno_path, get_ffmpeg_path, get_ffprobe_path, get_ytdlp_path};
pub use setup::{
    check_dependencies, check_deps_status, get_dependency_installation_state, install_dependencies,
    update_deno, update_ytdlp,
};
