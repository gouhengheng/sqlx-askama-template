use askama::Template;
use ntex::http::Response;
use ntex::web::types::State;
use ntex::web::{self, Responder};
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
async fn root(pool: State<AnyPool>) -> impl Responder {
    //  test count
    let user_query = UserQuery {
        user_id: 1,
        user_name: "ntex",
    };
    let count = user_query.adapter().count(&*pool).await.unwrap();
    println!("count: {count}");

    let mut conn = pool.acquire().await.unwrap();
    let users: Vec<User> = user_query.adapter().fetch_all_as(&mut *conn).await.unwrap();

    Response::Ok().body(IndexHtml { users }.render().unwrap())
}
#[ntex::main]
async fn main() -> std::io::Result<()> {
    install_default_drivers();

    let pool = AnyPool::connect("sqlite://db.file?mode=memory")
        .await
        .unwrap();
    web::HttpServer::new(async move || {
        web::App::new()
            .state(pool.clone())
            .route("/", web::get().to(root))
    })
    .bind(("0.0.0.0", 3000))?
    .run()
    .await
}
