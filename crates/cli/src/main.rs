use playlist_cli::commands;

/// Parse a `YYYY-MM-DD` date into a Unix timestamp (seconds), interpreting the date as
/// midnight in the canonical [`playlist_core::TIMEZONE`] (Australia/Sydney) to match how
/// dates are displayed. Midnight always exists in that zone (its DST transitions happen at
/// 2-3am), so `.single()` only returns `None` for unparseable input.
fn parse_date(value: &str) -> Option<f64> {
    use chrono::{NaiveDate, TimeZone};
    let naive = NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()?;
    let datetime = naive.and_hms_opt(0, 0, 0)?;
    playlist_core::TIMEZONE
        .from_local_datetime(&datetime)
        .single()
        .map(|dt| dt.timestamp() as f64)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <dbmigrate|import> [args...]", args[0]);
        std::process::exit(1);
    }

    let subcommand = &args[1];

    // Get database connection
    let database = playlist_core::get_database().await?;

    match subcommand.as_str() {
        "dbmigrate" => {
            println!("Running database migration...");
            commands::migrate::run(database).await?;
            println!("Migration completed successfully!");
        }
        "import" => {
            const USAGE: &str =
                "Usage: import <uri> [user_id] [--name <name>] [--date <YYYY-MM-DD>]";

            // Parse the remaining args: positional <uri> [user_id] plus the optional
            // --name / --date flags (which may appear in any order).
            let mut positionals: Vec<String> = Vec::new();
            let mut name: Option<String> = None;
            let mut date: Option<f64> = None;
            let mut rest = args[2..].iter();
            while let Some(arg) = rest.next() {
                match arg.as_str() {
                    "--name" => {
                        name = Some(rest.next().cloned().unwrap_or_else(|| {
                            eprintln!("--name requires a value\n{}", USAGE);
                            std::process::exit(1);
                        }));
                    }
                    "--date" => {
                        let value = rest.next().cloned().unwrap_or_else(|| {
                            eprintln!("--date requires a value\n{}", USAGE);
                            std::process::exit(1);
                        });
                        date = Some(parse_date(&value).unwrap_or_else(|| {
                            eprintln!("Invalid --date \"{}\"; expected YYYY-MM-DD", value);
                            std::process::exit(1);
                        }));
                    }
                    _ => positionals.push(arg.clone()),
                }
            }

            let Some(uri) = positionals.first() else {
                eprintln!("{}", USAGE);
                std::process::exit(1);
            };
            let user_id = positionals.get(1).cloned();

            println!("Importing playlist from URI: {}", uri);
            commands::import_playlist::import_playlist(database, uri, user_id, name, date).await?;
            println!("Import completed successfully!");
        }
        "set-compiler-name" => {
            if args.len() < 4 {
                eprintln!("Usage: {} set-compiler-name <compiler_id> <name>", args[0]);
                std::process::exit(1);
            }
            let compiler_id = &args[2];
            let name = &args[3];
            commands::set_compiler_name::set_compiler_name(database, compiler_id, name).await?;
        }
        _ => {
            eprintln!(
                "Unknown subcommand: {}. Use 'dbmigrate', 'import' or 'set-compiler-name'.",
                subcommand
            );
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_date;

    // Expected timestamps are hard-coded for the canonical Australia/Sydney timezone
    // (playlist_core::TIMEZONE) and are independent of the host timezone.

    #[test]
    fn parse_date_winter_date_is_midnight_sydney_aest() {
        // 2023-05-17T00:00 in Sydney (AEST, +10:00) = 2023-05-16T14:00:00Z.
        assert_eq!(parse_date("2023-05-17"), Some(1684245600.0));
    }

    #[test]
    fn parse_date_summer_date_is_midnight_sydney_aedt() {
        // 2024-01-10T00:00 in Sydney (AEDT, +11:00) = 2024-01-09T13:00:00Z.
        // The +10:00 non-DST offset would give 1704808800; this pins daylight saving.
        assert_eq!(parse_date("2024-01-10"), Some(1704805200.0));
    }

    #[test]
    fn parse_date_leap_day() {
        // 2024-02-29T00:00 in Sydney (AEDT, +11:00) = 2024-02-28T13:00:00Z.
        assert_eq!(parse_date("2024-02-29"), Some(1709125200.0));
    }

    #[test]
    fn parse_date_rejects_feb_29_in_non_leap_year() {
        assert_eq!(parse_date("2023-02-29"), None);
    }

    #[test]
    fn parse_date_rejects_out_of_range_month_and_day() {
        assert_eq!(parse_date("2023-13-01"), None);
        assert_eq!(parse_date("2023-00-01"), None);
        assert_eq!(parse_date("2023-01-32"), None);
        assert_eq!(parse_date("2023-01-00"), None);
    }

    #[test]
    fn parse_date_rejects_invalid_formats() {
        assert_eq!(parse_date(""), None);
        assert_eq!(parse_date("not-a-date"), None);
        assert_eq!(parse_date("2023/05/17"), None);
        assert_eq!(parse_date("17-05-2023"), None);
        assert_eq!(parse_date("2023-05-17T00:00:00"), None);
        assert_eq!(parse_date("2023-05"), None);
    }
}
