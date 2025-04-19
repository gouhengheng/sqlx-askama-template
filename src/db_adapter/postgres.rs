use sqlx::{PgConnection, Pool, Postgres, postgres::PgListener};

use super::DBAdapter;

impl<'c> DBAdapter<'c, Postgres> for &'c mut PgListener {}
impl<'c> DBAdapter<'c, Postgres> for &'c mut PgConnection {}

impl<'c> DBAdapter<'c, Postgres> for &'c Pool<Postgres> {}
