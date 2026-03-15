use askama::Template;
use axum::Router;
use axum::extract::State;
use axum::response::Html;
use axum::routing::get;
use sqlx::AnyPool;
use sqlx::any::install_default_drivers;
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
#[axum::debug_handler]
async fn root(State(pool): State<AnyPool>) -> impl axum::response::IntoResponse {
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
