use grammers_client::grammers_tl_types::enums::InputFileLocation;
use grammers_client::{grammers_tl_types as tl, types};
use std::io::Write;

/// Wrapper around `rpassword::prompt_password` to work
/// around the issue of not being able to access `/dev/tty`
/// (happens in JetBrains debug).
pub fn prompt_password(prompt: impl ToString) -> std::io::Result<String> {
    let prompt = prompt.to_string();
    match rpassword::prompt_password(&prompt) {
        Err(e)
            if e.to_string()
                .to_lowercase()
                .contains("device not configured") =>
        {
            // Error accessing /dev/tty, use stdin instead
            print!("{prompt}");
            std::io::stdout().flush()?;
            let mut password = String::new();
            std::io::stdin().read_line(&mut password)?;
            Ok(password.trim().to_string())
        }
        etc => etc,
    }
}

pub fn pick_largest(
    photo_sizes: Vec<types::photo_sizes::PhotoSize>,
) -> Option<types::photo_sizes::PhotoSize> {
    photo_sizes
        .into_iter()
        .filter_map(|ps| {
            use grammers_client::types::photo_sizes::PhotoSize;
            // Comments are from https://core.telegram.org/type/PhotoSize
            let width = match &ps {
                // Empty constructor. Image with this thumbnail is unavailable.
                PhotoSize::Empty(_) => {
                    return None;
                }
                // Image description.
                PhotoSize::Size(ps) => ps.width,
                // Description of an image and its content.
                PhotoSize::Cached(ps) => ps.width,
                // A low-resolution compressed JPG payload.
                PhotoSize::Stripped(_ps) => {
                    // Use it when no other option is available, so give it minimum size 
                    0
                }
                // Progressively encoded photosize.
                PhotoSize::Progressive(ps) => ps.width,
                // Messages with animated stickers can have a compressed svg (< 300 bytes) to show the outline
                // of the sticker before fetching the actual lottie animation.
                PhotoSize::Path(_ps) => {
                    // We don't need that
                    return None;
                }
            };
            Some((ps, width))
        })
        .max_by_key(|(_, w)| *w)
        .map(|(ps, _)| ps)
}

//
// ChatIdTrait
//

pub trait ChatIdTrait {
    /// Gets the associated chat ID, or whatever passes as a chat ID for the peer entity.
    /// Could be [None] for [tl::types::MessageEmpty]
    fn chat_id(&self) -> Option<i64>;
}

impl ChatIdTrait for tl::enums::Message {
    fn chat_id(&self) -> Option<i64> {
        match self {
            tl::enums::Message::Message(msg) => msg.chat_id(),
            tl::enums::Message::Service(msg) => msg.chat_id(),
            tl::enums::Message::Empty(msg) => msg.chat_id(),
        }
    }
}

impl ChatIdTrait for tl::types::Message {
    fn chat_id(&self) -> Option<i64> {
        self.peer_id.chat_id()
    }
}

impl ChatIdTrait for tl::types::MessageService {
    fn chat_id(&self) -> Option<i64> {
        self.peer_id.chat_id()
    }
}

impl ChatIdTrait for tl::types::MessageEmpty {
    fn chat_id(&self) -> Option<i64> {
        self.peer_id.as_ref().and_then(|peer| peer.chat_id())
    }
}

impl ChatIdTrait for tl::enums::Peer {
    fn chat_id(&self) -> Option<i64> {
        Some(match self {
            tl::enums::Peer::User(user) => user.user_id,
            tl::enums::Peer::Chat(chat) => chat.chat_id,
            tl::enums::Peer::Channel(channel) => channel.channel_id,
        })
    }
}

//
// DateTrait
//

pub trait DateTrait {
    fn date(&self) -> Option<i32>;
}

impl DateTrait for tl::enums::Message {
    fn date(&self) -> Option<i32> {
        match self {
            tl::enums::Message::Message(msg) => Some(msg.date),
            tl::enums::Message::Service(msg) => Some(msg.date),
            tl::enums::Message::Empty(..) => None,
        }
    }
}

impl DateTrait for tl::enums::PhoneCall {
    fn date(&self) -> Option<i32> {
        match self {
            tl::enums::PhoneCall::Empty(_pc) => None,
            tl::enums::PhoneCall::Waiting(pc) => Some(pc.date),
            tl::enums::PhoneCall::Requested(pc) => Some(pc.date),
            tl::enums::PhoneCall::Accepted(pc) => Some(pc.date),
            tl::enums::PhoneCall::Call(pc) => Some(pc.date),
            tl::enums::PhoneCall::Discarded(_pc) => None,
        }
    }
}

//
// Other
//

pub struct DownloadedMedia {
    pub media_rel_path: String,
    pub thumbnail_rel_path: Option<String>,
}

pub struct NotDownloadable;

impl types::Downloadable for NotDownloadable {
    fn to_raw_input_location(&self) -> Option<InputFileLocation> {
        None
    }
}

//
// Downloadable wrapper for dynamic dispatch
//

pub struct DownloadableWrapper {
    dl: Box<dyn types::Downloadable + Send + Sync + 'static>,
}

impl DownloadableWrapper {
    pub fn new<T: types::Downloadable + Send + Sync + 'static>(dl: T) -> Self {
        DownloadableWrapper { dl: Box::new(dl) }
    }
}

impl types::Downloadable for DownloadableWrapper {
    fn to_raw_input_location(&self) -> Option<InputFileLocation> {
        self.dl.to_raw_input_location()
    }

    fn to_data(&self) -> Option<Vec<u8>> {
        self.dl.to_data()
    }

    fn size(&self) -> Option<usize> {
        self.dl.size()
    }
}
