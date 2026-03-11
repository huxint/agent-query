use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

static QUIET: AtomicBool = AtomicBool::new(false);

pub fn set_quiet(q: bool) {
    QUIET.store(q, Ordering::Relaxed);
}

pub fn is_quiet() -> bool {
    QUIET.load(Ordering::Relaxed)
}

pub fn warn(msg: &str) {
    if !is_quiet() {
        eprintln!("{}", msg);
    }
}

/// Save output to a file or print to stdout.
pub fn write_output(content: &str, save_path: Option<&str>) -> anyhow::Result<()> {
    match save_path {
        Some(path) => {
            let p = Path::new(path);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(p, content)?;
            eprintln!("Output saved to {}", path);
        }
        None => {
            println!("{}", content);
        }
    }
    Ok(())
}
