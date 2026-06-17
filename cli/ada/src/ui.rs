//! Tiny color helpers used by `ada` to visually distinguish CLI-generated
//! output from output streamed by underlying tools (`helm`, `kubectl`,
//! `rad`, `az`, `k3d`, …). Colors auto-disable when stdout is not a TTY
//! or when `NO_COLOR` is set (handled by `owo_colors::if_supports_color`).

use owo_colors::{OwoColorize, Style, Stream::Stdout};

fn paint(text: &str, style: Style) -> String {
    text.if_supports_color(Stdout, |t| t.style(style)).to_string()
}

fn heading_style() -> Style {
    Style::new().bold().bright_cyan()
}
fn key_style() -> Style {
    Style::new().bright_blue()
}
fn cmd_style() -> Style {
    Style::new().bright_yellow()
}
fn cmd_prefix_style() -> Style {
    Style::new().bold().bright_yellow()
}
fn step_style() -> Style {
    Style::new().bold().bright_green()
}
fn note_style() -> Style {
    Style::new().bold().bright_magenta()
}
fn warn_style() -> Style {
    Style::new().bold().yellow()
}
fn dim_style() -> Style {
    Style::new().dimmed()
}

/// Top-level section heading, e.g. `Plan`, `Radius bootstrap plan:`.
pub fn heading(text: &str) {
    println!("{}", paint(text, heading_style()));
}

/// Key/value detail line inside a section, e.g. `  portfolio : min`.
pub fn detail(key: &str, value: &str) {
    let label = format!("{key:<12}:");
    println!("  {} {}", paint(&label, key_style()), value);
}

/// Plain indented bullet inside a section.
pub fn bullet(text: &str) {
    println!("  {}", paint(text, key_style()));
}

/// A shell command preview, prefixed with `$`.
pub fn command(rendered: &str) {
    println!(
        "{} {}",
        paint("$", cmd_prefix_style()),
        paint(rendered, cmd_style())
    );
}

/// In-progress status line for a CLI-driven step.
pub fn step(text: &str) {
    println!("{} {}", paint("->", step_style()), text);
}

/// Successful completion of a CLI-driven step.
pub fn ok(text: &str) {
    println!("{} {}", paint("✓", step_style()), text);
}

/// Informational note.
pub fn note(text: &str) {
    println!("{} {}", paint("note:", note_style()), text);
}

/// Non-fatal warning.
pub fn warn(text: &str) {
    eprintln!("{} {}", paint("warn:", warn_style()), text);
}

/// Dry-run annotation.
pub fn dry_run(text: &str) {
    println!("{} {}", paint("(dry-run)", note_style()), text);
}

/// Banner shown right before output from an external tool starts streaming,
/// so the user can tell which lines belong to which tool.
pub fn tool_banner(tool: &str, label: &str) {
    println!(
        "{} {} {}",
        paint("──", dim_style()),
        paint(&format!("[{tool}]"), heading_style()),
        paint(label, dim_style())
    );
}
