use futures_util::pin_mut;
use futures_util::stream::TryStreamExt;
use sqlx::postgres::PgListener;
use sqlx::{AnyPool, Error, any::install_default_drivers};
use sqlx::{MySqlPool, PgPool, Row, SqlitePool};

use sqlx_askama_template::{BackendDB, DatabaseDialect, PageInfo, SqlTemplate};

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
    select {{et(id)}} as id,{{et(name)}} as name
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
    let data = vec![
        User {
            id: 1,
            name: "admin".to_string(),
        },
        User {
            id: 99999,
            name: "super man".to_string(),
        },
    ];
    //  test count
    let user_query = UserQuery {
        user_id: 1,
        user_name: "admin",
    };
    let pool = AnyPool::connect(url).await?;

    let mut db_adatper = user_query.adapter_render();

    let count = db_adatper.count(&pool).await?;
    assert_eq!(2, count);

    // test pagination
    let user: Option<User> = user_query
        .adapter_render()
        .set_page(1, 1)
        .fetch_optional_as(&pool)
        .await?;
    assert_eq!(data.first(), user.as_ref());
    // println!("{user:?}");

    let mut conn = pool.acquire().await?;
    let user: Vec<User> = user_query
        .adapter_render()
        .set_page(1, 2)
        .fetch_all_as(&mut *conn)
        .await?;
    assert_eq!(data[1..], user);
    // println!("{user:?}");

    let page_info = user_query.adapter_render().count_page(1, &pool).await?;
    assert_eq!(PageInfo::new(2, 1), page_info);
    // println!("{page_info:?}");
    //fecth
    let mut tx = pool.begin().await?;

    let rows = UserQuery {
        user_id: 1,
        user_name: "admin",
    }
    .adapter_render()
    .fetch_all(&mut *tx)
    .await?;
    assert_eq!(2, rows.len());
    //println!("{:?}", rows.len());
    let row = UserQuery {
        user_id: 1,
        user_name: "admin",
    }
    .adapter_render()
    .fetch_optional(&mut *tx)
    .await?;
    assert!(row.is_some());
    let row = UserQuery {
        user_id: 1,
        user_name: "admin",
    }
    .adapter_render()
    .fetch_one(&mut *tx)
    .await?;
    assert_eq!(2, row.columns().len());
    // fetch_as
    let users: Vec<User> = user_query.adapter_render().fetch_all_as(&pool).await?;
    assert_eq!(data, users);
    //println!("{:?}", users);

    let u: Option<User> = UserQuery {
        user_id: 1,
        user_name: "admin",
    }
    .adapter_render()
    .fetch_optional_as(&mut *tx)
    .await?;
    assert_eq!(data.first(), u.as_ref());
    let u: User = UserQuery {
        user_id: 1,
        user_name: "admin",
    }
    .adapter_render()
    .fetch_one_as(&mut *tx)
    .await?;
    assert_eq!(data.first(), Some(&u));

    // stream
    let mut query = user_query.adapter_render();
    {
        let stream = query.fetch(&mut *tx);
        pin_mut!(stream);
        while let Some(row) = stream.try_next().await? {
            assert_eq!(2, row.columns().len());
        }
    }

    tx.rollback().await?;

    Ok(())
}
#[tokio::main]
async fn main() -> Result<(), Error> {
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
}
