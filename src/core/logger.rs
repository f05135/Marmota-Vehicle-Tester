use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

const MAX_LOG_FILE_BYTES: u64 = 5 * 1024 * 1024; // 5 MiB

struct SizeRotatingWriter {
    logs_dir: PathBuf,
    filename: &'static str,
    max_bytes: u64,
    current_bytes: u64,
    file: Option<File>,
}

impl SizeRotatingWriter {
    fn new(logs_dir: PathBuf, filename: &'static str, max_bytes: u64) -> Self {
        rotate_if_oversized(&logs_dir, filename, max_bytes);
        let path = logs_dir.join(filename);
        let current_bytes = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok();
        Self {
            logs_dir,
            filename,
            max_bytes,
            current_bytes,
            file,
        }
    }

    fn rotate(&mut self) {
        self.file.take();
        let base = self.logs_dir.join(self.filename);
        let bak1 = self.logs_dir.join(format!("{}.1", self.filename));
        let bak2 = self.logs_dir.join(format!("{}.2", self.filename));
        let _ = fs::remove_file(&bak2);
        if bak1.exists() {
            let _ = fs::rename(&bak1, &bak2);
        }
        if base.exists() {
            let _ = fs::rename(&base, &bak1);
        }
        self.current_bytes = 0;
        self.file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&base)
            .ok();
    }
}

impl Write for SizeRotatingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.current_bytes + (buf.len() as u64) > self.max_bytes {
            self.rotate();
        }
        if let Some(ref mut f) = self.file {
            let n = f.write(buf)?;
            self.current_bytes += n as u64;
            Ok(n)
        } else {
            Ok(buf.len())
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(ref mut f) = self.file {
            f.flush()?;
        }
        Ok(())
    }
}

fn rotate_if_oversized(logs_dir: &Path, filename: &str, max_bytes: u64) {
    let base = logs_dir.join(filename);
    if let Ok(meta) = fs::metadata(&base) {
        if meta.len() >= max_bytes {
            let bak1 = logs_dir.join(format!("{}.1", filename));
            let bak2 = logs_dir.join(format!("{}.2", filename));
            let _ = fs::remove_file(&bak2);
            if bak1.exists() {
                let _ = fs::rename(&bak1, &bak2);
            }
            let _ = fs::rename(&base, &bak1);
        }
    }
}

pub fn init_logging() -> PathBuf {
    let logs_dir = crate::config::get_logs_dir();
    let _ = fs::create_dir_all(&logs_dir);

    let rotating_writer = SizeRotatingWriter::new(logs_dir.clone(), "app.log", MAX_LOG_FILE_BYTES);
    let (non_blocking, _guard) = tracing_appender::non_blocking(rotating_writer);

    // Keep guard alive for application duration via Box::leak
    Box::leak(Box::new(_guard));

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,marmota_vehicle_tester=debug,tower_http=info"));

    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_thread_ids(false);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false);

    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(stdout_layer)
        .with(file_layer);

    let _ = subscriber.try_init();

    let os_str = if cfg!(target_os = "windows") {
        "Windows (Win32 / WebView2)"
    } else if cfg!(target_os = "macos") {
        "macOS (Darwin / WebKit)"
    } else {
        "Linux / Unix"
    };

    let config_path = crate::config::get_config_path();
    let assets_dir = crate::config::get_assets_dir();
    let run_mode = if crate::config::is_packaged_app() {
        "Standalone Package (.app / .exe -> Application Support)"
    } else {
        "Development Mode (cargo run -> Project Root Directory)"
    };

    tracing::info!(
        "╔══════════════════════════════════════════════════════════════════════════════╗"
    );
    tracing::info!(
        "║               MARMOTA VEHICLE TESTER — v2.0.0                                ║"
    );
    tracing::info!(
        "╠══════════════════════════════════════════════════════════════════════════════╣"
    );
    tracing::info!("║ Execution Mode:   {:<58} ║", run_mode);
    tracing::info!("║ Target Platform:  {:<58} ║", os_str);
    tracing::info!(
        "║ Active Log File:  {:<58} ║",
        logs_dir.join("app.log").display()
    );
    tracing::info!("║ Config File:      {:<58} ║", config_path.display());
    tracing::info!("║ Assets Directory: {:<58} ║", assets_dir.display());
    tracing::info!(
        "║ Logging Mode:     Structured Telemetry (INFO, DEBUG, WARN, ERROR)            ║"
    );
    tracing::info!(
        "╚══════════════════════════════════════════════════════════════════════════════╝"
    );

    logs_dir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_rotating_writer_rolls_over() {
        let dir = std::env::temp_dir().join(format!("wt_log_rot_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let mut writer = SizeRotatingWriter::new(dir.clone(), "app.log", 128);
        let chunk = [b'X'; 80];
        writer.write_all(&chunk).unwrap();
        writer.write_all(&chunk).unwrap();
        writer.write_all(&chunk).unwrap();
        writer.flush().unwrap();
        drop(writer);

        assert!(dir.join("app.log").exists());
        assert!(dir.join("app.log.1").exists());
        assert!(dir.join("app.log.2").exists());
        assert!(fs::metadata(dir.join("app.log")).unwrap().len() <= 128);

        let _ = fs::remove_dir_all(&dir);
    }
}
