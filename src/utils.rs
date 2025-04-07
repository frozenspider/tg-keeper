use grammers_client::grammers_tl_types as tl;

//
// ChatIdTrait
//

pub trait ChatIdTrait {
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
