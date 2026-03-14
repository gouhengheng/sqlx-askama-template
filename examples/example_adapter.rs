use axum::routing::get;
use axum::{Json, Router};

use sqlx::{AnyPool, Error, any::install_default_drivers};

use sqlx_askama_template::{DatabaseDialect, SqlTemplate, SqlTemplateExecute, backend_db};

#[derive(sqlx::prelude::FromRow, PartialEq, Eq, Debug, serde::Serialize)]
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

async fn test_adapter_query(url: &str) -> Result<(), Error> {
    //  test count
    let mut user_query = UserQuery {
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

    Ok(())
}
#[axum::debug_handler]
async fn root() -> Json<Vec<User>> {
    //  test count
    let user_query = UserQuery {
        user_id: 1,
        user_name: "admin",
    };
    let pool = AnyPool::connect("postgres://postgres:postgres@localhost/postgres")
        .await
        .unwrap();

    // let mut db_adatper = user_query.adapter_render();
    let (mut sql, arg, db_type, executor) = {
        let (db_type, executor) = backend_db(&pool).await.unwrap();
        let f = db_type.get_encode_placeholder_fn();
        let mut sql = String::new();
        let mut arg: Option<sqlx::any::AnyArguments<'_>> = <&UserQuery as SqlTemplate<
            '_,
            sqlx::any::Any,
        >>::render_sql_with_encode_placeholder_fn(
            &user_query, f, &mut sql
        )
        .unwrap();

        if let (Some(page_no), Some(page_size)) = (None, None) {
            let mut args: sqlx::any::AnyArguments<'_> = arg.unwrap_or_default();
            db_type
                .write_page_sql::<'_, '_, sqlx::any::Any>(&mut sql, page_size, page_no, &mut args)
                .unwrap();
            arg = Some(args);
        }
        (sql, arg, db_type, executor)
    };

    db_type.write_count_sql(&mut sql);

    let execute: SqlTemplateExecute<'_, sqlx_core::any::Any> = SqlTemplateExecute::new(&sql, arg);
    let (count,): (i64,) = execute.fetch_one_as(executor).await.unwrap();
    println!("count: {count}");

    let rows = user_query.adapter_render().fetch_all(&pool).await.unwrap();

    let mut conn = pool.acquire().await.unwrap();
    let rows = user_query
        .adapter_render()
        .fetch_all_as(&mut *conn)
        .await
        .unwrap();
    //let count = db_adatper.count(&pool).await.unwrap();
    //assert_eq!(2, count);
    Json(rows)
}
#[tokio::main]
async fn main() {
    install_default_drivers();
    // Example test case for a SQL template
    // This is a placeholder and should be replaced with actual test logic
    let app = Router::new().route("/", get(root));

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
