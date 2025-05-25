use std::collections::HashMap;

use sqlx::{AnyPool, Arguments, Error, Executor, FromRow, MySqlPool, any::install_default_drivers};

use sqlx_askama_template::SqlTemplate;
#[derive(sqlx::prelude::FromRow, PartialEq, Eq, Debug)]
struct User {
    id: i64,
    name: String,
}
#[derive(SqlTemplate)]
#[template(ext="html",askama=askama,source = r#"
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

async fn simple_query() -> Result<(), Error> {
    let users = vec![
        User {
            id: 1,
            name: "admin".to_string(),
        },
        User {
            id: 99999_i64,
            name: "super man".to_string(),
        },
    ];

    let user_query = UserQuery {
        user_id: 1,
        user_name: "admin".to_string(),
    };
    //pg

    let pool = sqlx::PgPool::connect("postgres://postgres:postgres@localhost/postgres").await?;
    let mut sql_buff = String::new();
    let execute = user_query.render_execute_able(&mut sql_buff)?;

    let rows = pool.fetch_all(execute).await?;
    let mut db_users = Vec::new();
    for row in &rows {
        db_users.push(User::from_row(row)?);
    }
    assert_eq!(db_users, users);

    //sqlite+any
    install_default_drivers();
    let pool = AnyPool::connect("sqlite://db.file?mode=memory").await?;
    let mut sql_buff = String::new();
    let rows = user_query
        .render_execute_able(&mut sql_buff)?
        .fetch_all(&pool)
        .await?;

    let mut db_users = Vec::new();
    for row in &rows {
        db_users.push(User::from_row(row)?);
    }
    assert_eq!(db_users, users);

    //mysql

    let pool = MySqlPool::connect("mysql://root:root@localhost/mysql").await?;

    let mut sql_buff = String::new();
    let db_users: Vec<User> = user_query
        .render_execute_able(&mut sql_buff)?
        .set_persistent(false)
        .fetch_all_as(&pool)
        .await?;

    assert_eq!(db_users, users);
    Ok(())
}

#[derive(SqlTemplate)]
#[add_type(Option<&'a i64>,bool)]
#[template(
    source = r#"
    {% let v="abc".to_string() %}
    SELECT {{et(v)}} as v,t.* FROM table t
    WHERE arg1 = {{e(arg1)}}
      AND arg2 = {{e(arg2)}}
      AND arg3 = {{e(arg3)}}
      AND arg4 = {{e(arg4.first())}}
      AND arg5 = {{e(arg5.get(&0))}}
      {% let v2=3_i64 %}
      AND arg6 = {{et(v2)}}
      {% let v3="abc".to_string() %}
      AND arg7 = {{et(v3)}}
      AND arg_list1 in {{el(arg4)}}
      {% let list=["abc".to_string()] %}
      AND arg_temp_list1 in {{etl(*list)}}
      AND arg_list2 in {{el(arg5.values())}}
      {% if let Some(first) = arg4.first() %}
        AND arg_option = {{et(**first)}}
      {% endif %}
      {% if let Some(first) = arg5.get(&0) %}
        AND arg_option1 = {{et(**first)}}
      {% endif %}     
"#,
    print = "all"
)]
pub struct QueryData<'a, T>
where
    T: Sized,
{
    arg1: i64,
    _arg1: i64, //same type
    arg2: String,
    arg3: &'a str,
    #[ignore_type]
    arg4: Vec<i64>,
    #[ignore_type]
    arg5: HashMap<i32, i64>,
    #[ignore_type]
    #[allow(unused)]
    arg6: T,
}

#[derive(SqlTemplate)]
#[template(source = r#"
    {% let status_list = ["active", "pending"] %}
    SELECT 
        u.id,
        u.name,
        COUNT(o.id) AS order_count
    FROM users u
    LEFT JOIN orders o ON u.id = o.user_id
    WHERE 1=1
    {% if let Some(min_age) = min_age %}
        AND age >= {{et(min_age)}}
    {% endif %}
    {% if filter_names.len()>0 %}
        AND name IN {{el(filter_names)}}
    {% endif %}
    AND status IN {{etl(*status_list)}}
    GROUP BY u.id
    ORDER BY {{order_field}}
    LIMIT {{e(limit)}}
    "#)]
#[add_type(i32)]
pub struct ComplexQuery<'a> {
    min_age: Option<i32>,
    #[ignore_type]
    filter_names: Vec<&'a str>,
    order_field: &'a str,
    limit: i64,
}

fn render_complex_sql() {
    let data = QueryData {
        arg1: 42,
        _arg1: 123,
        arg2: "value".to_string(),
        arg3: "reference",
        arg4: vec![12, 12, 55, 66],
        arg5: HashMap::from_iter([(0, 2), (1, 2), (2, 3)]),
        arg6: 1,
    };

    let (sql, arg) =
        <&QueryData<'_, i32> as SqlTemplate<'_, sqlx::Postgres>>::render_sql(&data).unwrap();

    assert_eq!(arg.unwrap().len(), 18);
    println!("----{sql}----");

    let data = ComplexQuery {
        filter_names: vec!["name1", "name2"],
        limit: 10,
        min_age: Some(18),
        order_field: "id",
    };

    let (sql, arg) =
        <&ComplexQuery<'_> as SqlTemplate<'_, sqlx::Postgres>>::render_sql(&data).unwrap();

    assert_eq!(arg.unwrap().len(), 6);
    println!("----{sql}----");
}
#[tokio::main]
async fn main() -> Result<(), Error> {
    simple_query().await?;
    render_complex_sql();

    Ok(())
}
