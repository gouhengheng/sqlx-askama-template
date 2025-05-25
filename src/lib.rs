#![doc = include_str!("../README.md")]

use sqlx_core::{Error, database::Database};
mod v3;
pub use askama;
pub use sqlx_askama_template_macro::*;
pub use v3::*;

/// SQL template trait
///
/// Defines basic operations for rendering SQL from templates
pub trait SqlTemplate<'q, DB>: Sized
where
    DB: Database,
{
    fn render_sql_with_encode_placeholder_fn(
        self,
        f: Option<fn(usize, &mut String)>,
        sql_buffer: &mut String,
    ) -> Result<Option<DB::Arguments<'q>>, Error>;
    /// Renders SQL template and returns query string with parameters
    fn render_sql(self) -> Result<(String, Option<DB::Arguments<'q>>), Error> {
        let mut sql_buff = String::new();
        let arg = self.render_sql_with_encode_placeholder_fn(None, &mut sql_buff)?;
        Ok((sql_buff, arg))
    }

    /// Renders SQL template and returns executable query result
    fn render_execute_able(
        self,
        sql_buffer: &'q mut String,
    ) -> Result<SqlTemplateExecute<'q, DB>, Error> {
        let (sql, arguments) = self.render_sql()?;
        *sql_buffer = sql;
        Ok(SqlTemplateExecute {
            sql: sql_buffer,

            arguments,

            persistent: true,
        })
    }
    fn render_db_adapter_manager(self, sql_buff: &'q mut String) -> DBAdapterManager<'q, DB, Self> {
        DBAdapterManager::new(self, sql_buff)
    }
}
