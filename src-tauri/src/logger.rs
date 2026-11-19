use std::path::Path;
use std::sync::Mutex;

static LOG_FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

pub fn init(dir: &Path) -> std::io::Result<()> {
    let logs = dir.join("logs");
    std::fs::create_dir_all(&logs)?;
    let name = format!("dkide-{}.log", chrono::Local::now().format("%Y-%m-%d"));
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(logs.join(name))?;
    *LOG_FILE.lock().unwrap() = Some(file);
    info("logger initialized");
    Ok(())
}

fn write(level: &str, target: &str, msg: &str) {
    let line = format!(
        "[{}] [{}] [{}] {}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
        level,
        target,
        msg
    );
    if let Some(file) = LOG_FILE.lock().unwrap().as_mut() {
        use std::io::Write;
        let _ = file.write_all(line.as_bytes());
        let _ = file.flush();
    }
    eprintln!("{}", line.trim_end());
}

pub fn info(msg: impl AsRef<str>) { write("INFO", "app", msg.as_ref()); }
pub fn warn(msg: impl AsRef<str>) { write("WARN", "app", msg.as_ref()); }
pub fn error(msg: impl AsRef<str>) { write("ERROR", "app", msg.as_ref()); }
pub fn tool(name: &str, msg: impl AsRef<str>) { write("TOOL", name, msg.as_ref()); }
pub fn llm(msg: impl AsRef<str>) { write("LLM", "chat", msg.as_ref()); }
