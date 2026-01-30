use futures_util::TryStreamExt;
use sqlx::any::AnyRow;
use sqlx::postgres::PgListener;
use sqlx::{AnyPool, Error, any::install_default_drivers};
use sqlx::{FromRow, MySqlPool, PgPool, SqlitePool};

use sqlx_askama_template::{BackendDB, DatabaseDialect, SqlTemplate};

use sqlx_askama_template::DBType;

#[derive(sqlx::prelude::FromRow, PartialEq, Eq, Debug)]
struct User {
    id: i64,
    name: String,
}
#[derive(SqlTemplate)]
#[template(source = r#"
    select {{e(user_id)}} as id,{{e(user_name)}} as name
    union all 
    {%- let id=99999_i64 %}
    {%- let name="super man" %}
    select {{e(id)}} as id,{{e(name)}} as name
"#)]
pub struct UserQuery<'a> {
    pub user_id: i64,
    pub user_name: &'a str,
}

async fn test_backend(urls: Vec<(DBType, &str)>) -> Result<(), Error> {
    install_default_drivers();
    for (db_type, url) in urls {
        let pool = AnyPool::connect(url).await?;
        // pool
        let (get_db_type, _get_conn) = pool.backend_db().await?;
        assert_eq!(db_type.backend_name(), get_db_type.backend_name());

        // connection
        let mut conn = pool.acquire().await?;
        let (get_db_type, _get_conn) = conn.backend_db().await?;
        assert_eq!(db_type.backend_name(), get_db_type.backend_name());

        // tx
        let mut conn = pool.begin().await?;
        let (get_db_type, _get_conn) = conn.backend_db().await?;
        assert_eq!(db_type.backend_name(), get_db_type.backend_name());

        match db_type {
            DBType::MySQL => {
                //mysql  DBType::MySQL, "mysql://root:root@localhost/mysql"
                let pool = MySqlPool::connect(url).await?;
                // pool
                let (get_db_type, _get_conn) = pool.backend_db().await?;
                assert_eq!(DBType::MySQL.backend_name(), get_db_type.backend_name());

                // connection
                let mut conn = pool.acquire().await?;
                let (get_db_type, _get_conn) = conn.backend_db().await?;
                assert_eq!(DBType::MySQL.backend_name(), get_db_type.backend_name());

                //
                let mut conn = pool.begin().await?;
                let (get_db_type, _get_conn) = conn.backend_db().await?;
                assert_eq!(DBType::MySQL.backend_name(), get_db_type.backend_name());
            }
            DBType::PostgreSQL => {
                //pg
                let pool = PgPool::connect(url).await?;
                // pool
                let (get_db_type, _get_conn) = pool.backend_db().await?;
                assert_eq!(
                    DBType::PostgreSQL.backend_name(),
                    get_db_type.backend_name()
                );

                // connection
                let mut conn = pool.acquire().await?;
                let (get_db_type, _get_conn) = conn.backend_db().await?;
                assert_eq!(
                    DBType::PostgreSQL.backend_name(),
                    get_db_type.backend_name()
                );

                // tx
                let mut conn = pool.begin().await?;
                let (get_db_type, _get_conn) = conn.backend_db().await?;
                assert_eq!(
                    DBType::PostgreSQL.backend_name(),
                    get_db_type.backend_name()
                );

                // listener
                let mut listener = PgListener::connect(url).await?;
                let (get_db_type, _get_conn) = listener.backend_db().await?;
                assert_eq!(
                    DBType::PostgreSQL.backend_name(),
                    get_db_type.backend_name()
                );
            }
            DBType::SQLite => {
                //sqlite DBType::SQLite, "sqlite://db.file?mode=memory"
                let pool = SqlitePool::connect(url).await?;
                // pool
                let (get_db_type, _get_conn) = pool.backend_db().await?;
                assert_eq!(DBType::SQLite.backend_name(), get_db_type.backend_name());

                // connection
                let mut conn = pool.acquire().await?;
                let (get_db_type, _get_conn) = conn.backend_db().await?;
                assert_eq!(DBType::SQLite.backend_name(), get_db_type.backend_name());

                // tx
                let mut conn = pool.begin().await?;
                let (get_db_type, _get_conn) = conn.backend_db().await?;
                assert_eq!(DBType::SQLite.backend_name(), get_db_type.backend_name());
            }
        }
        test_adapter_query(url).await?;
    }

    Ok(())
}

async fn test_adapter_query(url: &str) -> Result<(), Error> {
    //  test count
    let mut user_query = UserQuery {
        user_id: 1,
        user_name: "admin",
    };
    let pool = AnyPool::connect(url).await?;

    let db_adatper = user_query.adapter_render();

    let count = db_adatper.count(&pool).await?;
    assert_eq!(2, count);

    // test pagination
    let user: Option<User> = user_query
        .adapter_render()
        .set_page(1, 1)
        .fetch_optional_as(&pool)
        .await?;

    println!("{user:?}");

    let mut conn = pool.acquire().await?;
    let user: Vec<User> = user_query
        .adapter_render()
        .set_page(1, 2)
        .fetch_all_as(&mut *conn)
        .await?;

    println!("{user:?}");

    let page_info = user_query.adapter_render().count_page(1, &pool).await?;

    println!("{page_info:?}");
    //fecth
    let mut tx = pool.begin().await?;
    let user: Vec<User> = user_query.adapter_render().fetch_all_as(&mut *tx).await?;

    println!("{user:?}");

    let users: Vec<User> = UserQuery {
        user_id: 1,
        user_name: "admin",
    }
    .adapter_render()
    .fetch_all_as(&mut *tx)
    .await?;
    tx.rollback().await?;
    println!("{:?}", users);
    user_query.user_id = 2;
    user_query.user_name = "user";

    let users: Vec<User> = user_query.adapter_render().fetch_all_as(&pool).await?;
    println!("{:?}", users);
    let mut stream = user_query.adapter_render().fetch(&pool).await;

    while let Some(row) = stream.try_next().await? {
        let (id, name): (i64, String) = <(i64, String) as FromRow<'_, AnyRow>>::from_row(&row)?;
        println!("id:{}, name:{}", id, name);
    }

    Ok(())
}

fn main() -> Result<(), Error> {
    smol::block_on(async {
        unsafe {
            std::env::set_var("RUST_LOG", "sqlx_askama_template=DEBUG");
        }
        env_logger::init();
        let urls = vec![
            (
                DBType::PostgreSQL,
                "postgres://postgres:postgres@localhost/postgres",
            ),
            (DBType::SQLite, "sqlite://db.file?mode=memory"),
            //(DBType::MySQL, "mysql://root:root@localhost/mysql"),
        ];
        test_backend(urls).await?;
        Ok(())
    })
}
