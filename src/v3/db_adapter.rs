use std::{any::Any, marker::PhantomData, ops::Deref};

use futures_util::TryStreamExt;
use sqlx_core::{
    Either, Error,
    any::{AnyConnection, AnyPool},
    arguments::Arguments,
    database::Database,
    describe::Describe,
    encode::Encode,
    executor::{Execute, Executor},
    pool::PoolConnection,
    try_stream,
    types::Type,
};
/// Abstracts SQL dialect differences across database systems
///
/// Provides a unified interface for handling database-specific SQL syntax variations,
/// particularly for parameter binding, count queries, and pagination.
pub trait DatabaseDialect {
    fn backend_name(&self) -> &str;
    /// Gets placeholder generation function for parameter binding
    ///
    /// Database-specific placeholder formats:
    /// - PostgreSQL: $1, $2...
    /// - MySQL/SQLite: ?
    ///
    /// # Returns
    /// Option<fn(usize, &mut String)> placeholder generation function
    fn get_encode_placeholder_fn(&self) -> Option<fn(usize, &mut String)>;
    /// Wraps SQL in count query
    ///
    /// # Arguments
    /// * `sql` - Original SQL to modify
    fn write_count_sql(&self, sql: &mut String);
    /// Generates pagination SQL clause
    ///
    /// # Arguments
    /// * `sql` - Original SQL statement to modify
    /// * `page_size` - Items per page
    /// * `page_no` - Page number (auto-corrected to >=1)
    /// * `arg` - SQL arguments container
    ///
    /// # Note
    /// Automatically handles invalid page numbers
    fn write_page_sql<'c, 'q, DB>(
        &self,
        sql: &mut String,
        page_size: i64,
        page_no: i64,
        arg: &mut DB::Arguments<'q>,
    ) -> Result<(), Error>
    where
        DB: Database,
        i64: Encode<'q, DB> + Type<DB>;
}

/// Database type enumeration supporting major database systems
#[derive(Debug, PartialEq)]
pub enum DBType {
    /// PostgreSQL database
    PostgreSQL,
    /// MySQL database
    MySQL,
    /// SQLite database
    SQLite,
}
impl DBType {
    /// Creates a DBType instance from database name
    ///
    /// # Arguments
    /// * `db_name` - Database identifier ("PostgreSQL"|"MySQL"|"SQLite")
    ///
    /// # Errors
    /// Returns Error::Protocol for unsupported database types
    ///
    /// # Example
    /// ```
    /// let db_type = DBType::new("PostgreSQL")?;
    /// ```
    pub fn new(db_name: &str) -> Result<Self, Error> {
        match db_name {
            "PostgreSQL" => Ok(Self::PostgreSQL),
            "MySQL" => Ok(Self::MySQL),
            "SQLite" => Ok(Self::SQLite),
            _ => Err(Error::Protocol(format!("unsupport db `{}`", db_name))),
        }
    }
}

impl DatabaseDialect for DBType {
    fn backend_name(&self) -> &str {
        match self {
            Self::PostgreSQL => "PostgreSQL",
            Self::MySQL => "MySQL",
            Self::SQLite => "SQLite",
        }
    }
    /// Gets placeholder generation function for parameter binding
    ///
    /// Database-specific placeholder formats:
    /// - PostgreSQL: $1, $2...
    /// - MySQL/SQLite: ?
    ///
    /// # Returns
    /// Option<fn(usize, &mut String)> placeholder generation function
    fn get_encode_placeholder_fn(&self) -> Option<fn(usize, &mut String)> {
        match self {
            Self::PostgreSQL => Some(|i: usize, s: &mut String| s.push_str(&format!("${}", i))),
            Self::MySQL | Self::SQLite => Some(|_: usize, s: &mut String| s.push('?')),
        }
    }
    /// Wraps SQL in count query
    ///
    /// # Arguments
    /// * `sql` - Original SQL to modify
    fn write_count_sql(&self, sql: &mut String) {
        match self {
            Self::PostgreSQL | DBType::MySQL | DBType::SQLite => {
                pg_mysql_sqlite_count_sql(sql);
            }
        }
    }
    /// Generates pagination SQL clause
    ///
    /// # Arguments
    /// * `sql` - Original SQL statement to modify
    /// * `page_size` - Items per page
    /// * `page_no` - Page number (auto-corrected to >=1)
    /// * `arg` - SQL arguments container
    ///
    /// # Note
    /// Automatically handles invalid page numbers
    fn write_page_sql<'c, 'q, DB>(
        &self,
        sql: &mut String,
        page_size: i64,
        page_no: i64,

        arg: &mut DB::Arguments<'q>,
    ) -> Result<(), Error>
    where
        DB: Database,
        i64: Encode<'q, DB> + Type<DB>,
    {
        let f = self.get_encode_placeholder_fn();
        match self {
            Self::PostgreSQL | DBType::MySQL | DBType::SQLite => {
                pg_mysql_sqlite_page_sql(sql, page_size, page_no, f, arg)?;
                Ok(())
            }
        }
    }
}
fn pg_mysql_sqlite_count_sql(sql: &mut String) {
    *sql = format!("select count(1) from ({}) t", sql)
}
fn pg_mysql_sqlite_page_sql<'c, 'q, DB>(
    sql: &mut String,
    mut page_size: i64,
    mut page_no: i64,
    f: Option<fn(usize, &mut String)>,
    arg: &mut DB::Arguments<'q>,
) -> Result<(), Error>
where
    DB: Database,
    i64: Encode<'q, DB> + Type<DB>,
{
    if page_size < 1 {
        page_size = 1
    }
    if page_no < 1 {
        page_no = 1
    }
    let offset = (page_no - 1) * page_size;
    if let Some(f) = f {
        sql.push_str(" limit ");
        arg.add(page_size).map_err(Error::Encode)?;
        f(arg.len(), sql);
        sql.push_str(" offset ");
        arg.add(offset).map_err(Error::Encode)?;
        f(arg.len(), sql);
    } else {
        sql.push_str(" limit ");
        arg.add(page_size).map_err(Error::Encode)?;
        arg.format_placeholder(sql)
            .map_err(|e| Error::Encode(Box::new(e)))?;

        sql.push_str(" offset ");
        arg.add(offset).map_err(Error::Encode)?;
        arg.format_placeholder(sql)
            .map_err(|e| Error::Encode(Box::new(e)))?;
    }

    Ok(())
}

/// Trait for database connections/pools that can detect their backend type
///
/// # Type Parameters
/// - `'c`: Connection lifetime
/// - `DB`: Database type implementing [`sqlx::Database`]
///
/// # Required Implementations
/// Automatically implemented for types that:
/// - Implement [`Executor`] for database operations
/// - Implement [`Deref`] to an [`Any`] type
///
/// # Provided Methods
/// [`backend_db`]: Default implementation using the module-level function
pub trait BackendDB<'c, DB>
where
    DB: Database,
{
    fn backend_db(
        self,
    ) -> impl std::future::Future<
        Output = Result<(impl DatabaseDialect, impl Executor<'c, Database = DB> + 'c), Error>,
    > + Send;
}
impl<'c, DB, C, C1> BackendDB<'c, DB> for C
where
    DB: Database,
    C: Executor<'c, Database = DB> + 'c + Deref<Target = C1>,
    C1: Any,
    for<'c1> &'c1 mut DB::Connection: Executor<'c1, Database = DB>,
{
    async fn backend_db(
        self,
    ) -> Result<(impl DatabaseDialect, impl Executor<'c, Database = DB> + 'c), Error> {
        backend_db(self).await
    }
}
#[derive(Debug)]
pub struct AdapterExecutor<'c, DB: Database, C: Executor<'c, Database = DB>> {
    executor: Either<C, PoolConnection<DB>>,
    _m: PhantomData<&'c ()>,
}
impl<'c, DB, C> AdapterExecutor<'c, DB, C>
where
    DB: Database,
    C: Executor<'c, Database = DB>,
{
    fn new(executor: Either<C, PoolConnection<DB>>) -> Self {
        Self {
            executor,
            _m: PhantomData,
        }
    }
}

impl<'c, DB, C> Executor<'c> for AdapterExecutor<'c, DB, C>
where
    DB: Database,
    C: Executor<'c, Database = DB>,
    for<'c1> &'c1 mut DB::Connection: Executor<'c1, Database = DB>,
{
    type Database = DB;

    fn fetch_many<'e, 'q: 'e, E>(
        self,
        query: E,
    ) -> futures_core::stream::BoxStream<
        'e,
        Result<
            Either<<Self::Database as Database>::QueryResult, <Self::Database as Database>::Row>,
            Error,
        >,
    >
    where
        'c: 'e,
        E: 'q + Execute<'q, Self::Database>,
    {
        match self.executor {
            Either::Left(executor) => executor.fetch_many(query),
            Either::Right(mut conn) => Box::pin(try_stream! {


                let mut s = conn.fetch_many(query);

                while let Some(v) = s.try_next().await? {
                    r#yield!(v);
                }

                Ok(())
            }),
        }
    }

    fn fetch_optional<'e, 'q: 'e, E>(
        self,
        query: E,
    ) -> futures_core::future::BoxFuture<'e, Result<Option<<Self::Database as Database>::Row>, Error>>
    where
        'c: 'e,
        E: 'q + Execute<'q, Self::Database>,
    {
        match self.executor {
            Either::Left(executor) => executor.fetch_optional(query),
            Either::Right(mut conn) => Box::pin(async move { conn.fetch_optional(query).await }),
        }
    }

    fn prepare_with<'e, 'q: 'e>(
        self,
        sql: &'q str,
        parameters: &'e [<Self::Database as Database>::TypeInfo],
    ) -> futures_core::future::BoxFuture<
        'e,
        Result<<Self::Database as Database>::Statement<'q>, Error>,
    >
    where
        'c: 'e,
    {
        match self.executor {
            Either::Left(executor) => executor.prepare_with(sql, parameters),
            Either::Right(mut conn) => {
                Box::pin(async move { conn.prepare_with(sql, parameters).await })
            }
        }
    }

    fn describe<'e, 'q: 'e>(
        self,
        sql: &'q str,
    ) -> futures_core::future::BoxFuture<'e, Result<Describe<Self::Database>, Error>>
    where
        'c: 'e,
    {
        match self.executor {
            Either::Left(executor) => executor.describe(sql),
            Either::Right(mut conn) => Box::pin(async move { conn.describe(sql).await }),
        }
    }
}
pub async fn backend_db<'c, DB, C, C1>(c: C) -> Result<(DBType, AdapterExecutor<'c, DB, C>), Error>
where
    DB: Database,
    C: Executor<'c, Database = DB> + 'c + Deref<Target = C1>,
    C1: Any + 'static,
{
    if DB::NAME != sqlx_core::any::Any::NAME {
        return Ok((
            DBType::new(DB::NAME)?,
            AdapterExecutor::new(Either::Left(c)),
        ));
    }

    let any_ref = c.deref() as &dyn Any;
    // 处理 AnyConnection
    if let Some(conn) = any_ref.downcast_ref::<AnyConnection>() {
        return Ok((
            DBType::new(conn.backend_name())?,
            AdapterExecutor::new(Either::Left(c)),
        ));
    }

    // 处理 AnyPool
    if let Some(pool) = any_ref.downcast_ref::<AnyPool>() {
        let conn = pool.acquire().await?;

        let db_type = DBType::new(conn.backend_name())?;
        let db_con: Box<dyn Any> = Box::new(conn);
        let return_con = db_con
            .downcast::<PoolConnection<DB>>()
            .map_err(|_| Error::Protocol(format!("unsupport db `{}`", DB::NAME)))?;

        return Ok((db_type, AdapterExecutor::new(Either::Right(*return_con))));
    }
    Err(Error::Protocol(format!("unsupport db `{}`", DB::NAME)))
}
