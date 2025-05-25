use sqlx::{AnyPool, Error, any::install_default_drivers};

use sqlx_askama_template::SqlTemplate;

use sqlx_askama_template::{DBType, backend_db};
#[derive(sqlx::prelude::FromRow, PartialEq, Eq, Debug)]
struct User {
    id: i64,
    name: String,
}
#[derive(SqlTemplate)]
#[template(source = r#"
    select {{e(user_id)}} as id,{{e(user_name)}} as name
    union all 
    {% let id=99999_i64 %}
    {% let name="super man" %}
    select {{et(id)}} as id,{{et(name)}} as name
"#)]
#[add_type(&'q str)]
pub struct UserQuery {
    pub user_id: i64,
    pub user_name: String,
}

async fn test_backend() -> Result<(), Error> {
    let urls = [
        (
            DBType::PostgreSQL,
            "postgres://postgres:postgres@localhost/postgres",
        ),
        (DBType::SQLite, "sqlite://db.file?mode=memory"),
        (DBType::MySQL, "mysql://root:root@localhost/mysql"),
    ];
    install_default_drivers();
    for (db_type, url) in urls {
        let pool = AnyPool::connect(url).await?;
        // pool
        let (get_db_type, get_conn) = backend_db(&pool).await?;
        assert_eq!(db_type, get_db_type);
        assert!(get_conn.is_right());

        // connection
        let mut conn = pool.acquire().await?;
        let (get_db_type, get_conn) = backend_db(&mut *conn).await?;
        assert_eq!(db_type, get_db_type);
        assert!(get_conn.is_left());

        // tx
        let mut conn = pool.begin().await?;
        let (get_db_type, get_conn) = backend_db(&mut *conn).await?;
        assert_eq!(db_type, get_db_type);
        assert!(get_conn.is_left());
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    test_backend().await?;
    install_default_drivers();
    // pg+any test count
    let user_query = UserQuery {
        user_id: 1,
        user_name: "admin".to_string(),
    };
    let mut sql_buff = String::new();
    let db_adatper = user_query.render_db_adapter_manager(&mut sql_buff);
    let pool = AnyPool::connect("postgres://postgres:postgres@localhost/postgres").await?;

    let count = db_adatper.count(&pool).await?;
    assert_eq!(2, count);
    println!("{}", sql_buff);

    // test page
    let db_adatper = user_query.render_db_adapter_manager(&mut sql_buff);
    let user: Option<User> = db_adatper.set_page(1, 1).fetch_optional_as(&pool).await?;

    println!("{:?}", user);

    //sqlite+any

    let pool = AnyPool::connect("sqlite://db.file?mode=memory").await?;
    let user: Vec<User> = user_query
        .render_db_adapter_manager(&mut sql_buff)
        .set_page(1, 2)
        .fetch_all_as(&pool)
        .await?;
    println!("{}", sql_buff);
    println!("{:?}", user);

    let page_info = user_query
        .render_db_adapter_manager(&mut sql_buff)
        .count_page(1, &pool)
        .await?;
    println!("{}", sql_buff);
    println!("{:?}", page_info);

    let user: Vec<User> = user_query
        .render_db_adapter_manager(&mut sql_buff)
        .fetch_all_as(&pool)
        .await?;
    println!("{}", sql_buff);
    println!("{:?}", user);
    Ok(())
}
