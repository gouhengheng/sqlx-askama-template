use sqlx::{AnyPool, any::install_default_drivers};
use sqlx_askama_template::{Error, SqlTemplate};

#[derive(sqlx::prelude::FromRow, PartialEq, Eq, Debug)]
struct User {
    id: i64,
    name: String,
}
#[derive(SqlTemplate)]
#[template(
    ext = "txt",
    source = r#"
    select {{e(user_id)}} as id,{{e(user_name)}} as name
    union all 
    {% let id=99999_i64 %}
    {% let name="super man" %}
    select {{et(id)}} as id,{{et(name)}} as name
"#
)]
#[addtype(&'q str)]
pub struct UserQuery {
    pub user_id: i64,
    pub user_name: String,
}
#[tokio::main]
async fn main() -> Result<(), Error> {
    // test count
    let user_query = UserQuery {
        user_id: 1,
        user_name: "admin".to_string(),
    };
    let mut sql_buff = String::new();
    let db_adatper = user_query.render_db_adpter_manager(&mut sql_buff);
    let pool = sqlx::PgPool::connect("postgres://postgres:postgres@localhost/postgres").await?;

    let count = db_adatper.count(&pool).await?;
    assert_eq!(2, count);
    println!("{}", sql_buff);

    // test page
    let db_adatper = user_query.render_db_adpter_manager(&mut sql_buff);
    let user: Option<User> = db_adatper
        .to_page_query(1, 1)
        .fetch_optional_as(&pool)
        .await?;

    println!("{:?}", user);

    //sqlite+any
    install_default_drivers();
    let pool = AnyPool::connect("sqlite://db.file?mode=memory").await?;
    let user: Vec<User> = user_query
        .render_db_adpter_manager(&mut sql_buff)
        .to_page_query(1, 2)
        .fetch_all_as(&pool)
        .await?;

    println!("{:?}", user);

    let page_info = user_query
        .render_db_adpter_manager(&mut sql_buff)
        .count_page(1, &pool)
        .await?;
    println!("{:?}", page_info);

    let user: Vec<User> = user_query
        .render_db_adpter_manager(&mut sql_buff)
        .fetch_all_as(&pool)
        .await?;
    println!("{:?}", user);
    Ok(())
}
