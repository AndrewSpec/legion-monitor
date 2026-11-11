//! Legion Monitor — controlling a Lenovo Legion R45w-30 over DDC/CI.
//!
//! `legion-monitor`         opens the application window
//! `legion-monitor --dump`  prints the monitor state in the terminal

mod about;
mod catalog;
mod ddc;
mod i18n;
mod panel;
mod profiles;
mod settings;
mod topology;
mod ui;
mod worker;

use catalog::{multi_mode, window_sources};
use i18n::{Key, Lang, Strings};

/// Window icon, rendered from `assets/icon.svg` into 256x256 RGBA.
/// Regenerate with `rsvg-convert -w 256 -h 256 assets/icon.svg -o assets/icon.png`
/// followed by `tools/png_to_rgba.py assets/icon.png assets/icon.rgba`.
const ICON_RGBA: &[u8] = include_bytes!("../assets/icon.rgba");
const ICON_SIZE: u32 = 256;

fn app_icon() -> eframe::egui::IconData {
    debug_assert_eq!(
        ICON_RGBA.len(),
        (ICON_SIZE * ICON_SIZE * 4) as usize,
        "the embedded icon buffer does not match its declared size"
    );
    eframe::egui::IconData {
        rgba: ICON_RGBA.to_vec(),
        width: ICON_SIZE,
        height: ICON_SIZE,
    }
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--dump") {
        dump();
        return Ok(());
    }

    // --page <name> opens the application on the given page (handy for screenshots
    // and quick access: `legion-monitor --page multi-view`).
    let start = args
        .iter()
        .position(|a| a == "--page")
        .and_then(|i| args.get(i + 1))
        .and_then(|name| catalog::Section::by_slug(name));

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 800.0])
            .with_min_inner_size([980.0, 660.0])
            .with_title(about::NAME)
            .with_app_id("legion-monitor")
            .with_icon(app_icon()),
        ..Default::default()
    };

    eframe::run_native(
        about::NAME,
        options,
        Box::new(move |cc| {
            Ok(Box::new(match start {
                Some(section) => ui::App::starting_at(cc, section),
                None => ui::App::new(cc),
            }))
        }),
    )
}

/// Text report — also works without the GUI and without a connected monitor.
fn dump() {
    let tr: Strings = i18n::strings(Lang::system());
    match ddc::detect() {
        Ok(m) => {
            println!(
                "{}",
                tr.fill(Key::DumpMonitor, &[m.title(&tr), m.connector.clone()])
            );
            let vcp = if m.vcp.is_empty() {
                "?"
            } else {
                m.vcp.as_str()
            };
            println!(
                "{}",
                tr.fill(Key::DumpBus, &[m.bus.clone(), vcp.to_string()])
            );
            if !m.serial.is_empty() {
                println!(
                    "{}",
                    tr.fill(Key::DumpSerial, std::slice::from_ref(&m.serial))
                );
            }
            match ddc::capabilities(m.display) {
                Ok(caps) => {
                    println!(
                        "{}",
                        tr.fill(Key::DumpFeatures, &[caps.features.len().to_string()])
                    );
                    let mut codes: Vec<u8> = caps.features.iter().map(|(c, _)| *c).collect();
                    for extra in [0xf0u8, 0xf2, 0xf4, 0xf5] {
                        if !codes.contains(&extra) {
                            codes.push(extra);
                        }
                    }
                    let vals = ddc::read(m.display, &codes);
                    println!();
                    for (code, name) in &caps.features {
                        if let Some(v) = vals.get(code) {
                            let label = match catalog::fn_key(*code) {
                                Some(key) => tr.t(key).to_string(),
                                None => catalog::unknown_label(&tr, *code, name),
                            };
                            if catalog::kind_for(*code, v.max) == catalog::Kind::ReadOnly {
                                continue;
                            }
                            let range = if v.max > 1 {
                                format!("  {}", tr.fill(Key::DumpRange, &[v.max.to_string()]))
                            } else {
                                String::new()
                            };
                            println!("  {:<26} {}{}", label, v.value, range);
                        }
                    }

                    let mode = multi_mode(
                        vals.get(&0xf2).map(|v| v.value),
                        vals.get(&0xf4).map(|v| v.value),
                    );
                    println!();
                    println!(
                        "{}",
                        tr.fill(Key::DumpMode, &[tr.t(mode.key()).to_string()])
                    );
                    if let Some(input) = vals.get(&0x60) {
                        println!(
                            "{}",
                            tr.fill(Key::DumpSource, &[ddc::input_name(&tr, input.value)])
                        );
                    }
                    // Windows only make sense when multi-view is enabled.
                    if mode != catalog::MultiMode::Off {
                        if let Some((l, r)) = window_sources(vals.get(&0xf0).map(|v| v.value)) {
                            println!(
                                "{}",
                                tr.fill(
                                    Key::DumpWindows,
                                    &[ddc::input_name(&tr, l), ddc::input_name(&tr, r)]
                                )
                            );
                        }
                    }
                }
                Err(e) => println!("{}", tr.fill(Key::DumpCapsFailed, &[e])),
            }
        }
        Err(e) => {
            println!("{}", tr.t(Key::DumpNoMonitor));
            println!("{}", tr.t(Key::DumpConnect));
            println!("{}", tr.fill(Key::DumpDetails, &[e]));
        }
    }
}

#[cfg(test)]
mod tests {
    /// The embedded buffer is easy to replace by accident; catch size mismatches.
    #[test]
    fn icon_buffer_matches_size() {
        assert_eq!(
            super::ICON_RGBA.len(),
            (super::ICON_SIZE * super::ICON_SIZE * 4) as usize
        );
    }
}
