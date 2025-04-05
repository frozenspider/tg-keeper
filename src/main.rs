use anyhow::{Result, Context};
use config::Config as AppConfig;
use grammers_client::{Client, Config, InitParams, Update};
use grammers_session::Session;
use log::{info, warn};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

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
    let api_id: i32 = settings.get("tg_api_id")
        .context("tg_api_id not found in config.toml")?;
    let api_hash: String = settings.get("tg_api_hash")
        .context("tg_api_hash not found in config.toml")?;

    // Create client configuration
    let config = Config {
        session: Session::load_file_or_create("tg-keeper.session")?,
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
        let phone = input("Enter your phone number: ")?;
        let token = client.request_login_code(&phone).await?;
        let code = input("Enter the code you received: ")?;
        
        let user = match client.sign_in(&token, &code).await {
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
        client.session().save_to_file("tg-keeper.session")?;
    }

    // Start watching for updates
    info!("Watching for updates...");
    loop {
        let update = client.next_update().await?;
        match update {
            Update::NewMessage(message) => {
                info!("New message: {:?}", message);
            }
            Update::MessageEdited(message) => {
                info!("Message edited: {:?}", message);
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
