use crate::utils::*;
use anyhow::{Context, Result};
use grammers_client::grammers_tl_types::{self as tl, Serializable};
use rusqlite::{params, types::Null, Connection};
use std::path::Path;

const DB_FILE: &str = "tg-keeper.db";

pub struct Database {
    conn: Connection,
}

const TYPE_MESSAGE: &str = "message";
const TYPE_MESSAGE_DELETED: &str = "message_deleted";

const SQL_INSERT: &str =
    "INSERT INTO events (chat_id, message_id, date, is_edited, type, serialized) VALUES (?1, ?2, ?3, ?4, ?5, ?6)";

impl Database {
    pub fn new() -> Result<Self> {
        let conn = Connection::open(DB_FILE).context("Failed to open database connection")?;

        // Create tables if they don't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY,
                chat_id INTEGER,
                message_id INTEGER NOT NULL,
                date INTEGER,
                is_edited INTEGER NOT NULL,
                type TEXT NOT NULL,
                serialized BLOB
            )",
            [],
        )
        .context("Failed to create events table")?;

        Ok(Database { conn })
    }

    pub fn save_message(
        &mut self,
        raw_message: &tl::enums::Message,
        is_edited: bool,
    ) -> Result<()> {
        let serialized = raw_message.to_bytes();

        let chat_id = raw_message.chat_id().unwrap();
        let date = raw_message.date();

        self.conn
            .execute(
                SQL_INSERT,
                params![
                    chat_id,
                    raw_message.id(),
                    date,
                    is_edited,
                    TYPE_MESSAGE,
                    serialized
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
                params![Null, id, Null, false, TYPE_MESSAGE_DELETED, Null],
            )
            .context("Failed to save message deleted to database")?;
        }
        tx.commit()?;
        Ok(())
    }
}

pub fn ensure_db_exists() -> Result<()> {
    if !Path::new(DB_FILE).exists() {
        log::info!("Creating new database at {}", DB_FILE);
        // Just opening the connection will create the file
        let _db = Database::new()?;
    }
    Ok(())
}
