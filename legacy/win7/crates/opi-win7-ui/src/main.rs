// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

#[cfg(target_os = "windows")]
fn main() {
    if let Err(e) = opi_win7_ui::window::run() {
        eprintln!("opi-win7-ui: {e}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("opi-win7-ui only runs on Windows (host build is for protocol tests)");
    std::process::exit(2);
}
