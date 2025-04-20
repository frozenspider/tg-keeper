# tg-keeper

A [Telegram](https://telegram.org/) messenger client that receives messages in real time and savesthem in SQLite database
for future reference.

tg-keeper is a read-only client, it never attempts to send or modify messages, or perform any other mutating actions
(e.g. mark as read, etc). It also doesn't send your data anywhere, only storing it locally.

## Features

- **Message Archiving**: Stores all incoming and edited messages (private and group) in their serialized form
- **Media Download**: Automatically downloads and saves media files from messages
- **Chat Caching**: Maintains an up-to-date cache of chat information
- **Deleted Message Tracking**: Records when messages are deleted
- **Persistent Authentication**: Uses session files to maintain authentication between runs.

## Details

This app relied heavily on `grammers` library for Telegram API interactions and stored messages are serialized
in the same raw format as `grammers` provides them.
This means that the messages are stored as close as possible to their original format, preserving all the details
and metadata that Telegram provides, but it also means that the database structure is not user-friendly,
requires deserialization to access the message content and may require updates if the `grammers` library changes its
serialization format in the future.

When messages are deleted, tg-keeper records the deletion event but cannot associate it with a specific chat ID
(Telegram doesn't provide this information). Messages aren't deleted under any circumstances.

## Requirements

- Rust 1.85 or later
- Telegram API credentials (api_id and api_hash). You can obtain them by registering an application
  at https://my.telegram.org/apps
- A registered Telegram phone number

## Setup

1. Clone this repository
2. Copy `config.example.toml` to `config.toml` and fill in your Telegram API credentials
3. Build the project with `cargo build --release`
4. Run with `cargo run --release`

## Database Structure

Client uses a SQLite database (`data/tg-keeper.db`) with the following structure:

### Events Table

Stores message events with the following columns:
- `id`: Primary key, auto-generated
- `chat_id`: ID of the chat where the message was posted
- `message_id`: Telegram's message ID
- `date`: Timestamp of the message, if any
- `type`: Event type (`message_new`, `message_edited`, `message_deleted`)
- `serialized`: Raw serialized message data in `grammers` internal format
- `media_rel_path`: Relative path to the downloaded media file, if any

### Chats Table

Stores chat information with the following columns:
- `chat_id`: Primary key, the Telegram chat ID
- `serialized`: Raw serialized chat data in `grammers` internal format

Notes:

1. **Incremental Updates**: This table only stores the most recent version of each chat. Historical chat states are not preserved.
2. **Missing Chats**: If a message refers to a chat that hasn't been seen yet, the chat information might not be available in the table.

### Media Storage

Media files are downloaded and stored in the `data/media/chat_[ID]` directory structure, with filenames based on message IDs.
There's no deduplication, so if the same media is sent in multiple messages, it will be downloaded multiple times.

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- [Grammers](https://github.com/Lonami/grammers) - Rust Telegram client library
- [Rusqlite](https://github.com/rusqlite/rusqlite) - SQLite bindings for Rust
