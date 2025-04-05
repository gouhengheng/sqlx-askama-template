#![doc = include_str!("../README.md")]
use futures_core::stream::BoxStream;
use sqlx::{
    Arguments, Database, Either, Error, Execute, Executor, FromRow, IntoArguments,
    database::HasStatementCache,
    query,
    query::{Map, Query, QueryAs},
    query_as, query_as_with, query_with,
};
pub use sqlx_askama_template_macro::*;
use std::cell::RefCell;
/// Internal executor for SQL templates
pub struct SqlTemplateExecute<'q, DB: Database> {
    /// Reference to SQL query string
    pub(crate) sql: &'q String,
    /// SQL parameters
    pub(crate) arguments: Option<DB::Arguments<'q>>,
    /// Persistent flag
    pub(crate) persistent: bool,
}
impl<'q, DB: Database> SqlTemplateExecute<'q, DB> {
    /// Creates a new SQL template executor
    pub fn new(sql: &'q String, arguments: Option<DB::Arguments<'q>>) -> Self {
        SqlTemplateExecute {
            sql,
            arguments,
            persistent: true,
        }
    }
    /// If `true`, the statement will get prepared once and cached to the
    /// connection's statement cache.
    ///
    /// If queried once with the flag set to `true`, all subsequent queries
    /// matching the one with the flag will use the cached statement until the
    /// cache is cleared.
    ///
    /// If `false`, the prepared statement will be closed after execution.
    ///
    /// Default: `true`.
    pub fn set_persistent(mut self, persistent: bool) -> Self {
        self.persistent = persistent;
        self
    }
}
impl<'q, DB: Database + HasStatementCache> SqlTemplateExecute<'q, DB>
where
    DB::Arguments<'q>: IntoArguments<'q, DB>,
{
    /// to sqlx::QueryAs
    /// Converts the SQL template to a `QueryAs` object, which can be executed to fetch rows
    #[inline]
    pub fn to_query_as<O>(self) -> QueryAs<'q, DB, O, DB::Arguments<'q>>
    where
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row>,
    {
        let q = match self.arguments {
            Some(args) => query_as_with(self.sql, args),
            None => query_as(self.sql),
        };
        q.persistent(self.persistent)
    }
    /// to sqlx::Query
    /// Converts the SQL template to a `Query` object, which can be executed to fetch rows
    #[inline]
    pub fn to_query(self) -> Query<'q, DB, DB::Arguments<'q>> {
        let q = match self.arguments {
            Some(args) => {
                //   let wrap = ArgWrapper(args);
                query_with(self.sql, args)
            }
            None => query(self.sql),
        };
        q.persistent(self.persistent)
    }
    /// call sqlx::Query::map
    /// Map each row in the result to another type.
    #[inline]
    pub fn map<F, O>(
        self,
        f: F,
    ) -> Map<'q, DB, impl FnMut(DB::Row) -> Result<O, Error> + Send, DB::Arguments<'q>>
    where
        F: FnMut(DB::Row) -> O + Send,
        O: Unpin,
        DB::Arguments<'q>: IntoArguments<'q, DB>,
    {
        self.to_query().map(f)
    }

    /// call sqlx::Query::try_map
    /// Map each row in the result to another type, returning an error if the mapping fails.
    #[inline]
    pub fn try_map<F, O>(self, f: F) -> Map<'q, DB, F, DB::Arguments<'q>>
    where
        F: FnMut(DB::Row) -> Result<O, Error> + Send,
        O: Unpin,
        DB::Arguments<'q>: IntoArguments<'q, DB>,
    {
        self.to_query().try_map(f)
    }

    /// call sqlx::Query::execute
    /// Execute the query and return the number of rows affected.
    #[inline]
    pub async fn execute<'e, 'c: 'e, E>(self, executor: E) -> Result<DB::QueryResult, Error>
    where
        'q: 'e,
        E: Executor<'c, Database = DB>,
    {
        self.to_query().execute(executor).await
    }
    /// call    sqlx::Query::execute_many
    /// Execute multiple queries and return the rows affected from each query, in a stream.
    #[inline]
    pub async fn execute_many<'e, 'c: 'e, E>(
        self,
        executor: E,
    ) -> BoxStream<'e, Result<DB::QueryResult, Error>>
    where
        'q: 'e,
        E: Executor<'c, Database = DB>,
    {
        #[allow(deprecated)]
        self.to_query().execute_many(executor).await
    }
    /// call sqlx::Query::fetch
    /// Execute the query and return the generated results as a stream.
    #[inline]
    pub fn fetch<'e, 'c: 'e, E>(self, executor: E) -> BoxStream<'e, Result<DB::Row, Error>>
    where
        'q: 'e,
        E: Executor<'c, Database = DB>,
    {
        self.to_query().fetch(executor)
    }
    /// call sqlx::Query::fetch_many
    /// Execute multiple queries and return the generated results as a stream.
    ///
    /// For each query in the stream, any generated rows are returned first,
    /// then the `QueryResult` with the number of rows affected.
    #[inline]
    #[allow(clippy::type_complexity)]
    pub fn fetch_many<'e, 'c: 'e, E>(
        self,
        executor: E,
    ) -> BoxStream<'e, Result<Either<DB::QueryResult, DB::Row>, Error>>
    where
        'q: 'e,
        E: Executor<'c, Database = DB>,
    {
        #[allow(deprecated)]
        self.to_query().fetch_many(executor)
    }
    /// call sqlx::Query::fetch_all
    /// Execute the query and return all the resulting rows collected into a [`Vec`].
    ///
    /// ### Note: beware result set size.
    /// This will attempt to collect the full result set of the query into memory.
    ///
    /// To avoid exhausting available memory, ensure the result set has a known upper bound,
    /// e.g. using `LIMIT`.
    #[inline]
    pub async fn fetch_all<'e, 'c: 'e, E>(self, executor: E) -> Result<Vec<DB::Row>, Error>
    where
        'q: 'e,
        E: Executor<'c, Database = DB>,
    {
        self.to_query().fetch_all(executor).await
    }
    /// call sqlx::Query::fetch_one
    /// Execute the query, returning the first row or [`Error::RowNotFound`] otherwise.
    ///
    /// ### Note: for best performance, ensure the query returns at most one row.
    /// Depending on the driver implementation, if your query can return more than one row,
    /// it may lead to wasted CPU time and bandwidth on the database server.
    ///
    /// Even when the driver implementation takes this into account, ensuring the query returns at most one row
    /// can result in a more optimal query plan.
    ///
    /// If your query has a `WHERE` clause filtering a unique column by a single value, you're good.
    ///
    /// Otherwise, you might want to add `LIMIT 1` to your query.
    #[inline]
    pub async fn fetch_one<'e, 'c: 'e, E>(self, executor: E) -> Result<DB::Row, Error>
    where
        'q: 'e,
        E: Executor<'c, Database = DB>,
    {
        self.to_query().fetch_one(executor).await
    }
    /// call sqlx::Query::fetch_optional
    /// Execute the query, returning the first row or `None` otherwise.
    ///
    /// ### Note: for best performance, ensure the query returns at most one row.
    /// Depending on the driver implementation, if your query can return more than one row,
    /// it may lead to wasted CPU time and bandwidth on the database server.
    ///
    /// Even when the driver implementation takes this into account, ensuring the query returns at most one row
    /// can result in a more optimal query plan.
    ///
    /// If your query has a `WHERE` clause filtering a unique column by a single value, you're good.
    ///
    /// Otherwise, you might want to add `LIMIT 1` to your query.
    #[inline]
    pub async fn fetch_optional<'e, 'c: 'e, E>(self, executor: E) -> Result<Option<DB::Row>, Error>
    where
        'q: 'e,
        E: Executor<'c, Database = DB>,
    {
        self.to_query().fetch_optional(executor).await
    }

    // QueryAs functions wrapp

    /// call sqlx::QueryAs::fetch
    /// Execute the query and return the generated results as a stream.
    pub fn fetch_as<'e, 'c: 'e, O, E>(self, executor: E) -> BoxStream<'e, Result<O, Error>>
    where
        'q: 'e,
        E: 'e + Executor<'c, Database = DB>,
        DB: 'e,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
    {
        self.to_query_as().fetch(executor)
    }
    /// call sqlx::QueryAs::fetch_many
    /// Execute multiple queries and return the generated results as a stream
    /// from each query, in a stream.
    pub fn fetch_many_as<'e, 'c: 'e, O, E>(
        self,
        executor: E,
    ) -> BoxStream<'e, Result<Either<DB::QueryResult, O>, Error>>
    where
        'q: 'e,
        E: 'e + Executor<'c, Database = DB>,
        DB: 'e,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
    {
        #[allow(deprecated)]
        self.to_query_as().fetch_many(executor)
    }
    /// call sqlx::QueryAs::fetch_all
    /// Execute the query and return all the resulting rows collected into a [`Vec`].
    ///
    /// ### Note: beware result set size.
    /// This will attempt to collect the full result set of the query into memory.
    ///
    /// To avoid exhausting available memory, ensure the result set has a known upper bound,
    /// e.g. using `LIMIT`.
    #[inline]
    pub async fn fetch_all_as<'e, 'c: 'e, O, E>(self, executor: E) -> Result<Vec<O>, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c, Database = DB>,
        DB: 'e,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
    {
        self.to_query_as().fetch_all(executor).await
    }
    /// call sqlx::QueryAs::fetch_one
    /// Execute the query, returning the first row or [`Error::RowNotFound`] otherwise.
    ///
    /// ### Note: for best performance, ensure the query returns at most one row.
    /// Depending on the driver implementation, if your query can return more than one row,
    /// it may lead to wasted CPU time and bandwidth on the database server.
    ///
    /// Even when the driver implementation takes this into account, ensuring the query returns at most one row
    /// can result in a more optimal query plan.
    ///
    /// If your query has a `WHERE` clause filtering a unique column by a single value, you're good.
    ///
    /// Otherwise, you might want to add `LIMIT 1` to your query.
    pub async fn fetch_one_as<'e, 'c: 'e, O, E>(self, executor: E) -> Result<O, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c, Database = DB>,
        DB: 'e,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
    {
        self.to_query_as().fetch_one(executor).await
    }
    /// call sqlx::QueryAs::fetch_optional
    /// Execute the query, returning the first row or `None` otherwise.
    ///
    /// ### Note: for best performance, ensure the query returns at most one row.
    /// Depending on the driver implementation, if your query can return more than one row,
    /// it may lead to wasted CPU time and bandwidth on the database server.
    ///
    /// Even when the driver implementation takes this into account, ensuring the query returns at most one row
    /// can result in a more optimal query plan.
    ///
    /// If your query has a `WHERE` clause filtering a unique column by a single value, you're good.
    ///
    /// Otherwise, you might want to add `LIMIT 1` to your query.
    pub async fn fetch_optional_as<'e, 'c: 'e, O, E>(self, executor: E) -> Result<Option<O>, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c, Database = DB>,
        DB: 'e,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
    {
        self.to_query_as().fetch_optional(executor).await
    }
}

impl<'q, DB: Database> Execute<'q, DB> for SqlTemplateExecute<'q, DB> {
    /// Returns the SQL query string
    #[inline]
    fn sql(&self) -> &'q str {
        self.sql
    }

    /// Gets prepared statement (not supported in this implementation)
    #[inline]
    fn statement(&self) -> Option<&DB::Statement<'q>> {
        None
    }

    /// Takes ownership of the bound arguments
    #[inline]
    fn take_arguments(&mut self) -> Result<Option<DB::Arguments<'q>>, sqlx::error::BoxDynError> {
        Ok(self.arguments.take())
    }

    /// Checks if query is persistent
    #[inline]
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
