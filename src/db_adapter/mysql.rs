use sqlx::{MySql, MySqlConnection, Pool};

use super::DBAdapter;

impl<'c> DBAdapter<'c, MySql> for &'c mut MySqlConnection {}

impl<'c> DBAdapter<'c, MySql> for &'c Pool<MySql> {}

