mod db;
mod utils;

use crate::utils::*;
use anyhow::{Context, Result};
use config::Config as AppConfig;
use grammers_client::grammers_tl_types as tl;
use grammers_client::types::{Downloadable, Media};
use grammers_client::{Client, Config, InitParams};
use grammers_session::Session;
use std::fs;
use std::path::{Path, PathBuf};

const SESSION_FILE: &str = "tg-keeper.session";

const DATA_DIR: &str = "data";
const MEDIA_SUBDIR: &str = "media";

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    let data_path = Path::new(DATA_DIR);
    let media_path = data_path.join(MEDIA_SUBDIR);
    fs::create_dir_all(&media_path)?;
    let database_file = data_path.join("tg-keeper.db");

    let mut database = db::Database::new(&database_file)?;

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
    log::info!("Connected to Telegram!");

    // Sign in if needed
    if !client.is_authorized().await? {
        log::info!("Not logged in, sending code request...");
        let phone: String = settings
            .get("tg_phone")
            .context("tg_phone not found in config.toml")?;
        log::info!("Using phone number from config: {}", phone);
        let token = client.request_login_code(&phone).await?;
        let code = input("Enter the code you received: ")?;

        let user = match client.sign_in(&token, &code).await {
            Ok(user) => user,
            Err(grammers_client::client::auth::SignInError::PasswordRequired(password_token)) => {
                log::info!("2FA is required");
                let password = input("Enter your 2FA password: ")?;
                client.check_password(password_token, password).await?
            }
            Err(e) => return Err(e.into()),
        };
        let mut name = user.full_name();
        if name.is_empty() {
            name.push_str("<unnamed>");
        };
        log::info!("Logged in successfully as {name}!");

        // Save the session after successful authentication
        client.session().save_to_file(SESSION_FILE)?;
    }

    // Start watching for updates
    log::info!("Watching for updates...");
    loop {
        let (update, _chats) = client.next_raw_update().await?;
        match update {
            tl::enums::Update::NewMessage(wrapper) => {
                log::info!("New message: {:?}", wrapper);
                database.save_message(&wrapper.message, false)?;

                if let Err(e) = try_download_media_raw(&media_path, &wrapper.message, &client).await
                {
                    log::error!("Failed to download media: {}", e)
                }
            }
            tl::enums::Update::EditMessage(wrapper) => {
                log::info!("Message edited: {:?}", wrapper);
                database.save_message(&wrapper.message, true)?;

                if let Err(e) = try_download_media_raw(&media_path, &wrapper.message, &client).await
                {
                    log::error!("Failed to download media: {}", e)
                }
            }
            tl::enums::Update::DeleteMessages(wrapper) => {
                log::info!("Message(s) deleted: {:?}", wrapper);
                database.save_messages_deleted(&wrapper.messages)?;
            }
            _ => {
                log::debug!("Unhandled raw update: {:?}", update);
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

/// Download media from raw message with the correct extension
async fn try_download_media_raw(
    media_path: &Path,
    raw_message: &tl::enums::Message,
    client: &Client,
) -> Result<Option<PathBuf>> {
    use tl::enums::*;

    let msg_id = raw_message.id();

    let Message::Message(raw_message) = raw_message else {
        return Ok(None); // Only Messages can have media
    };
    let Some(ref raw_media) = raw_message.media else {
        return Ok(None); // No media in this message
    };
    let Some(media) = Media::from_raw(raw_media.clone()) else {
        return Ok(None); // No media in this message
    };

    let chat_id = raw_message.chat_id().unwrap();

    // Determine file extension based on media type
    let ext = match media {
        Media::Photo(ref _p) => "jpg".to_owned(),
        Media::Sticker(ref _v) => "webp".to_owned(),
        Media::Document(ref doc) => {
            let name = doc.name();
            let ext_option = if !name.is_empty() {
                Path::new(name).extension().and_then(|s| s.to_str())
            } else {
                None
            };
            if let Some(ext) = ext_option {
                ext.to_owned()
            } else {
                doc.mime_type()
                    .and_then(mime2ext::mime2ext)
                    .unwrap_or("bin")
                    .to_owned()
            }
        }
        Media::Contact(_) => "vcf".to_owned(),
        Media::Poll(_)
        | Media::Geo(_)
        | Media::GeoLive(_)
        | Media::Venue(_)
        | Media::Dice(_)
        | Media::WebPage(_) => {
            // Not downloadable
            return Ok(None);
        }
        media => unreachable!("Unexpected media type: {:?}", media),
    };
    let file_name = format!("{msg_id}.{ext}");

    // Get chat info for the filename
    let chat_name = format!("chat_{chat_id}");

    // Create filename with date, chat name, message ID, and correct extension
    let file_path = media_path.join(&chat_name).join(&file_name);
    fs::create_dir_all(file_path.parent().unwrap())?;
    log::info!("Attempting to download media to: {}", file_path.display());
    if file_path.exists() {
        log::info!("File already exists, overwriting: {}", file_path.display());
    }

    // Download the media
    client.download_media(&media, &file_path).await?;
    log::info!("Successfully downloaded media to: {}", file_path.display());
    Ok(Some(file_path))
}
