use std::marker::PhantomData;

use askama::Result;
use futures_core::stream::BoxStream;
use futures_util::{StreamExt, stream};
use sqlx::{Database, Either, Encode, Executor, FromRow, prelude::Type};

use crate::{Error, SqlTemplate, SqlTemplateExecute};

use super::{DBAdapter, PageInfo, PageQuery};

pub struct DBAdapterManager<'q, DB, T>
where
    DB: Database,
    T: SqlTemplate<'q, DB>,
{
    pub(crate) sql_buff: &'q mut String,
    pub(crate) template: T,
    persistent: bool,
    _p: PhantomData<&'q DB>,
}
impl<'q, DB, T> DBAdapterManager<'q, DB, T>
where
    DB: Database,
    T: SqlTemplate<'q, DB>,
{
    pub fn new(template: T, sql_buff: &'q mut String) -> Self {
        Self {
            sql_buff,
            template,
            persistent: true,
            _p: PhantomData,
        }
    }
}
impl<'q, DB, T> DBAdapterManager<'q, DB, T>
where
    DB: Database,
    T: SqlTemplate<'q, DB>,
{
    pub fn set_persistent(mut self, persistent: bool) -> Self {
        self.persistent = persistent;
        self
    }
    #[inline]
    pub async fn count<'c, Adapter>(self, db_adapter: Adapter) -> Result<i64, Error>
    where
        Adapter: DBAdapter<'c, DB>,
        (i64,): for<'r> FromRow<'r, DB::Row>,
    {
        let (f, db_adapter) = db_adapter.get_encode_placeholder_fn().await?;

        let (sql, arg) = self.template.render_sql_with_encode_placeholder_fn(f)?;

        let (sql, executor) = db_adapter.write_count_sql(sql).await?;
        *self.sql_buff = sql;
        let execute = SqlTemplateExecute::new(&*self.sql_buff, arg).set_persistent(self.persistent);

        let (count,): (i64,) = execute.fetch_one_as(executor).await?;
        Ok(count)
    }
    #[inline]
    pub async fn count_page<'c, Adapter>(
        self,

        page_size: i64,
        db_adapter: Adapter,
    ) -> Result<PageInfo, Error>
    where
        Adapter: DBAdapter<'c, DB>,
        (i64,): for<'r> FromRow<'r, DB::Row>,
    {
        let count = self.count(db_adapter).await?;

        Ok(PageInfo::new(count, page_size))
    }
    #[inline]
    pub fn to_page_query<'c>(self, page_size: i64, page_no: i64) -> PageQuery<'q, DB, T>
    where
        i64: for<'q1> Encode<'q1, DB> + Type<DB>,
    {
        PageQuery::new(self, page_size, page_no)
    }
    #[inline]
    pub async fn render_adapter_sql<'c, Adapter>(
        self,

        db_adapter: Adapter,
    ) -> Result<
        (
            SqlTemplateExecute<'q, DB>,
            impl Executor<'c, Database = DB> + 'c,
        ),
        Error,
    >
    where
        'q: 'c,
        Adapter: DBAdapter<'c, DB>,
    {
        let (f, db_adapter) = db_adapter.get_encode_placeholder_fn().await?;
        let (sql, arg) = self.template.render_sql_with_encode_placeholder_fn(f)?;
        *self.sql_buff = sql;
        let execute = SqlTemplateExecute::new(&*self.sql_buff, arg).set_persistent(self.persistent);
        //  let executor = db_adapter.get_executor().await?;
        Ok((execute, db_adapter))
    }
    /// like sqlx::Query::execute
    /// Execute the query and return the number of rows affected.
    #[inline]
    pub async fn execute<'c, Adapter>(self, db_adapter: Adapter) -> Result<DB::QueryResult, Error>
    where
        'q: 'c,
        Adapter: DBAdapter<'c, DB>,
    {
        let (execute, executor) = self.render_adapter_sql(db_adapter).await?;
        execute.execute(executor).await
    }
    /// like    sqlx::Query::execute_many
    /// Execute multiple queries and return the rows affected from each query, in a stream.
    #[inline]
    pub async fn execute_many<'e, 'c, Adapter>(
        self,

        db_adapter: Adapter,
    ) -> BoxStream<'e, Result<DB::QueryResult, Error>>
    where
        //'q: 'e,
        'q: 'c,
        'c: 'e,
        Adapter: DBAdapter<'c, DB>,
    {
        let res = self.render_adapter_sql(db_adapter).await;
        match res {
            Ok((execute, executor)) => execute.execute_many(executor).await,
            Err(e) => stream::once(async move { Err(e) }).boxed(),
        }
    }
    /// like sqlx::Query::fetch
    /// Execute the query and return the generated results as a stream.
    #[inline]
    pub async fn fetch<'e, 'c, Adapter>(
        self,

        db_adapter: Adapter,
    ) -> BoxStream<'e, Result<DB::Row, Error>>
    where
        'q: 'c,
        'c: 'e,
        Adapter: DBAdapter<'c, DB>,
    {
        let res = self.render_adapter_sql(db_adapter).await;

        match res {
            Ok((execute, executor)) => execute.fetch(executor),
            Err(e) => stream::once(async move { Err(e) }).boxed(),
        }
    }
    /// like sqlx::Query::fetch_many
    /// Execute multiple queries and return the generated results as a stream.
    ///
    /// For each query in the stream, any generated rows are returned first,
    /// then the `QueryResult` with the number of rows affected.
    #[inline]
    #[allow(clippy::type_complexity)]
    pub async fn fetch_many<'e, 'c, Adapter>(
        self,

        db_adapter: Adapter,
    ) -> BoxStream<'e, Result<Either<DB::QueryResult, DB::Row>, Error>>
    where
        'q: 'c,
        'c: 'e,
        Adapter: DBAdapter<'c, DB>,
    {
        let res = self.render_adapter_sql(db_adapter).await;

        match res {
            Ok((execute, executor)) => execute.fetch_many(executor),
            Err(e) => stream::once(async move { Err(e) }).boxed(),
        }
    }
    /// like sqlx::Query::fetch_all
    /// Execute the query and return all the resulting rows collected into a [`Vec`].
    ///
    /// ### Note: beware result set size.
    /// This will attempt to collect the full result set of the query into memory.
    ///
    /// To avoid exhausting available memory, ensure the result set has a known upper bound,
    /// e.g. using `LIMIT`.
    #[inline]
    pub async fn fetch_all<'c, Adapter>(self, db_adapter: Adapter) -> Result<Vec<DB::Row>, Error>
    where
        'q: 'c,
        Adapter: DBAdapter<'c, DB>,
    {
        let (execute, executor) = self.render_adapter_sql(db_adapter).await?;
        execute.fetch_all(executor).await
    }
    /// like sqlx::Query::fetch_one
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
    pub async fn fetch_one<'c, Adapter>(self, db_adapter: Adapter) -> Result<DB::Row, Error>
    where
        'q: 'c,
        Adapter: DBAdapter<'c, DB>,
    {
        let (execute, executor) = self.render_adapter_sql(db_adapter).await?;
        execute.fetch_one(executor).await
    }
    /// like sqlx::Query::fetch_optional
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
    pub async fn fetch_optional<'c, Adapter>(
        self,

        db_adapter: Adapter,
    ) -> Result<Option<DB::Row>, Error>
    where
        'q: 'c,
        Adapter: DBAdapter<'c, DB>,
    {
        let (execute, executor) = self.render_adapter_sql(db_adapter).await?;
        execute.fetch_optional(executor).await
    }

    /// like sqlx::QueryAs::fetch
    /// Execute the query and return the generated results as a stream.
    pub async fn fetch_as<'e, 'c, Adapter, O>(
        self,

        db_adapter: Adapter,
    ) -> BoxStream<'e, Result<O, Error>>
    where
        'c: 'e,
        'q: 'c,
        Adapter: DBAdapter<'c, DB>,
        DB: 'e,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
    {
        let res = self.render_adapter_sql(db_adapter).await;

        match res {
            Ok((execute, executor)) => execute.fetch_as(executor),

            Err(e) => stream::once(async move { Err(e) }).boxed(),
        }
    }
    /// like sqlx::QueryAs::fetch_many
    /// Execute multiple queries and return the generated results as a stream
    /// from each query, in a stream.
    pub async fn fetch_many_as<'e, 'c, Adapter, O>(
        self,

        db_adapter: Adapter,
    ) -> BoxStream<'e, Result<Either<DB::QueryResult, O>, Error>>
    where
        'q: 'c,
        DB: 'e,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
        Adapter: DBAdapter<'c, DB>,
        'c: 'e,
    {
        let res = self.render_adapter_sql(db_adapter).await;
        match res {
            Ok((execute, executor)) => execute.fetch_many_as(executor),
            Err(e) => stream::once(async move { Err(e) }).boxed(),
        }
    }
    /// like sqlx::QueryAs::fetch_all
    /// Execute the query and return all the resulting rows collected into a [`Vec`].
    ///
    /// ### Note: beware result set size.
    /// This will attempt to collect the full result set of the query into memory.
    ///
    /// To avoid exhausting available memory, ensure the result set has a known upper bound,
    /// e.g. using `LIMIT`.
    #[inline]
    pub async fn fetch_all_as<'e, 'c, Adapter, O>(
        self,

        db_adapter: Adapter,
    ) -> Result<Vec<O>, Error>
    where
        'q: 'e,
        'q: 'c,
        DB: 'e,
        Adapter: DBAdapter<'c, DB>,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
    {
        let (execute, executor) = self.render_adapter_sql(db_adapter).await?;
        execute.fetch_all_as(executor).await
    }
    /// like sqlx::QueryAs::fetch_one
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
    pub async fn fetch_one_as<'e, 'c, Adapter, O>(self, db_adapter: Adapter) -> Result<O, Error>
    where
        'q: 'e,
        'q: 'c,
        DB: 'e,
        Adapter: DBAdapter<'c, DB>,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
    {
        let (execute, executor) = self.render_adapter_sql(db_adapter).await?;
        execute.fetch_one_as(executor).await
    }
    /// like sqlx::QueryAs::fetch_optional
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
    pub async fn fetch_optional_as<'e, 'c, Adapter, O>(
        self,

        db_adapter: Adapter,
    ) -> Result<Option<O>, Error>
    where
        'q: 'e,
        'q: 'c,
        DB: 'e,
        Adapter: DBAdapter<'c, DB>,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
    {
        let (execute, executor) = self.render_adapter_sql(db_adapter).await?;
        execute.fetch_optional_as(executor).await
    }
}
