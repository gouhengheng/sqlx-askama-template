#![doc = include_str!("../README.md")]

use db_adapter::DBAdapterManager;
use sqlx::Database;

pub use sqlx_askama_template_macro::*;

pub mod db_adapter;
mod error;
mod sql_templte_execute;
mod template_arg;
pub use error::*;
pub use sql_templte_execute::*;
pub use template_arg::*;
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
    ) -> Result<(String, Option<DB::Arguments<'q>>), Error>;
    /// Renders SQL template and returns query string with parameters
    fn render_sql(self) -> Result<(String, Option<DB::Arguments<'q>>), Error> {
        self.render_sql_with_encode_placeholder_fn(None)
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
    fn render_db_adpter_manager(self, sql_buff: &'q mut String) -> DBAdapterManager<'q, DB, Self> {
        DBAdapterManager::new(self, sql_buff)
    }
}
