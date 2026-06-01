mod commands;

/// Parse a `YYYY-MM-DD` date into a Unix timestamp (seconds), interpreting the date as
/// midnight in the local timezone to match how dates are displayed.
fn parse_date(value: &str) -> Option<f64> {
    use chrono::{Local, NaiveDate, TimeZone};
    let naive = NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()?;
    let datetime = naive.and_hms_opt(0, 0, 0)?;
    Local
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
