use anyhow::{Context, Result};
use chrono::Local;
use config::Config as AppConfig;
use grammers_client::types::{Media, Message};
use grammers_client::{Client, Config, InitParams, Update};
use grammers_session::Session;
use log::{error, info, warn};
use std::fs;
use std::path::{Path, PathBuf};

const SESSION_FILE: &str = "tg-keeper.session";
const MEDIA_DIR: &str = "media";

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    // Create media directory if it doesn't exist
    ensure_media_dir_exists()?;

    // Load configuration
    let config_path = PathBuf::from("config.toml");
    if !config_path.exists() {
        return Err(anyhow::anyhow!("config.toml not found. Please copy config.example.toml to config.toml and fill in your credentials."));
    }

    let settings = AppConfig::builder()
        .add_source(config::File::from(config_path))
        .build()
        .context("Failed to load config.toml")?;

    // Get API credentials from config
    let api_id: i32 = settings
        .get("tg_api_id")
        .context("tg_api_id not found in config.toml")?;
    let api_hash: String = settings
        .get("tg_api_hash")
        .context("tg_api_hash not found in config.toml")?;

    // Create client configuration
    let config = Config {
        session: Session::load_file_or_create(SESSION_FILE)?,
        api_id,
        api_hash: api_hash.clone(),
        params: InitParams {
            catch_up: true,
            ..Default::default()
        },
    };

    // Create and connect client
    let client = Client::connect(config).await?;
    info!("Connected to Telegram!");

    // Sign in if needed
    if !client.is_authorized().await? {
        info!("Not logged in, sending code request...");
        let phone: String = settings
            .get("tg_phone")
            .context("tg_phone not found in config.toml")?;
        info!("Using phone number from config: {}", phone);
        let token = client.request_login_code(&phone).await?;
        let code = input("Enter the code you received: ")?;

        let _user = match client.sign_in(&token, &code).await {
            Ok(user) => {
                info!("Logged in successfully as {}!", user.first_name());
                user
            }
            Err(grammers_client::client::auth::SignInError::PasswordRequired(password_token)) => {
                info!("2FA is required");
                let password = input("Enter your 2FA password: ")?;
                let user = client.check_password(password_token, password).await?;
                info!("Logged in successfully as {}!", user.first_name());
                user
            }
            Err(e) => return Err(e.into()),
        };

        // Save the session after successful authentication
        client.session().save_to_file(SESSION_FILE)?;
    }

    // Start watching for updates
    info!("Watching for updates...");
    loop {
        let update = client.next_update().await?;
        match update {
            Update::NewMessage(message) => {
                info!("New message: {:?}", message);

                if let Err(e) = try_download_media(&message).await {
                    error!("Failed to download photo: {}", e)
                }
            }
            Update::MessageEdited(message) => {
                info!("Message edited: {:?}", message);

                if let Err(e) = try_download_media(&message).await {
                    error!("Failed to download photo: {}", e)
                }
            }
            Update::MessageDeleted(message_id) => {
                info!("Message deleted: {:?}", message_id);
            }
            _ => {
                warn!("Unhandled update: {:?}", update);
            }
        }
    }
}

// Helper function to get user input
fn input(message: &str) -> Result<String> {
    use std::io::Write;
    print!("{}", message);
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

// Ensure media directory exists
fn ensure_media_dir_exists() -> Result<()> {
    let media_path = Path::new(MEDIA_DIR);
    if !media_path.exists() {
        info!("Creating media directory at {}", media_path.display());
        fs::create_dir_all(media_path)?;
    }
    Ok(())
}

// Download media from message with the correct extension
async fn try_download_media(message: &Message) -> Result<Option<PathBuf>> {
    let Some(media) = message.media() else {
        return Ok(None); // No media in this message
    };

    // Determine file extension based on media type
    let file_name = match media {
        Media::Document(doc) if !doc.name().is_empty() => doc.name().to_owned(),
        media => {
            // Generate filename based on date/time
            let now = Local::now();
            let date_str = now.format("%Y-%m-%d_%H-%M-%S");

            let ext = match media {
                Media::Photo(_p) => "jpg".to_owned(),
                Media::Sticker(_s) => "webp".to_owned(),
                Media::Document(doc) => doc
                    .mime_type()
                    .and_then(mime2ext::mime2ext)
                    .unwrap_or("bin")
                    .to_owned(),
                Media::Contact(_) => "vcf".to_owned(),
                Media::Poll(_) => "poll".to_owned(), // Just a placeholder, not downloadable
                Media::Geo(_) | Media::GeoLive(_) => "geo".to_owned(), // Just a placeholder, not downloadable
                Media::Venue(_) => "venue".to_owned(), // Just a placeholder, not downloadable
                Media::Dice(_) => "dice".to_owned(),   // Just a placeholder, not downloadable
                Media::WebPage(_) => "html".to_owned(), // Just a placeholder, not downloadable
                media => unreachable!("Unexpected media type: {:?}", media),
            };
            format!("{date_str}_{}.{ext}", message.id())
        }
    };

    // Get chat info for the filename
    let chat_name = format!("chat_{}", message.chat().id());

    // Create filename with date, chat name, message ID, and correct extension
    let file_path = Path::new(MEDIA_DIR).join(&chat_name).join(&file_name);
    fs::create_dir_all(file_path.parent().unwrap())?;
    info!("Attempting to download media to: {}", file_path.display());

    // Download the media
    let media_present = message.download_media(&file_path).await?;
    assert!(media_present);
    info!("Successfully downloaded media to: {}", file_path.display());
    Ok(Some(file_path))
}
