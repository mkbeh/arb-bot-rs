use comfy_table::{Cell, Color, Table, modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL};
use owo_colors::OwoColorize;

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
