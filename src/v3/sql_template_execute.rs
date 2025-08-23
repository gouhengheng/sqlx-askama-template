use futures_core::stream::BoxStream;
use futures_util::{StreamExt, TryStreamExt};
use sqlx_core::{
    Either, Error,
    arguments::IntoArguments,
    database::{Database, HasStatementCache},
    executor::{Execute, Executor},
    from_row::FromRow,
    query::{Map, Query, query, query_with},
    query_as::{QueryAs, query_as, query_as_with},
};
/// Internal executor for SQL templates
pub struct SqlTemplateExecute<'q, DB: Database> {
    /// Reference to SQL query string
    pub(crate) sql: &'q str,
    /// SQL parameters
    pub(crate) arguments: Option<DB::Arguments<'q>>,
    /// Persistent flag
    pub(crate) persistent: bool,
}
impl<'q, DB: Database> Clone for SqlTemplateExecute<'q, DB>
where
    DB::Arguments<'q>: Clone,
{
    fn clone(&self) -> Self {
        SqlTemplateExecute {
            sql: self.sql,
            arguments: self.arguments.clone(),
            persistent: self.persistent,
        }
    }
}
impl<'q, DB: Database> SqlTemplateExecute<'q, DB> {
    /// Creates a new SQL template executor
    pub fn new(sql: &'q str, arguments: Option<DB::Arguments<'q>>) -> Self {
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
impl<'q, DB> SqlTemplateExecute<'q, DB>
where
    DB: Database + HasStatementCache,
    DB::Arguments<'q>: IntoArguments<'q, DB>,
{
    /// to sqlx_core::QueryAs
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
    /// to sqlx_core::Query
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
    /// like sqlx_core::Query::map
    /// Map each row in the result to another type.
    #[inline]
    pub fn map<F, O>(
        self,
        f: F,
    ) -> Map<'q, DB, impl FnMut(DB::Row) -> Result<O, sqlx_core::Error> + Send, DB::Arguments<'q>>
    where
        F: FnMut(DB::Row) -> O + Send,
        O: Unpin,
    {
        self.to_query().map(f)
    }

    /// like sqlx_core::Query::try_map
    /// Map each row in the result to another type, returning an error if the mapping fails.
    #[inline]
    pub fn try_map<F, O>(self, f: F) -> Map<'q, DB, F, DB::Arguments<'q>>
    where
        F: FnMut(DB::Row) -> Result<O, sqlx_core::Error> + Send,
        O: Unpin,
    {
        self.to_query().try_map(f)
    }
}
impl<'q, DB> SqlTemplateExecute<'q, DB>
where
    DB: Database,
{
    /// like sqlx_core::Query::execute
    /// Execute the query and return the number of rows affected.
    #[inline]
    pub async fn execute<'e, 'c: 'e, E>(self, executor: E) -> Result<DB::QueryResult, Error>
    where
        'q: 'e,
        DB::Arguments<'q>: 'e,
        E: Executor<'c, Database = DB>,
    {
        executor.execute(self).await
    }
    /// like    sqlx_core::Query::execute_many
    /// Execute multiple queries and return the rows affected from each query, in a stream.
    #[inline]
    pub fn execute_many<'e, 'c: 'e, E>(
        self,
        executor: E,
    ) -> BoxStream<'e, Result<DB::QueryResult, Error>>
    where
        'q: 'e,
        DB::Arguments<'q>: 'e,
        E: Executor<'c, Database = DB>,
    {
        #[allow(deprecated)]
        executor.execute_many(self)
    }
    /// like sqlx_core::Query::fetch
    /// Execute the query and return the generated results as a stream.
    #[inline]
    pub fn fetch<'e, 'c: 'e, E>(self, executor: E) -> BoxStream<'e, Result<DB::Row, Error>>
    where
        'q: 'e,
        DB::Arguments<'q>: 'e,
        E: Executor<'c, Database = DB>,
    {
        executor.fetch(self)
    }
    /// like sqlx_core::Query::fetch_many
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
        DB::Arguments<'q>: 'e,
        E: Executor<'c, Database = DB>,
    {
        #[allow(deprecated)]
        executor.fetch_many(self)
    }
    /// like sqlx_core::Query::fetch_all
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
        DB::Arguments<'q>: 'e,
        E: Executor<'c, Database = DB>,
    {
        executor.fetch_all(self).await
    }
    /// like sqlx_core::Query::fetch_one
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
        DB::Arguments<'q>: 'e,
        E: Executor<'c, Database = DB>,
    {
        executor.fetch_one(self).await
    }
    /// like sqlx_core::Query::fetch_optional
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
        DB::Arguments<'q>: 'e,
        E: Executor<'c, Database = DB>,
    {
        executor.fetch_optional(self).await
    }

    // QueryAs functions wrapp

    /// like sqlx_core::QueryAs::fetch
    /// Execute the query and return the generated results as a stream.
    pub fn fetch_as<'e, 'c: 'e, O, E>(self, executor: E) -> BoxStream<'e, Result<O, Error>>
    where
        'q: 'e,
        DB::Arguments<'q>: 'e,
        E: 'e + Executor<'c, Database = DB>,
        DB: 'e,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
    {
        self.fetch_many_as(executor)
            .try_filter_map(|step| async move { Ok(step.right()) })
            .boxed()
    }
    /// like sqlx_core::QueryAs::fetch_many
    /// Execute multiple queries and return the generated results as a stream
    /// from each query, in a stream.
    pub fn fetch_many_as<'e, 'c: 'e, O, E>(
        self,
        executor: E,
    ) -> BoxStream<'e, Result<Either<DB::QueryResult, O>, Error>>
    where
        'q: 'e,
        DB::Arguments<'q>: 'e,
        E: 'e + Executor<'c, Database = DB>,
        DB: 'e,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
    {
        #[allow(deprecated)]
        executor
            .fetch_many(self)
            .map(|v| match v {
                Ok(Either::Right(row)) => O::from_row(&row).map(Either::Right),
                Ok(Either::Left(v)) => Ok(Either::Left(v)),
                Err(e) => Err(e),
            })
            .boxed()
    }
    /// like sqlx_core::QueryAs::fetch_all
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
        DB::Arguments<'q>: 'e,
        E: 'e + Executor<'c, Database = DB>,
        DB: 'e,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
    {
        self.fetch_as(executor).try_collect().await
    }
    /// like sqlx_core::QueryAs::fetch_one
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
        DB::Arguments<'q>: 'e,
        E: 'e + Executor<'c, Database = DB>,
        DB: 'e,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
    {
        self.fetch_optional_as(executor)
            .await
            .and_then(|row| row.ok_or(sqlx_core::Error::RowNotFound))
    }
    /// like sqlx_core_core::QueryAs::fetch_optional
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
        DB::Arguments<'q>: 'e,
        E: 'e + Executor<'c, Database = DB>,
        DB: 'e,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
    {
        let row = executor.fetch_optional(self).await?;
        if let Some(row) = row {
            O::from_row(&row).map(Some)
        } else {
            Ok(None)
        }
    }
}

impl<'q, DB: Database> Execute<'q, DB> for SqlTemplateExecute<'q, DB> {
    /// Returns the SQL query string
    #[inline]
    fn sql(&self) -> &'q str {
        log::debug!("Executing SQL: {}", self.sql);
        self.sql
    }

    /// Gets prepared statement (not supported in this implementation)
    #[inline]
    fn statement(&self) -> Option<&DB::Statement<'q>> {
        None
    }

    /// Takes ownership of the bound arguments
    #[inline]
    fn take_arguments(
        &mut self,
    ) -> Result<Option<DB::Arguments<'q>>, sqlx_core::error::BoxDynError> {
        Ok(self.arguments.take())
    }

    /// Checks if query is persistent
    #[inline]
    fn persistent(&self) -> bool {
        self.persistent
    }
}
