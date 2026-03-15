use actix_web::web::{Data, Html};
use actix_web::{App, HttpServer, web};

use askama::Template;
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
async fn root(pool: Data<AnyPool>) -> impl actix_web::Responder {
    //  test count
    let user_query = UserQuery {
        user_id: 1,
        user_name: "actix",
    };
    let count = user_query.adapter_render().count(&**pool).await.unwrap();
    println!("count: {count}");

    let mut conn = pool.acquire().await.unwrap();
    let users: Vec<User> = user_query
        .adapter_render()
        .fetch_all_as(&mut *conn)
        .await
        .unwrap();

    Html::new(IndexHtml { users }.render().unwrap())
}
#[actix_web::main] // or #[tokio::main]
async fn main() -> std::io::Result<()> {
    install_default_drivers();
    let pool = AnyPool::connect("sqlite://db.file?mode=memory")
        .await
        .unwrap();

    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(pool.clone()))
            .route("/", web::get().to(root))
    })
    .bind("0.0.0.0:3000")?
    .run()
    .await
}
