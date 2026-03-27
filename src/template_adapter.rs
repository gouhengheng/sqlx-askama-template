use std::marker::PhantomData;

use crate::SqlTemplate;
use askama::Result;
use futures_core::{Stream, future::BoxFuture, stream::BoxStream};

use futures_util::{FutureExt, StreamExt, TryFutureExt, TryStreamExt, future};
use sqlx_core::{
    Either, Error, database::Database, encode::Encode, from_row::FromRow, types::Type,
};

use crate::{DatabaseDialect, db_adapter::BackendDB, sql_template_execute::SqlTemplateExecute};

/// Pagination metadata container
#[derive(Debug, PartialEq, Eq)]
pub struct PaginationInfo {
    /// Total number of records
    pub total: i64,
    /// Records per pagination
    pub pagination_size: i64,
    /// Calculated pagination count
    pub pagination_count: i64,
}

impl PaginationInfo {
    /// Constructs new PaginationInfo with automatic pagination count calculation
    ///
    /// # Arguments
    /// * `total` - Total records in dataset
    /// * `pagination_size` - Desired records per pagination
    pub fn new(total: i64, pagination_size: i64) -> PaginationInfo {
        let mut pagination_count = total / pagination_size;
        if total % pagination_size > 0 {
            pagination_count += 1;
        }
        Self {
            total,
            pagination_size,
            pagination_count,
        }
    }
}
/// Database adapter manager handling SQL rendering and execution
///
/// # Generic Parameters
/// - `'q`: Query lifetime
/// - `DB`: Database type
/// - `T`: SQL template type
pub struct DBAdapter<'q, DB, T>
where
    DB: Database,
    T: SqlTemplate<'q, DB>,
{
    pub(crate) template: T,
    persistent: bool,
    _p: PhantomData<&'q DB>,
    pagination_size: Option<i64>,
    pagination_no: Option<i64>,
}

impl<'q, DB, T> DBAdapter<'q, DB, T>
where
    DB: Database,
    T: SqlTemplate<'q, DB>,
{
    /// Creates a new DBAdapter for the SQL template.
    ///
    /// # Arguments
    /// * `template` - SQL template instance
    pub fn new(template: T) -> Self {
        Self {
            template,
            persistent: true,
            pagination_no: None,
            pagination_size: None,
            _p: PhantomData,
        }
    }
}
impl<'q, 'c, 'e, DB, T> DBAdapter<'q, DB, T>
where
    DB: Database + Sync,
    T: SqlTemplate<'q, DB> + Send + 'q,
    i64: Encode<'q, DB> + Type<DB>,
    DB::Arguments: 'q,
    'q: 'e,
    'c: 'e,
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
    pub fn count<Adapter>(self, db_adapter: Adapter) -> BoxFuture<'e, Result<i64, Error>>
    where
        Adapter: BackendDB<'c, DB> + 'c,
        (i64,): for<'r> FromRow<'r, DB::Row>,
    {
        let template = self.template.clone();

        async move {
            let (db_type, executor) = db_adapter.backend_db().await?;
            let f = db_type.placeholder_fn();
            let mut sql = String::new();
            let arg = template.render_with_placeholder(f, &mut sql)?;

            db_type.write_count_sql(&mut sql);
            let execute = SqlTemplateExecute::new(sql, arg).set_persistent(self.persistent);
            let (count,): (i64,) = execute.fetch_one_as(executor).await?;
            Ok(count)
        }
        .boxed()
    }
    /// Calculates complete pagination metadata
    ///
    /// # Arguments
    /// * `pagination_size` - Records per pagination
    /// * `db_adapter` - Database connection adapter
    #[inline]
    pub async fn pagination_info<Adapter>(
        self,
        pagination_size: i64,
        db_adapter: Adapter,
    ) -> Result<PaginationInfo, Error>
    where
        Adapter: BackendDB<'c, DB> + 'c,
        (i64,): for<'r> FromRow<'r, DB::Row>,
    {
        let count = self.count(db_adapter).await?;
        Ok(PaginationInfo::new(count, pagination_size))
    }
    /// Sets pagination parameters
    pub fn set_pagination(mut self, pagination_size: i64, pagination_no: i64) -> Self {
        self.pagination_no = Some(pagination_no);
        self.pagination_size = Some(pagination_size);
        self
    }

    /// like sqlx::Query::execute
    /// Execute the query and return the number of rows affected.
    #[inline]
    pub async fn execute<Adapter>(self, db_adapter: Adapter) -> Result<DB::QueryResult, Error>
    where
        Adapter: BackendDB<'c, DB> + 'c,
    {
        self.execute_many(db_adapter).try_collect().await
    }
    /// like    sqlx::Query::execute_many
    /// Execute multiple queries and return the rows affected from each query, in a stream.
    #[inline]
    pub fn execute_many<Adapter>(
        self,

        db_adapter: Adapter,
    ) -> impl Stream<Item = Result<DB::QueryResult, Error>>
    where
        Adapter: BackendDB<'c, DB> + 'c,
    {
        self.fetch_many(db_adapter)
            .try_filter_map(|step| async move {
                Ok(match step {
                    Either::Left(rows) => Some(rows),
                    Either::Right(_) => None,
                })
            })
    }
    /// like sqlx::Query::fetch
    /// Execute the query and return the generated results as a stream.
    #[inline]
    pub fn fetch<Adapter>(self, db_adapter: Adapter) -> impl Stream<Item = Result<DB::Row, Error>>
    where
        Adapter: BackendDB<'c, DB> + 'c,
    {
        self.fetch_many(db_adapter)
            .try_filter_map(|step| async move {
                Ok(match step {
                    Either::Left(_) => None,
                    Either::Right(row) => Some(row),
                })
            })
    }
    /// like sqlx::Query::fetch_many
    /// Execute multiple queries and return the generated results as a stream.
    ///
    /// For each query in the stream, any generated rows are returned first,
    /// then the `QueryResult` with the number of rows affected.
    #[inline]
    #[allow(clippy::type_complexity)]
    pub fn fetch_many<Adapter>(
        self,
        db_adapter: Adapter,
    ) -> BoxStream<'e, Result<Either<DB::QueryResult, DB::Row>, Error>>
    where
        Adapter: BackendDB<'c, DB> + 'c,
    {
        let template = self.template.clone();
        let pagination_no = self.pagination_no;
        let pagination_size = self.pagination_size;
        Box::pin(async_stream::try_stream! {
            let (db_type, executor) = db_adapter.backend_db().await?;
            let f = db_type.placeholder_fn();
            let mut sql = String::new();
            let mut arg = template.render_with_placeholder(f, &mut sql)?;

            if let (Some(pagination_no), Some(pagination_size)) = (pagination_no, pagination_size) {
                let mut args = arg.unwrap_or_default();
                db_type.write_pagination_sql(&mut sql, pagination_size, pagination_no, &mut args)?;
                arg = Some(args);
            }

            let execute = SqlTemplateExecute::new(sql, arg).set_persistent(self.persistent);
            let mut stream = execute.fetch_many(executor);
            while let Some(item) = stream.try_next().await? {
                yield item;
            }
        })
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
    pub async fn fetch_all<Adapter>(self, db_adapter: Adapter) -> Result<Vec<DB::Row>, Error>
    where
        Adapter: BackendDB<'c, DB> + 'c,
    {
        self.fetch(db_adapter).try_collect().await
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
    pub async fn fetch_one<Adapter>(self, db_adapter: Adapter) -> Result<DB::Row, Error>
    where
        Adapter: BackendDB<'c, DB> + 'c,
    {
        self.fetch_optional(db_adapter)
            .and_then(|row| match row {
                Some(row) => future::ok(row),
                None => future::err(Error::RowNotFound),
            })
            .await
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
    pub async fn fetch_optional<Adapter>(
        self,

        db_adapter: Adapter,
    ) -> Result<Option<DB::Row>, Error>
    where
        Adapter: BackendDB<'c, DB> + 'c,
    {
        let row = self.fetch_many(db_adapter).try_next().await?;
        match row {
            Some(Either::Right(row)) => Ok(Some(row)),
            Some(Either::Left(_)) => Ok(None),
            None => Ok(None),
        }
    }

    /// like sqlx::QueryAs::fetch
    /// Execute the query and return the generated results as a stream.
    pub async fn fetch_as<Adapter, O>(
        self,

        db_adapter: Adapter,
    ) -> impl Stream<Item = Result<O, Error>>
    where
        Adapter: BackendDB<'c, DB> + 'c,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
    {
        self.fetch_many_as(db_adapter)
            .try_filter_map(|step| async move { Ok(step.right()) })
    }
    /// like sqlx::QueryAs::fetch_many
    /// Execute multiple queries and return the generated results as a stream
    /// from each query, in a stream.
    pub fn fetch_many_as<Adapter, O>(
        self,
        db_adapter: Adapter,
    ) -> BoxStream<'e, Result<Either<DB::QueryResult, O>, Error>>
    where
        'q: 'e,
        Adapter: BackendDB<'c, DB> + 'c,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
    {
        let template = self.template.clone();
        let pagination_no = self.pagination_no;
        let pagination_size = self.pagination_size;
        Box::pin(async_stream::try_stream! {
        let (db_type, executor) = db_adapter.backend_db().await?;
        let f = db_type.placeholder_fn();
        let mut sql = String::new();
        let mut arg = template.render_with_placeholder(f, &mut sql)?;

        if let (Some(pagination_no), Some(pagination_size)) = (pagination_no, pagination_size) {
            let mut args = arg.unwrap_or_default();
            db_type.write_pagination_sql(&mut sql, pagination_size, pagination_no, &mut args)?;
            arg = Some(args);
        }

        let execute = SqlTemplateExecute::new(sql, arg).set_persistent(self.persistent);
        let mut stream = execute.fetch_many(executor).map(|v| match v {
            Ok(Either::Right(row)) => O::from_row(&row).map(Either::Right),
            Ok(Either::Left(v)) => Ok(Either::Left(v)),
            Err(e) => Err(e),
        });
        while let Some(item) = stream.try_next().await? {
            yield item;
        }
          })
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
    pub async fn fetch_all_as<Adapter, O>(self, db_adapter: Adapter) -> Result<Vec<O>, Error>
    where
        Adapter: BackendDB<'c, DB> + 'c,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
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
    pub async fn fetch_one_as<Adapter, O>(self, db_adapter: Adapter) -> Result<O, Error>
    where
        Adapter: BackendDB<'c, DB> + 'c,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row> + 'e,
    {
        self.fetch_optional_as(db_adapter)
            .and_then(|o| match o {
                Some(o) => future::ok(o),
                None => future::err(Error::RowNotFound),
            })
            .await
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
    pub async fn fetch_optional_as<Adapter, O>(
        self,

        db_adapter: Adapter,
    ) -> Result<Option<O>, Error>
    where
        Adapter: BackendDB<'c, DB> + 'c,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row>,
    {
        let row = self.fetch_many_as(db_adapter).try_next().await?;
        match row {
            Some(Either::Right(o)) => Ok(Some(o)),
            Some(Either::Left(_)) => Ok(None),
            None => Ok(None),
        }
    }
}
