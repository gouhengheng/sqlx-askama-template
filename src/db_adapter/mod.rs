use askama::Result;

use sqlx::{Arguments, Database, Encode, Executor, prelude::Type};

use crate::Error;
mod page_adapter;
pub mod sql_utils;
mod template_adapter;
pub use page_adapter::*;
pub use template_adapter::*;
#[cfg(feature = "any")]
pub mod any;
#[cfg(feature = "mysql")]
pub mod mysql;
#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(feature = "sqlite")]
pub mod sqlite;
pub enum DBType {
    PostgreSQL,
    MySQL,
    SQLite,
    UnKown,
}
impl From<&str> for DBType {
    fn from(value: &str) -> Self {
        match value {
            #[cfg(feature = "postgres")]
            sqlx::Postgres::NAME => Self::PostgreSQL,
            #[cfg(feature = "mysql")]
            sqlx::MySql::NAME => Self::MySQL,
            #[cfg(feature = "sqlite")]
            sqlx::Sqlite::NAME => Self::SQLite,
            _ => Self::UnKown,
        }
    }
}
impl DBType {
    #[allow(clippy::type_complexity)]
    pub fn get_encode_placeholder_fn<'c, DB: Database>(
        self,
        adapter: impl DBAdapter<'c, DB>,
    ) -> Result<(Option<fn(usize, &mut String)>, impl DBAdapter<'c, DB>), Error> {
        match self {
            Self::PostgreSQL => Ok((
                Some(|i: usize, s: &mut String| s.push_str(&format!("${}", i))),
                adapter,
            )),
            Self::MySQL | Self::SQLite => {
                Ok((Some(|_: usize, s: &mut String| s.push('?')), adapter))
            }
            Self::UnKown => Err("unkown db type".into()),
        }
    }
    pub fn write_count_sql<'c, DB: Database>(
        self,
        sql: String,
        adapter: impl DBAdapter<'c, DB>,
    ) -> Result<(String, impl DBAdapter<'c, DB>), Error> {
        match self {
            Self::PostgreSQL | DBType::MySQL | DBType::SQLite => {
                Ok(pg_mysql_sqlite_count_sql(sql, adapter))
            }
            Self::UnKown => Err("unkown db type".into()),
        }
    }

    pub fn write_page_sql<'c, DB>(
        self,
        sql: String,
        page_size: i64,
        page_no: i64,
        f: Option<fn(usize, &mut String)>,
        arg: DB::Arguments<'_>,
        adapter: impl DBAdapter<'c, DB>,
    ) -> Result<(String, DB::Arguments<'_>, impl DBAdapter<'c, DB>), Error>
    where
        DB: Database,
        i64: for<'q> Encode<'q, DB> + Type<DB>,
    {
        match self {
            Self::PostgreSQL | DBType::MySQL | DBType::SQLite => {
                pg_mysql_sqlite_page_sql(sql, page_size, page_no, f, arg, adapter)
            }
            Self::UnKown => Err("unkown db type".into()),
        }
    }
}
fn pg_mysql_sqlite_count_sql<'c, DB: Database>(
    sql: String,
    adapter: impl DBAdapter<'c, DB>,
) -> (String, impl DBAdapter<'c, DB>) {
    (format!("select count(1) from ({}) t", sql), adapter)
}
fn pg_mysql_sqlite_page_sql<'c, DB>(
    mut sql: String,
    mut page_size: i64,
    mut page_no: i64,
    f: Option<fn(usize, &mut String)>,
    mut arg: DB::Arguments<'_>,
    adapter: impl DBAdapter<'c, DB>,
) -> Result<(String, DB::Arguments<'_>, impl DBAdapter<'c, DB>), Error>
where
    DB: Database,
    i64: for<'q> Encode<'q, DB> + Type<DB>,
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
        arg.add(page_size).map_err(sqlx::Error::Encode)?;
        f(arg.len(), &mut sql);
        sql.push_str(" offset ");
        arg.add(offset).map_err(sqlx::Error::Encode)?;
        f(arg.len(), &mut sql);
    } else {
        sql.push_str(" limit ");
        arg.add(page_size).map_err(sqlx::Error::Encode)?;
        arg.format_placeholder(&mut sql)?;

        sql.push_str(" offset ");
        arg.add(offset).map_err(sqlx::Error::Encode)?;
        arg.format_placeholder(&mut sql)?;
    }

    Ok((sql, arg, adapter))
}
#[allow(clippy::type_complexity)]
#[allow(async_fn_in_trait)]
pub trait DBAdapter<'c, DB: Database>: Executor<'c, Database = DB> + 'c {
    fn get_executor(
        self,
    ) -> impl std::future::Future<Output = Result<impl Executor<'c, Database = DB> + 'c, Error>> + Send
    {
        async { Ok(self) }
    }
    async fn get_encode_placeholder_fn(
        self,
    ) -> Result<(Option<fn(usize, &mut String)>, impl DBAdapter<'c, DB>), Error> {
        let (db_type, adapter) = self.get_backend_name().await?;
        db_type.get_encode_placeholder_fn(adapter)
    }
    fn get_backend_name(
        self,
    ) -> impl std::future::Future<Output = Result<(DBType, impl DBAdapter<'c, DB>), Error>> + Send
    {
        async { Ok((DBType::from(DB::NAME), self)) }
    }
    async fn write_count_sql(self, sql: String) -> Result<(String, impl DBAdapter<'c, DB>), Error> {
        let (db_type, adapter) = self.get_backend_name().await?;

        db_type.write_count_sql(sql, adapter)
    }
    async fn write_page_sql(
        self,
        sql: String,
        page_size: i64,
        page_no: i64,
        f: Option<fn(usize, &mut String)>,
        arg: DB::Arguments<'_>,
    ) -> Result<(String, DB::Arguments<'_>, impl DBAdapter<'c, DB>), Error>
    where
        i64: for<'q> Encode<'q, DB> + Type<DB>,
    {
        let (db_type, adapter) = self.get_backend_name().await?;

        db_type.write_page_sql(sql, page_size, page_no, f, arg, adapter)
    }
}

//impl<'c, DB: Database, C: Executor<'c, Database = DB> + 'c> DBAdapter<'c, DB> for C {}
