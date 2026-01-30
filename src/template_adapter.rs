use std::marker::PhantomData;

use askama::Result;
use futures_core::stream::BoxStream;
use futures_util::{StreamExt, TryStreamExt, stream};

use crate::SqlTemplate;
use sqlx_core::{
    Either, Error, database::Database, encode::Encode, executor::Executor, from_row::FromRow,
    types::Type,
};

use super::{DatabaseDialect, db_adapter::BackendDB, sql_template_execute::SqlTemplateExecute};

/// Pagination metadata container
#[derive(Debug)]
pub struct PageInfo {
    /// Total number of records
    pub total: i64,
    /// Records per page
    pub page_size: i64,
    /// Calculated page count
    pub page_count: i64,
}

impl PageInfo {
    /// Constructs new PageInfo with automatic page count calculation
    ///
    /// # Arguments
    /// * `total` - Total records in dataset
    /// * `page_size` - Desired records per page
    pub fn new(total: i64, page_size: i64) -> PageInfo {
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
/// Database adapter manager handling SQL rendering and execution
///
/// # Generic Parameters
/// - `'q`: Query lifetime
/// - `DB`: Database type
/// - `T`: SQL template type
pub struct DBAdapterManager<'s, DB, T>
where
    DB: Database,
    T: SqlTemplate<'s, DB>,
{
    pub(crate) sql: String,
    pub(crate) template: T,
    persistent: bool,
    _p: PhantomData<&'s DB>,
    page_size: Option<i64>,
    page_no: Option<i64>,
}

impl<'q, DB, T> DBAdapterManager<'q, DB, T>
where
    DB: Database,
    T: SqlTemplate<'q, DB>,
{
    /// Creates new adapter with SQL buffer
    ///
    /// # Arguments
    /// * `template` - SQL template instance
    pub fn new(template: T) -> Self {
        Self {
            sql: String::new(),
            template,
            persistent: true,
            page_no: None,
            page_size: None,
            _p: PhantomData,
        }
    }

    pub fn sql(&self) -> &String {
        &self.sql
    }
}
impl<'q, 's, DB, T> DBAdapterManager<'s, DB, T>
where
    DB: Database,
    T: SqlTemplate<'s, DB>,
    i64: Encode<'q, DB> + Type<DB>,
{
    /// Configures query persistence (default: true)
    pub fn set_persistent(mut self, persistent: bool) -> Self {
        self.persistent = persistent;
        self
    }
    /// Executes count query for pagination
    ///
    /// # Arguments
    /// * `db_adapter` - Database connection adapter
    #[inline]
    pub async fn count<'c, Adapter>(mut self, db_adapter: Adapter) -> Result<i64, Error>
    where
        Adapter: BackendDB<'c, DB>,
        (i64,): for<'r> FromRow<'r, DB::Row>,
    {
        let (mut sql, arg, db_type, executor) = Self::render_sql_with_adapter(
            self.template.clone(),
            db_adapter,
            self.page_no,
            self.page_size,
        )
        .await?;

        db_type.write_count_sql(&mut sql);
        self.sql = sql;
        let execute = SqlTemplateExecute::new(self.sql, arg).set_persistent(self.persistent);
        let (count,): (i64,) = execute.fetch_one_as(executor).await?;
        Ok(count)
    }
    /// Calculates complete pagination metadata
    ///
    /// # Arguments
    /// * `page_size` - Records per page
    /// * `db_adapter` - Database connection adapter
    #[inline]
    pub async fn count_page<'c, Adapter>(
        self,

        page_size: i64,
        db_adapter: Adapter,
    ) -> Result<PageInfo, Error>
    where
        Adapter: BackendDB<'c, DB>,
        (i64,): for<'r> FromRow<'r, DB::Row>,
    {
        let count = self.count(db_adapter).await?;

        Ok(PageInfo::new(count, page_size))
    }
    /// Sets pagination parameters
    pub fn set_page(mut self, page_size: i64, page_no: i64) -> Self {
        self.page_no = Some(page_no);
        self.page_size = Some(page_size);
        self
    }
    /// Core SQL rendering method with pagination support
    #[inline]
    pub async fn render_sql_with_adapter<'c, Adapter>(
        template: T,

        db_adapter: Adapter,
        page_no: Option<i64>,
        page_size: Option<i64>,
    ) -> Result<
        (
            String,
            Option<DB::Arguments>,
            impl DatabaseDialect,
            impl Executor<'c, Database = DB>,
        ),
        Error,
    >
    where
        Adapter: BackendDB<'c, DB>,
    {
        let (db_type, executor) = db_adapter.backend_db().await?;
        let f = db_type.get_encode_placeholder_fn();
        let mut sql = String::new();
        let mut arg = template.render_sql_with_encode_placeholder_fn(f, &mut sql)?;

        if let (Some(page_no), Some(page_size)) = (page_no, page_size) {
            let mut args = arg.unwrap_or_default();
            db_type.write_page_sql(&mut sql, page_size, page_no, &mut args)?;
            arg = Some(args);
        }
        Ok((sql, arg, db_type, executor))
    }

    /// like sqlx::Query::execute
    /// Execute the query and return the number of rows affected.
    #[inline]
    pub async fn execute<'c, Adapter>(self, db_adapter: Adapter) -> Result<DB::QueryResult, Error>
    where
        Adapter: BackendDB<'c, DB>,
    {
        self.execute_many(db_adapter).await.try_collect().await
    }
    /// like    sqlx::Query::execute_many
    /// Execute multiple queries and return the rows affected from each query, in a stream.
    #[inline]
    pub async fn execute_many<'c, 'e, Adapter>(
        self,

        db_adapter: Adapter,
    ) -> BoxStream<'e, Result<DB::QueryResult, Error>>
    where
        'c: 'e,
        'q: 'e,
        Adapter: BackendDB<'c, DB>,
    {
        self.fetch_many(db_adapter)
            .await
            .try_filter_map(|step| async move {
                Ok(match step {
                    Either::Left(rows) => Some(rows),
                    Either::Right(_) => None,
                })
            })
            .boxed()
    }
    /// like sqlx::Query::fetch
    /// Execute the query and return the generated results as a stream.
    #[inline]
    pub async fn fetch<'c, 'e, Adapter>(
        self,

        db_adapter: Adapter,
    ) -> BoxStream<'e, Result<DB::Row, Error>>
    where
        'c: 'e,
        'q: 'e,
        Adapter: BackendDB<'c, DB>,
    {
        self.fetch_many(db_adapter)
            .await
            .try_filter_map(|step| async move {
                Ok(match step {
                    Either::Left(_) => None,
                    Either::Right(row) => Some(row),
                })
            })
            .boxed()
    }
    /// like sqlx::Query::fetch_many
    /// Execute multiple queries and return the generated results as a stream.
    ///
    /// For each query in the stream, any generated rows are returned first,
    /// then the `QueryResult` with the number of rows affected.
    #[inline]
    pub async fn fetch_many<'c, 'e, Adapter>(
        mut self,
        db_adapter: Adapter,
    ) -> BoxStream<'e, Result<Either<DB::QueryResult, DB::Row>, Error>>
    where
        'c: 'e,
        'q: 'e,
        Adapter: BackendDB<'c, DB>,
    {
        let res = Self::render_sql_with_adapter(
            self.template.clone(),
            db_adapter,
            self.page_no,
            self.page_size,
        )
        .await;

        match res {
            Ok((sql, arg, _db_type, executor)) => {
                self.sql = sql;
                let execute =
                    SqlTemplateExecute::new(self.sql, arg).set_persistent(self.persistent);
                executor.fetch_many(execute)
            }
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
        Adapter: BackendDB<'c, DB>,
    {
        self.fetch(db_adapter).await.try_collect().await
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
        Adapter: BackendDB<'c, DB>,
    {
        self.fetch_optional(db_adapter)
            .await
            .and_then(|row| match row {
                Some(row) => Ok(row),
                None => Err(Error::RowNotFound),
            })
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
        Adapter: BackendDB<'c, DB>,
    {
        self.fetch(db_adapter).await.try_next().await
    }

    /// like sqlx::QueryAs::fetch
    /// Execute the query and return the generated results as a stream.
    pub async fn fetch_as<'c, 'e, Adapter, O>(
        self,

        db_adapter: Adapter,
    ) -> BoxStream<'e, Result<O, Error>>
    where
        'c: 'e,
        'q: 'e,
        Adapter: BackendDB<'c, DB>,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
    {
        self.fetch_many_as(db_adapter)
            .await
            .try_filter_map(|step| async move { Ok(step.right()) })
            .boxed()
    }
    /// like sqlx::QueryAs::fetch_many
    /// Execute multiple queries and return the generated results as a stream
    /// from each query, in a stream.
    pub async fn fetch_many_as<'c, 'e, Adapter, O>(
        self,
        db_adapter: Adapter,
    ) -> BoxStream<'e, Result<Either<DB::QueryResult, O>, Error>>
    where
        'c: 'e,
        'q: 'e,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
        Adapter: BackendDB<'c, DB>,
    {
        self.fetch_many(db_adapter)
            .await
            .map(|v| match v {
                Ok(Either::Right(row)) => O::from_row(&row).map(Either::Right),
                Ok(Either::Left(v)) => Ok(Either::Left(v)),
                Err(e) => Err(e),
            })
            .boxed()
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
    pub async fn fetch_all_as<'c, Adapter, O>(self, db_adapter: Adapter) -> Result<Vec<O>, Error>
    where
        Adapter: BackendDB<'c, DB>,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row>,
    {
        self.fetch_as(db_adapter).await.try_collect().await
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
    pub async fn fetch_one_as<'c, Adapter, O>(self, db_adapter: Adapter) -> Result<O, Error>
    where
        Adapter: BackendDB<'c, DB>,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row>,
    {
        self.fetch_optional_as(db_adapter)
            .await
            .and_then(|row| row.ok_or(Error::RowNotFound))
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
    pub async fn fetch_optional_as<'c, Adapter, O>(
        self,

        db_adapter: Adapter,
    ) -> Result<Option<O>, Error>
    where
        Adapter: BackendDB<'c, DB>,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row>,
    {
        let row = self.fetch_optional(db_adapter).await?;
        if let Some(row) = row {
            O::from_row(&row).map(Some)
        } else {
            Ok(None)
        }
    }
}
