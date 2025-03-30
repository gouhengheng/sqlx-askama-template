use sqlx::{Arguments, Database, Execute};
pub use sqlx_askama_template_macro::*;
use std::cell::RefCell;

/// Represents the result of SQL template rendering
///
/// Contains generated SQL query and bound parameters ready for execution
///
/// # Generic Parameters
/// - `'q`: Lifetime for query parameters
/// - `DB`: Database type (PostgreSQL/MySQL/SQLite etc.)
pub struct SqlTemplateResult<'q, DB: Database> {
    /// The generated SQL query string
    pub(crate) sql: String,
    /// Bound SQL parameters
    pub(crate) arguments: Option<DB::Arguments<'q>>,
    /// Whether the query should be prepared as persistent
    pub(crate) persistent: bool,
}

impl<'q, DB: Database> SqlTemplateResult<'q, DB> {
    /// Creates a new SQL template result
    pub fn new(sql: String, arguments: Option<DB::Arguments<'q>>) -> Self {
        SqlTemplateResult {
            sql,
            arguments,
            persistent: true,
        }
    }

    /// Gets reference to the SQL query string
    pub fn get_sql(&self) -> &str {
        &self.sql
    }

    /// Updates the SQL query string
    pub fn set_sql(&mut self, sql: String) {
        self.sql = sql;
    }

    /// Takes ownership of the bound arguments
    pub fn get_arguments(&mut self) -> Option<DB::Arguments<'q>> {
        self.arguments.take()
    }

    /// Sets new SQL arguments
    pub fn set_arguments(&mut self, arguments: DB::Arguments<'q>) {
        self.arguments = Some(arguments);
    }

    /// Checks if query is marked as persistent
    pub fn is_persistent(&self) -> bool {
        self.persistent
    }

    /// Sets the persistent flag for the query
    pub fn set_persistent(&mut self, persistent: bool) {
        self.persistent = persistent;
    }

    /// Converts into an executable query object
    pub fn as_execute(&'q mut self) -> impl Execute<'q, DB> {
        SqlTemplateExecute {
            sql: &self.sql,
            arguments: self.arguments.take(),
            persistent: self.persistent,
        }
    }
}

/// Internal executor for SQL templates
struct SqlTemplateExecute<'q, DB: Database> {
    /// Reference to SQL query string
    pub(crate) sql: &'q str,
    /// SQL parameters
    pub(crate) arguments: Option<DB::Arguments<'q>>,
    /// Persistent flag
    pub(crate) persistent: bool,
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
    fn render_execute(self) -> Result<SqlTemplateResult<'q, DB>, askama::Error> {
        let (sql, arguments) = self.render_sql()?;
        Ok(SqlTemplateResult::new(sql, arguments))
    }
}
