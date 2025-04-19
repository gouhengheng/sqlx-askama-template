use sqlx::{Pool, Sqlite, SqliteConnection};

use super::DBAdapter;

impl<'c> DBAdapter<'c, Sqlite> for &'c mut SqliteConnection {}

impl<'c> DBAdapter<'c, Sqlite> for &'c Pool<Sqlite> {}
