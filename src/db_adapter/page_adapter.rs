use askama::Result;
use futures_core::stream::BoxStream;

use futures_util::{StreamExt, stream};
use sqlx::{Database, Either, Encode, Executor, FromRow, Type};

use crate::{Error, SqlTemplate, SqlTemplateExecute};

use super::{DBAdapter, DBAdapterManager};

#[derive(Debug)]
pub struct PageInfo {
    pub total: i64,
    pub page_size: i64,
    pub page_count: i64,
}

impl PageInfo {
    pub(crate) fn new(total: i64, page_size: i64) -> PageInfo {
        let mut page_count = total / page_size;
        if total % page_size > 0 {
            page_count += 1;
        }
        Self {
            total,
            page_size,
            page_count,
        }
    }
}
pub struct PageQuery<'q, DB, T>
where
    i64: for<'q1> Encode<'q1, DB> + Type<DB>,
    DB: Database,
    T: SqlTemplate<'q, DB>,
{
    adapter: DBAdapterManager<'q, DB, T>,
    page_size: i64,
    page_no: i64,
    persistent: bool,
}

impl<'q, 'c, DB, T> PageQuery<'q, DB, T>
where
    DB: Database,
    i64: for<'r> Encode<'r, DB> + Type<DB>,
    T: SqlTemplate<'q, DB>,
{
    pub(crate) fn new(adapter: DBAdapterManager<'q, DB, T>, page_size: i64, page_no: i64) -> Self {
        Self {
            adapter,

            page_no,
            page_size,
            persistent: true,
        }
    }
    pub fn set_persistent(mut self, persistent: bool) -> Self {
        self.persistent = persistent;
        self
    }
    pub async fn render_page_sql<Adapter>(
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
        Adapter: DBAdapter<'c, DB>,
    {
        let (f, db_adapter) = db_adapter.get_encode_placeholder_fn().await?;
        let (sql, arg) = self
            .adapter
            .template
            .render_sql_with_encode_placeholder_fn(f)?;
        let arg = arg.unwrap_or_default();
        let (sql, arg, executor) = db_adapter
            .write_page_sql(sql, self.page_size, self.page_no, f, arg)
            .await?;
        *self.adapter.sql_buff = sql;
        let execute = SqlTemplateExecute::new(&*self.adapter.sql_buff, Some(arg))
            .set_persistent(self.persistent);

        Ok((execute, executor))
    }
    /// like sqlx::Query::fetch
    /// Execute the query and return the generated results as a stream.
    #[inline]
    pub async fn fetch<'e, Adapter>(
        self,
        db_adapter: Adapter,
    ) -> BoxStream<'e, Result<DB::Row, Error>>
    where
        'q: 'e,
        'c: 'e,
        Adapter: DBAdapter<'c, DB>,
    {
        let res = self.render_page_sql(db_adapter).await;
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
    pub async fn fetch_many<'e, Adapter>(
        self,
        db_adapter: Adapter,
    ) -> BoxStream<'e, Result<Either<DB::QueryResult, DB::Row>, Error>>
    where
        'q: 'e,
        'c: 'e,
        Adapter: DBAdapter<'c, DB>,
    {
        let res = self.render_page_sql(db_adapter).await;
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
    pub async fn fetch_all<'e, Adapter>(self, db_adapter: Adapter) -> Result<Vec<DB::Row>, Error>
    where
        'q: 'e,
        'c: 'e,
        Adapter: DBAdapter<'c, DB>,
    {
        let (execute, executor) = self.render_page_sql(db_adapter).await?;
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
    pub async fn fetch_one<'e, Adapter>(self, db_adapter: Adapter) -> Result<DB::Row, Error>
    where
        'q: 'e,
        'c: 'e,
        Adapter: DBAdapter<'c, DB>,
    {
        let (execute, executor) = self.render_page_sql(db_adapter).await?;
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
    pub async fn fetch_optional<'e, Adapter>(
        self,
        db_adapter: Adapter,
    ) -> Result<Option<DB::Row>, Error>
    where
        'q: 'e,
        'c: 'e,
        Adapter: DBAdapter<'c, DB>,
    {
        let (execute, executor) = self.render_page_sql(db_adapter).await?;
        execute.fetch_optional(executor).await
    }

    // QueryAs functions wrapp

    /// like sqlx::QueryAs::fetch
    /// Execute the query and return the generated results as a stream.
    pub async fn fetch_as<'e, O, Adapter>(
        self,
        db_adapter: Adapter,
    ) -> BoxStream<'e, Result<O, Error>>
    where
        'q: 'e,
        'c: 'e,
        DB: 'e,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
        Adapter: DBAdapter<'c, DB>,
    {
        let res = self.render_page_sql(db_adapter).await;
        match res {
            Ok((execute, executor)) => execute.fetch_as(executor),
            Err(e) => stream::once(async move { Err(e) }).boxed(),
        }
    }
    /// like sqlx::QueryAs::fetch_many
    /// Execute multiple queries and return the generated results as a stream
    /// from each query, in a stream.
    pub async fn fetch_many_as<'e, O, Adapter>(
        self,
        db_adapter: Adapter,
    ) -> BoxStream<'e, Result<Either<DB::QueryResult, O>, Error>>
    where
        'q: 'e,
        'c: 'e,
        DB: 'e,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
        Adapter: DBAdapter<'c, DB>,
    {
        let res = self.render_page_sql(db_adapter).await;
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
    pub async fn fetch_all_as<'e, O, Adapter>(self, db_adapter: Adapter) -> Result<Vec<O>, Error>
    where
        'q: 'e,
        'c: 'e,
        DB: 'e,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
        Adapter: DBAdapter<'c, DB>,
    {
        let (execute, executor) = self.render_page_sql(db_adapter).await?;
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
    pub async fn fetch_one_as<'e, O, Adapter>(self, db_adapter: Adapter) -> Result<O, Error>
    where
        'q: 'e,
        'c: 'e,
        DB: 'e,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
        Adapter: DBAdapter<'c, DB>,
    {
        let (execute, executor) = self.render_page_sql(db_adapter).await?;
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
    ///
    ///
    pub async fn fetch_optional_as<'e, O, Adapter>(
        self,
        db_adapter: Adapter,
    ) -> Result<Option<O>, Error>
    where
        'q: 'e,
        DB: 'e,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
        Adapter: DBAdapter<'c, DB>,
    {
        let (execute, executor) = self.render_page_sql(db_adapter).await?;
        execute.fetch_optional_as(executor).await
    }
}
