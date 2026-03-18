use askama::Template;
use axum::Router;
use axum::extract::State;
use axum::response::Html;
use axum::routing::get;

use sqlx::any::install_default_drivers;
use sqlx::{AnyPool, Database, Error, MySql, MySqlPool, PgPool, Row};
use sqlx_askama_template::{PageInfo, SqlTemplate};

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
    select {{e(id)}} as id,{{e(name)}} as name
"#)]
pub struct UserQuery<'a> {
    pub user_id: i64,
    pub user_name: &'a str,
}
#[derive(askama::Template)]
#[template(
    ext = "html",
    source = r#"
   <html>
    <head>
        <title>SQLx Askama Template</title>
    </head>
    <body>
        <h1>SQLx Askama Template</h1>
        <p>Welcome to the SQLx Askama Template!</p>
        <h1>query database with askama template</h1>
        <table border="1">
            <tr>
                <th>id</th>
                <th>name</th>
            </tr>
            {% for user in users %}
            <tr>
                <td>{{ user.id }}</td>
                <td>{{ user.name }}</td>    
            </tr>
            {% endfor %}
    </body>
</html>"#
)]
struct IndexHtml {
    users: Vec<User>,
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

    let db_adatper = user_query.adapter_render();

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
    let a = user_query.adapter_render();
    let row = a.fetch(&mut *tx);
    drop(row);
    tx.rollback().await?;

    #[derive(SqlTemplate)]
    #[template(source = r#"
    update test_user set name = {{e(name)}} where id = {{e(id)}}
"#)]
    pub struct Update {
        id: i32,
        name: String,
    }
    // 测试 execute
    let pool = PgPool::connect("postgres://postgres:postgres@localhost/postgres").await?;

    let res = Update {
        id: 1,
        name: "update".to_string(),
    }
    .adapter_render()
    .execute(&pool)
    .await?;
    println!("execute result: {}", res.rows_affected());

    let pool = MySqlPool::connect("mysql://root:root@localhost/test").await?;
    let res = Update {
        id: 1,
        name: "update".to_string(),
    }
    .adapter_render()
    .execute(&pool)
    .await?;
    println!("execute result: {}", res.rows_affected());
    let mut execute = Update {
        id: 1,
        name: "update".to_string(),
    };
    let bind = execute.adapter_render();
    use futures_util::TryStreamExt;
    let execute_many: <MySql as Database>::QueryResult =
        bind.execute_many(&pool).try_collect().await?;
    println!("execute result: {}", execute_many.rows_affected());

    execute.id = 2;
    Ok(())
}
#[axum::debug_handler]
async fn root(State(pool): State<AnyPool>) -> impl axum::response::IntoResponse {
    test_adapter_query("sqlite://db.file?mode=memory")
        .await
        .unwrap();
    //  test count
    let user_query = UserQuery {
        user_id: 1,
        user_name: "axum",
    };
    let count = user_query.adapter_render().count(&pool).await.unwrap();
    println!("count: {count}");

    let mut conn = pool.acquire().await.unwrap();
    let rows = user_query
        .adapter_render()
        .fetch_all_as(&mut *conn)
        .await
        .unwrap();
    Html(IndexHtml { users: rows }.render().unwrap())
}
#[tokio::main]
async fn main() {
    install_default_drivers();

    let pool = AnyPool::connect("sqlite://db.file?mode=memory")
        .await
        .unwrap();
    let app = Router::new().route("/", get(root)).with_state(pool);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
