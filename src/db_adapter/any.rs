use super::DBAdapter;

use futures_util::TryStreamExt;
use sqlx::{Any, AnyConnection, Database, Executor, Pool, pool::PoolConnection};
use sqlx_core::try_stream;

impl<'c> DBAdapter<'c, Any> for &'c mut AnyConnection {
    async fn get_backend_name(
        self,
    ) -> askama::Result<(super::DBType, impl DBAdapter<'c, Any>), crate::Error> {
        Ok((super::DBType::from(self.backend_name()), self))
    }
}

impl<'c> DBAdapter<'c, Any> for &'c Pool<Any> {
    async fn get_backend_name(
        self,
    ) -> askama::Result<(super::DBType, impl DBAdapter<'c, Any>), crate::Error> {
        let conn = self.acquire().await?;
        Ok((
            super::DBType::from(conn.backend_name()),
            PoolConenctionWrap(conn),
        ))
    }
}
#[derive(Debug)]
struct PoolConenctionWrap(PoolConnection<Any>);

impl<'c> Executor<'c> for PoolConenctionWrap {
    type Database = Any;

    fn fetch_many<'e, 'q: 'e, E>(
        self,
        query: E,
    ) -> futures_core::stream::BoxStream<
        'e,
        Result<
            sqlx::Either<
                <Self::Database as Database>::QueryResult,
                <Self::Database as Database>::Row,
            >,
            sqlx::Error,
        >,
    >
    where
        'c: 'e,
        E: 'q + sqlx::Execute<'q, Self::Database>,
    {
        Box::pin(try_stream! {
            let mut conn = self.0;
            let mut s = conn.fetch_many(query);

            while let Some(v) = s.try_next().await? {
                r#yield!(v);
            }

            Ok(())
        })
    }

    fn fetch_optional<'e, 'q: 'e, E>(
        self,
        query: E,
    ) -> futures_core::future::BoxFuture<
        'e,
        Result<Option<<Self::Database as Database>::Row>, sqlx::Error>,
    >
    where
        'c: 'e,
        E: 'q + sqlx::Execute<'q, Self::Database>,
    {
        Box::pin(async move {
            let mut conn = self.0;
            conn.fetch_optional(query).await
        })
    }

    fn prepare_with<'e, 'q: 'e>(
        self,
        sql: &'q str,
        parameters: &'e [<Self::Database as Database>::TypeInfo],
    ) -> futures_core::future::BoxFuture<
        'e,
        Result<<Self::Database as Database>::Statement<'q>, sqlx::Error>,
    >
    where
        'c: 'e,
    {
        Box::pin(async move {
            let mut conn = self.0;
            conn.prepare_with(sql, parameters).await
        })
    }

    fn describe<'e, 'q: 'e>(
        self,
        sql: &'q str,
    ) -> futures_core::future::BoxFuture<'e, Result<sqlx::Describe<Self::Database>, sqlx::Error>>
    where
        'c: 'e,
    {
        Box::pin(async move {
            let mut conn = self.0;
            conn.describe(sql).await
        })
    }
}
impl<'c> DBAdapter<'c, Any> for PoolConenctionWrap {
    async fn get_backend_name(
        self,
    ) -> askama::Result<(super::DBType, impl DBAdapter<'c, Any>), crate::Error> {
        Ok((super::DBType::from(self.0.backend_name()), self))
    }
}
