#![doc = include_str!("../README.md")]
use sqlx::{Arguments, Database, Execute};
pub use sqlx_askama_template_macro::*;
use std::cell::RefCell;

/// Internal executor for SQL templates
pub struct SqlTemplateExecute<'q, DB: Database> {
    /// Reference to SQL query string
    pub(crate) sql: &'q str,
    /// SQL parameters
    pub(crate) arguments: Option<DB::Arguments<'q>>,
    /// Persistent flag
    pub(crate) persistent: bool,
}
impl<DB: Database> SqlTemplateExecute<'_, DB> {
    pub fn set_persistent(&mut self, persistent: bool) {
        self.persistent = persistent;
    }
}
impl<'q, DB: Database> Execute<'q, DB> for SqlTemplateExecute<'q, DB> {
    /// Returns the SQL query string
    fn sql(&self) -> &'q str {
        self.sql
    }

    /// Gets prepared statement (not supported in this implementation)
    fn statement(&self) -> Option<&DB::Statement<'q>> {
        None
    }

    /// Takes ownership of the bound arguments
    fn take_arguments(&mut self) -> Result<Option<DB::Arguments<'q>>, sqlx::error::BoxDynError> {
        Ok(self.arguments.take())
    }

    /// Checks if query is persistent
    fn persistent(&self) -> bool {
        self.persistent
    }
}

/// SQL template argument processor
///
/// Handles parameter encoding and binding for SQL templates
pub struct TemplateArg<'q, DB: Database> {
    /// Stores any encoding errors
    error: RefCell<Option<sqlx::error::BoxDynError>>,
    /// Stores SQL parameters
    arguments: RefCell<Option<DB::Arguments<'q>>>,
}

impl<DB: Database> Default for TemplateArg<'_, DB> {
    /// Creates default TemplateArg
    fn default() -> Self {
        TemplateArg {
            error: RefCell::new(None),
            arguments: RefCell::new(None),
        }
    }
}

impl<'q, DB: Database> TemplateArg<'q, DB> {
    /// Encodes a single parameter and returns its placeholder
    ///
    /// # Arguments
    /// * `t` - Value to encode
    ///
    /// # Returns
    /// Parameter placeholder string (e.g. "$1" or "?")
    pub fn encode<T>(&self, t: T) -> String
    where
        T: sqlx::Encode<'q, DB> + sqlx::Type<DB> + 'q,
    {
        let mut err = self.error.borrow_mut();
        let mut arguments = self.arguments.borrow_mut().take().unwrap_or_default();

        if let Err(e) = arguments.add(t) {
            if err.is_none() {
                *err = Some(e);
            }
        }

        let mut placeholder = String::new();
        if let Err(e) = arguments.format_placeholder(&mut placeholder) {
            if err.is_none() {
                *err = Some(Box::new(e));
            }
        }

        *self.arguments.borrow_mut() = Some(arguments);
        placeholder
    }

    /// Encodes a parameter list and returns placeholder sequence
    ///
    /// # Arguments
    /// * `args` - Iterator of values to encode
    ///
    /// # Returns
    /// Parameter placeholder sequence (e.g. "($1,$2,$3)")
    pub fn encode_list<T>(&self, args: impl Iterator<Item = T>) -> String
    where
        T: sqlx::Encode<'q, DB> + sqlx::Type<DB> + 'q,
    {
        let mut err = self.error.borrow_mut();
        let mut arguments = self.arguments.borrow_mut().take().unwrap_or_default();
        let mut placeholder = String::new();
        placeholder.push('(');

        for arg in args {
            if let Err(e) = arguments.add(arg) {
                if err.is_none() {
                    *err = Some(e);
                }
            }

            if let Err(e) = arguments.format_placeholder(&mut placeholder) {
                if err.is_none() {
                    *err = Some(Box::new(e));
                }
            }
            placeholder.push(',');
        }

        if placeholder.ends_with(",") {
            placeholder.pop();
        }
        placeholder.push(')');

        *self.arguments.borrow_mut() = Some(arguments);
        placeholder
    }

    /// Takes any encoding error that occurred
    pub fn get_err(&self) -> Option<sqlx::error::BoxDynError> {
        self.error.borrow_mut().take()
    }

    /// Takes ownership of the encoded arguments
    pub fn get_arguments(&self) -> Option<DB::Arguments<'q>> {
        self.arguments.borrow_mut().take()
    }
}

/// SQL template trait
///
/// Defines basic operations for rendering SQL from templates
pub trait SqlTemplate<'q, DB>: Sized
where
    DB: Database,
{
    /// Renders SQL template and returns query string with parameters
    fn render_sql(self) -> Result<(String, Option<DB::Arguments<'q>>), askama::Error>;

    /// Renders SQL template and returns executable query result
    fn render_execute_able(
        self,
        sql_buffer: &'q mut String,
    ) -> Result<SqlTemplateExecute<'q, DB>, askama::Error> {
        let (sql, arguments) = self.render_sql()?;
        *sql_buffer = sql;
        Ok(SqlTemplateExecute {
            sql: sql_buffer,

            arguments,

            persistent: true,
        })
    }
}
