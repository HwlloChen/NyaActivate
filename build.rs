use std::path::Path;
use std::process::Command;

fn main() {
    // Git commit hash
    if let Ok(output) = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        if let Ok(hash) = String::from_utf8(output.stdout) {
            let hash = hash.trim();
            if !hash.is_empty() {
                println!("cargo:rustc-env=GIT_HASH={}", hash);
            }
        }
    }

    // Build timestamp (formatted as ISO 8601)
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let (year, month, day, hour, min, sec) = seconds_to_datetime(secs);
    println!(
        "cargo:rustc-env=BUILD_TIME={:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        year, month, day, hour, min, sec
    );

    let git_head = Path::new(".git/HEAD");
    if git_head.exists() {
        println!("cargo:rerun-if-changed=.git/HEAD");
    }
    println!("cargo:rerun-if-changed=build.rs");
}

#[allow(clippy::many_single_char_names)]
fn seconds_to_datetime(secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = secs / 86400;
    let rem = secs % 86400;
    let hour = (rem / 3600) as u32;
    let min = ((rem % 3600) / 60) as u32;
    let sec = (rem % 60) as u32;

    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32, hour, min, sec)
}
