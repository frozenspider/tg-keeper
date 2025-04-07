use crate::utils::*;
use anyhow::{Context, Result};
use grammers_client::grammers_tl_types::{self as tl, Deserializable, Serializable};
use grammers_client::{types, ChatMap};
use rusqlite::{params, types::Null, Connection};
use std::collections::HashMap;
use std::path::Path;

pub struct Database {
    conn: Connection,
    chats: HashMap<i64, (types::Chat, Vec<u8>)>,
}

const TYPE_MESSAGE_NEW: &str = "message_new";
const TYPE_MESSAGE_EDITED: &str = "message_edited";
const TYPE_MESSAGE_DELETED: &str = "message_deleted";

const SQL_INSERT: &str =
    "INSERT INTO events (chat_id, message_id, date, type, serialized, media_rel_path) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6)";

impl Database {
    pub fn new(db_file: &Path) -> Result<Self> {
        let conn = Connection::open(db_file).context("Failed to open database connection")?;

        // Create tables if they don't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY,
                chat_id INTEGER,
                message_id INTEGER NOT NULL,
                date INTEGER,
                type TEXT NOT NULL,
                serialized BLOB,
                media_rel_path TEXT
            )",
            [],
        )
        .context("Failed to create events table")?;

        // Create chats table if it doesn't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS chats (
                chat_id INTEGER PRIMARY KEY,
                serialized BLOB NOT NULL
            )",
            [],
        )
        .context("Failed to create chats table")?;

        // Load chats from database
        let mut chats = HashMap::new();
        let mut stmt = conn
            .prepare("SELECT chat_id, serialized FROM chats")
            .context("Failed to prepare query for loading chats")?;

        let rows = stmt
            .query_map([], |row| {
                let chat_id: i64 = row.get(0)?;
                let serialized: Vec<u8> = row.get(1)?;
                Ok((chat_id, serialized))
            })
            .context("Failed to execute query for loading chats")?;

        for row in rows {
            let (chat_id, serialized) = row.context("Failed to get chat row")?;
            let chat = deserialize_chat(&serialized).context("Failed to deserialize chat")?;
            chats.insert(chat_id, (chat, serialized));
        }
        drop(stmt);

        log::info!("Loaded {} chats from database", chats.len());

        Ok(Database { conn, chats })
    }

    pub fn save_message(
        &mut self,
        raw_message: &tl::enums::Message,
        is_edited: bool,
        media_rel_path: Option<String>,
    ) -> Result<()> {
        let serialized = raw_message.to_bytes();

        let chat_id = raw_message.chat_id().unwrap();
        let date = raw_message.date();
        let event_type = if is_edited {
            TYPE_MESSAGE_EDITED
        } else {
            TYPE_MESSAGE_NEW
        };

        self.conn
            .execute(
                SQL_INSERT,
                params![
                    chat_id,
                    raw_message.id(),
                    date,
                    event_type,
                    serialized,
                    media_rel_path
                ],
            )
            .context("Failed to save message to database")?;
        Ok(())
    }

    pub fn save_messages_deleted(&mut self, message_id: &[i32]) -> Result<()> {
        // Chat ID is unknown!
        let tx = self.conn.transaction()?;
        for id in message_id {
            tx.execute(
                SQL_INSERT,
                params![Null, id, Null, TYPE_MESSAGE_DELETED, Null, Null],
            )
            .context("Failed to save message deleted to database")?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Update the cached chats with new chat data
    pub fn update_chats(&mut self, chat_map: &ChatMap) -> Result<HashMap<i64, types::Chat>> {
        let mut updated_ctr = 0;

        for chat in chat_map.iter_chats() {
            let chat_id = chat.id();
            let serialized = serialize_chat(chat);

            // Only update if the chat is new or different from what we have
            let should_update = self
                .chats
                .get(&chat_id)
                .is_none_or(|(_, existing_serialized)| existing_serialized != &serialized);

            if should_update {
                log::debug!("Updating chat {}", chat_id);
                self.chats
                    .insert(chat_id, (chat.clone(), serialized.clone()));

                // Also update in database
                self.conn
                    .execute(
                        "INSERT OR REPLACE INTO chats (chat_id, serialized) VALUES (?1, ?2)",
                        params![chat_id, serialized],
                    )
                    .context("Failed to update chat in database")?;

                updated_ctr += 1;
            }
        }

        if updated_ctr > 0 {
            log::info!("Updated {updated_ctr} chats in cache");
        }

        let result = self.chats.iter().map(|(k, v)| (*k, v.0.clone())).collect();

        Ok(result)
    }
}

fn serialize_chat(chat: &types::Chat) -> Vec<u8> {
    let mut vec = Vec::with_capacity(1024);
    // Serialize the chat type as first byte
    vec.push(match chat {
        types::Chat::User(_) => 0,
        types::Chat::Group(_) => 1,
        types::Chat::Channel(_) => 2,
    });
    match chat {
        types::Chat::User(user) => user.raw.serialize(&mut vec),
        types::Chat::Group(group) => group.raw.serialize(&mut vec),
        types::Chat::Channel(channel) => channel.raw.serialize(&mut vec),
    }
    vec
}

fn deserialize_chat(serialized: &[u8]) -> Result<types::Chat> {
    // Check the first byte to determine the type of chat
    let chat_type = serialized[0];
    let serialized = &serialized[1..]; // Skip the first byte

    // Deserialize the chat based on its type
    match chat_type {
        0 => {
            let user = tl::enums::User::from_bytes(serialized)?;
            Ok(types::Chat::User(types::chat::User { raw: user }))
        }
        1 => {
            let chat = tl::enums::Chat::from_bytes(serialized)?;
            Ok(types::Chat::Group(types::chat::Group { raw: chat }))
        }
        2 => {
            let channel = tl::types::Channel::from_bytes(serialized)?;
            Ok(types::Chat::Channel(types::chat::Channel { raw: channel }))
        }
        _ => unreachable!("Unknown chat type: {}", chat_type),
    }
}
