use comfy_table::{
    Cell, Color, ContentArrangement, Table, modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL,
};
use owo_colors::OwoColorize;

pub fn build_banner() -> String {
    let clear = "\x1B[2J\x1B[H";
    let indent = "      ";
    let top = "\n\n";

    let d = ".".dimmed().to_string();
    let i = "|".bright_cyan().dimmed().to_string();
    let s = "░".bright_cyan().dimmed().to_string();
    let v = "v".cyan().to_string();

    #[allow(clippy::uninlined_format_args)]
    let rain = format!(
        "{indent}   {d}      {i}   {s}   {d}   {v}      {i}   {d}   {s}   {v}   {i}   {d}   {s}\n\
         {indent}   {s}      {d}   {i}   {v}   {d}      {s}   {v}   {i}   {d}   {s}   {i}   {v}",
        indent = indent
    );

    #[allow(clippy::uninlined_format_args)]
    let logo = format!(
        "{indent}  █████╗ ██████╗ ██████╗       ██████╗  ██████╗ ████████╗\n\
         {indent} ██╔══██╗██╔══██╗██╔══██╗      ██╔══██╗██╔═══██╗╚══██╔══╝\n\
         {indent} ███████║██████╔╝██████╔╝ ███╗ ██████╔╝██║   ██║   ██║\n\
         {indent} ██╔══██║██╔══██╗██╔══██╗ ╚══╝ ██╔══██╗██║   ██║   ██║\n\
         {indent} ██║  ██║██║  ██║██████╔╝      ██████╔╝╚██████╔╝   ██║\n\
         {indent} ╚═╝  ╚═╝╚═╝  ╚═╝╚═════╝       ╚═════╝  ╚═════╝    ╚═╝",
        indent = indent
    )
    .bright_cyan()
    .bold()
    .to_string();

    let version = env!("CARGO_PKG_VERSION");
    let line = format!("{indent}{}", "━".repeat(60).dimmed());

    let name = "ARB-BOT".bright_white().bold().to_string();
    let sep = "•".dimmed().to_string();
    let desc = "High-Frequency Trading Engine".white().to_string();
    let ver = format!("v{version}").bright_yellow().to_string();

    format!(
        "{clear}{top}{rain}\n\
         {logo}\n\
         {line}\n\
         {indent}  {name} {sep} {desc} {sep} {ver}\n\
         {line}"
    )
}

pub fn print_exchanges() {
    let items = [
        (
            "binance",
            "Binance spot market arbitrage",
            cfg!(feature = "binance"),
        ),
        (
            "kucoin",
            "KuCoin spot market arbitrage",
            cfg!(feature = "kucoin"),
        ),
        (
            "solana",
            "Solana DEX arbitrage (Jupiter, Raydium, etc)",
            cfg!(feature = "solana"),
        ),
    ];

    let active_items: Vec<_> = items.into_iter().filter(|i| i.2).collect();

    if active_items.is_empty() {
        print_feature_error("binance kucoin solana");
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Exchange")
                .fg(Color::White)
                .add_attribute(comfy_table::Attribute::Bold),
            Cell::new("Status")
                .fg(Color::White)
                .add_attribute(comfy_table::Attribute::Bold),
            Cell::new("Description")
                .fg(Color::White)
                .add_attribute(comfy_table::Attribute::Bold),
        ]);

    for (name, desc, _) in active_items {
        let (color, name_str) = match name {
            "binance" => (Color::Green, "Binance"),
            "kucoin" => (Color::Yellow, "KuCoin"),
            "solana" => (Color::Magenta, "Solana"),
            _ => (Color::White, name),
        };

        table.add_row(vec![
            Cell::new(name_str)
                .fg(color)
                .add_attribute(comfy_table::Attribute::Bold),
            Cell::new("● ACTIVE").fg(Color::Green),
            Cell::new(desc).fg(Color::White),
        ]);
    }

    println!("\n{}", "  Available Exchanges".bright_white().bold());
    for line in table.to_string().lines() {
        println!("  {line}");
    }
}

pub fn print_feature_error(feature_name: &str) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_width(55)
        .add_row(vec![
            Cell::new("⚠️  SYSTEM NOTICE")
                .fg(Color::Red)
                .add_attribute(comfy_table::Attribute::Bold),
        ])
        .add_row(vec![
            Cell::new(format!(
                "The feature for \"{feature_name}\" was not enabled during compilation.\n\n\
                 To use this exchange, please recompile with:\n\
                 cargo build --features \"{feature_name}\"",
            ))
            .fg(Color::Yellow),
        ]);

    println!("\n  {}", "  Maintenance Mode".bright_white().bold());
    for line in table.to_string().lines() {
        println!("  {line}");
    }
}

pub fn print_version() {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_width(50);

    let version = env!("CARGO_PKG_VERSION");
    let authors = env!("CARGO_PKG_AUTHORS").replace(':', ", ");

    table.add_row(vec![
        Cell::new("🚀 ARB-BOT CORE")
            .fg(Color::Cyan)
            .add_attribute(comfy_table::Attribute::Bold),
        Cell::new(format!("v{version}"))
            .fg(Color::Yellow)
            .add_attribute(comfy_table::Attribute::Bold),
    ]);

    table.add_row(vec![
        Cell::new("Engine Status").fg(Color::White),
        Cell::new("Production Ready").fg(Color::Green),
    ]);

    table.add_row(vec![
        Cell::new("Build Arch").fg(Color::White),
        Cell::new(std::env::consts::ARCH).fg(Color::Cyan),
    ]);

    if !authors.is_empty() {
        table.add_row(vec![
            Cell::new("Developers").fg(Color::White),
            Cell::new(authors).fg(Color::Magenta),
        ]);
    }

    println!("\n  {}", "  System Information".bright_white().bold());
    for line in table.to_string().lines() {
        println!("  {line}");
    }
    println!();
}

pub fn print_config_error(path: &std::path::Path, error: &anyhow::Error) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_width(60);

    table.add_row(vec![
        Cell::new("⚠️  CONFIG ERROR")
            .fg(Color::Red)
            .add_attribute(comfy_table::Attribute::Bold),
    ]);

    let mut error_chain = String::new();
    for (i, cause) in error.chain().enumerate() {
        if i == 0 {
            error_chain.push_str(&format!("{cause}\n"));
        } else {
            error_chain.push_str(&format!("  └─ Caused by: {cause}\n"));
        }
    }

    let error_text = format!(
        "Failed to load configuration file.\n\n\
         Path: {}\n\
         Error: {error_chain}\n\n\
         Please check if the file exists and has correct permissions.",
        path.display(),
    );

    table.add_row(vec![Cell::new(error_text).fg(Color::White)]);

    println!("\n  {}", "  Configuration Issue".bright_white().bold());

    for line in table.to_string().lines() {
        println!("  {line}");
    }
}
