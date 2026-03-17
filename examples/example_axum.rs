use askama::Template;
use axum::Router;

use axum::extract::State;
use axum::response::Html;
use axum::routing::get;

use futures_util::TryStreamExt;
use sqlx::any::{AnyRow, install_default_drivers};
use sqlx::{AnyPool, Error, FromRow};
use sqlx_askama_template::{SqlTemplate, SqlTemplateExecute};

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
async fn test_adapter_execute(url: &str) -> Result<(), Error> {
    let execute = SqlTemplateExecute::new("select 1".into(), None);
    let pool = AnyPool::connect(url).await?;
    let (result,): (i32,) = execute.fetch_one_as(&pool).await?;
    println!("execute result: {result}");
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

    println!("{:?}", users);
    let stream = user_query.adapter_render().fetch(&mut *tx);
    drop(stream);
    // user_query.user_id = 2;
    //user_query.user_name = "user";
    tx.rollback().await?;
    let users: Vec<User> = user_query.adapter_render().fetch_all_as(&pool).await?;
    println!("{:?}", users);
    let mut stream = user_query.adapter_render().fetch(&pool);

    while let Some(row) = stream.try_next().await? {
        let (id, name): (i64, String) = <(i64, String) as FromRow<'_, AnyRow>>::from_row(&row)?;
        println!("id:{}, name:{}", id, name);
    }
    test_adapter_execute(url).await?;
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
    let users = user_query
        .adapter_render()
        .fetch_all_as(&mut *conn)
        .await
        .unwrap();
    Html(IndexHtml { users }.render().unwrap())
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
