#![doc = include_str!("../README.md")]

use sqlx_core::{Error, database::Database};

pub use askama;
pub use sqlx_askama_template_macro::*;
mod db_adapter;
mod sql_template_execute;
mod template_adapter;
mod template_arg;

pub use db_adapter::*;
pub use sql_template_execute::*;
pub use template_adapter::*;
pub use template_arg::*;

/// SQL template trait
///
/// Defines basic operations for rendering SQL from templates
pub trait SqlTemplate<'q, DB>: Sized + Clone
where
    DB: Database,
{
    /// Renders the SQL template using a custom placeholder encoding function
    ///
    /// Writes the rendered SQL to the provided buffer and handles parameter encoding.
    /// The placeholder function (if provided) formats parameter placeholders (e.g., $1, ?)
    /// based on their index.
    ///
    /// # Parameters
    /// - `f`: Optional function to format parameter placeholders (receives index and buffer)
    /// - `sql_buffer`: Mutable string buffer to store the rendered SQL
    ///
    /// # Returns
    /// Encoded database arguments (None if no parameters) or an error if rendering fails
    fn render_sql_with_encode_placeholder_fn(
        self,
        f: Option<fn(usize, &mut String)>,
        sql_buffer: &mut String,
    ) -> Result<Option<DB::Arguments>, Error>;
    /// Renders SQL template and returns query string with parameters
    fn render_sql(self) -> Result<(String, Option<DB::Arguments>), Error> {
        let mut sql_buff = String::new();
        let arg = self.render_sql_with_encode_placeholder_fn(None, &mut sql_buff)?;
        Ok((sql_buff, arg))
    }

    /// Renders SQL template and returns executable query result
    fn render_executable(self) -> Result<SqlTemplateExecute<DB>, Error> {
        let (sql, arguments) = self.render_sql()?;

        Ok(SqlTemplateExecute {
            sql,
            arguments,
            persistent: true,
        })
    }

    /// Creates a database adapter manager for the template
    ///
    /// Provides an adapter pattern interface for managing template rendering
    /// in database-specific scenarios.
    ///
    /// # Returns
    /// A new `DBAdapterManager` instance wrapping the template
    fn adapter_render(self) -> DBAdapterManager<'q, DB, Self> {
        DBAdapterManager::new(self)
    }
}
