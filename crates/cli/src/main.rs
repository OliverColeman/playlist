mod commands;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Playlist CLI Tool");

    // Get database connection
    let database = playlist_core::get_database().await?;

    // Run migration
    commands::migrate::run(database).await?;

    println!("Migration completed successfully!");

    Ok(())
}
