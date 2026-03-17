use askama::Template;
use axum::Router;
use axum::extract::State;
use axum::response::Html;
use axum::routing::get;

use sqlx::any::install_default_drivers;
use sqlx::{AnyPool, Error};
use sqlx_askama_template::SqlTemplate;

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
    let mut render = user_query.adapter_render();
    let stream = render.fetch(&pool);
    drop(stream);
    user_query.user_id = 2;
    user_query.user_name = "user";

    let users: Vec<User> = user_query.adapter_render().fetch_all_as(&pool).await?;
    println!("{:?}", users);

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
