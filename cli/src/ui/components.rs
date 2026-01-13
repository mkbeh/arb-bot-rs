use owo_colors::OwoColorize;

use crate::ui::print_feature_error;

pub fn build_banner() -> String {
    let clear = "\x1B[2J\x1B[H";
    let indent = "      ";
    let top = "\n\n";

    let raw_logo_lines = [
        "  █████╗ ██████╗ ██████╗       ██████╗  ██████╗ ████████╗",
        " ██╔══██╗██╔══██╗██╔══██╗      ██╔══██╗██╔═══██╗╚══██╔══╝",
        " ███████║██████╔╝██████╔╝ ███╗ ██████╔╝██║   ██║   ██║",
        " ██╔══██║██╔══██╗██╔══██╗ ╚══╝ ██╔══██╗██║   ██║   ██║",
        " ██║  ██║██║  ██║██████╔╝      ██████╔╝╚██████╔╝   ██║",
        " ╚═╝  ╚═╝╚═╝  ╚═╝╚═════╝       ╚═════╝  ╚═════╝    ╚═╝",
    ];

    let width = raw_logo_lines
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(55);

    let mut logo = String::new();
    for line in raw_logo_lines {
        logo.push_str(&format!("{}{}\n", indent, line));
    }
    let logo = logo.bright_cyan().bold().to_string();

    let d = ".".dimmed().to_string();
    let i = "|".bright_cyan().dimmed().to_string();
    let s = "░".bright_cyan().dimmed().to_string();
    let v = "v".cyan().to_string();
    let rain = format!(
        "{indent}   {d}      {i}   {s}   {d}   {v}      {i}   {d}   {s}   {v}   {i}   {d}   {s}\n\
         {indent}   {s}      {d}   {i}   {v}   {d}      {s}   {v}   {i}   {d}   {s}   {i}   {v}",
        indent = indent
    );

    let line = format!("{indent}{}", "━".repeat(width).dimmed());

    let version = env!("CARGO_PKG_VERSION");
    let arch = std::env::consts::ARCH;

    let raw_text = format!(
        "ARCH: {} • High-Frequency Trading Engine • v{}",
        arch, version
    );
    let raw_len = raw_text.chars().count();

    let padding = width.saturating_sub(raw_len) / 2;
    let leading_spaces = " ".repeat(padding);

    let arch_txt = format!("ARCH: {}", arch).white().bold().to_string();
    let sep = "•".dimmed().to_string();
    let desc = "High-Frequency Trading Engine".dimmed().to_string();
    let ver = format!("v{version}").bright_yellow().to_string();

    let centered_content = format!("{indent}{leading_spaces}{arch_txt} {sep} {desc} {sep} {ver}");

    format!(
        "{clear}{top}{rain}\n\
         {logo}\
         {line}\n\
         {centered_content}\n\
         {line}"
    )
}

pub fn print_version() {
    let version = env!("CARGO_PKG_VERSION");
    println!(
        "{} {}\n",
        "arb-bot".bright_cyan(),
        format!("v{version}").bright_yellow(),
    );
}

pub fn print_exchanges() {
    let items = [
        (
            "Binance",
            "spot market arbitrage",
            cfg!(feature = "binance"),
        ),
        ("KuCoin", "spot market arbitrage", cfg!(feature = "kucoin")),
        (
            "Solana",
            "DEX arbitrage (Jupiter, Raydium, etc)",
            cfg!(feature = "solana"),
        ),
    ];

    let active_items: Vec<_> = items.into_iter().filter(|i| i.2).collect();

    if active_items.is_empty() {
        print_feature_error("binance kucoin solana");
        return;
    }

    println!("\n  {}", "AVAILABLE EXCHANGES".dimmed().bold());
    println!("  {}", "━".repeat(60).dimmed());

    for (name, desc, _) in active_items {
        let dot = "●".bright_green();
        println!(
            "  {} {:<10} {} {}",
            dot,
            name.white().bold(),
            "•".dimmed(),
            desc.white()
        );
    }

    println!("  {}\n", "━".repeat(60).dimmed());
}
