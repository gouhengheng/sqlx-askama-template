# SQLx Askama Template  

[![Crates.io](https://img.shields.io/crates/v/sqlx-askama-template)](https://crates.io/crates/sqlx-askama-template)
[![Documentation](https://docs.rs/sqlx-askama-template/badge.svg)](https://docs.rs/sqlx-askama-template)
[![GitHub License](https://img.shields.io/github/license/gouhengheng/sqlx-askama-template)](https://github.com/gouhengheng/sqlx-askama-template)
[![GitHub Stars](https://img.shields.io/github/stars/gouhengheng/sqlx-askama-template)](https://github.com/gouhengheng/sqlx-askama-template/stargazers)
[![GitHub Issues](https://img.shields.io/github/issues/gouhengheng/sqlx-askama-template)](https://github.com/gouhengheng/sqlx-askama-template/issues)
[![CI Status](https://github.com/gouhengheng/sqlx-askama-template/actions/workflows/ci.yml/badge.svg)](https://github.com/gouhengheng/sqlx-askama-template/actions)

A SQLx query builder based on the Askama template engine, providing type-safe SQL templates and parameter binding.

## Features

- 🚀 **Zero-Cost Abstraction** - Compile-time optimized SQL generation
- 🔒 **Type Safety** - Automatic validation of SQL parameter types
- 📦 **Multi-Database Support** - PostgreSQL/MySQL/SQLite/Any
- 💡 **Smart Parameter Binding** - Auto-expansion for list parameters
- 🎨 **Template Syntax** - Full Askama templating capabilities

## Require
sqlx > 0.9.0

## Installation

Add to `Cargo.toml`:

```toml
[dependencies]
sqlx-askama-template = "0.4.0"
 
tokio = { version = "1.0", features = ["full"] }
sqlx = { version = "0.9.0", default-features = false, features = [
    "all-databases",
    "runtime-tokio",
    "macros",
] }
env_logger="0.11"
futures-util = "0.3.31"
```

## Quick Start

### Basic Usage

```rust
use futures_util::{TryStreamExt, pin_mut};
use sqlx::{AnyPool, Error, Row, any::install_default_drivers};

use sqlx_askama_template::{BackendDB, DBType, PaginationInfo, SqlTemplate};
#[derive(sqlx::prelude::FromRow, PartialEq, Eq, Debug)]
struct User {
    id: i64,
    name: String,
}
#[derive(SqlTemplate, PartialEq, Eq, Debug)]
#[template(source = r#"
    select {{e(user_id)}} as id,{{e(user_name)}} as name
    union all 
    {%- let id=99999_i32 %}
    {%- let name="super man" %}
    select {{e(id)}} as id,{{e(name)}} as name
"#)]
#[add_type(&'q str,i32)] //  binding Encode trait for local &str and i32 variables in template, 'q is default lifetime
pub struct UserQuery {
    pub user_id: i64,
    pub user_name: String,
}

async fn simple_query(urls: Vec<(DBType, &str)>) -> Result<(), Error> {
    let data = vec![
        User {
            id: 1,
            name: "admin".to_string(),
        },
        User {
            id: 99999_i64,
            name: "super man".to_string(),
        },
    ];

    for (db_type, url) in urls {
        //  test count
        let mut user_query = UserQuery {
            user_id: 1,
            user_name: "admin".into(),
        };
        let pool = AnyPool::connect(url).await?;

        let db_adatper = user_query.adapter();

        let count = db_adatper.count(&pool).await?;
        assert_eq!(2, count);

        // test pagination
        let user: Option<User> = user_query
            .adapter()
            .set_pagination(1, 1)
            .fetch_optional_as(&pool)
            .await?;
        assert_eq!(data.first(), user.as_ref());
        // println!("{user:?}");

        let mut conn = pool.acquire().await?;
        let user: Vec<User> = user_query
            .adapter()
            .set_pagination(1, 2)
            .fetch_all_as(&mut *conn)
            .await?;
        assert_eq!(data[1..], user);
        // println!("{user:?}");

        let page_info = user_query.adapter().pagination_info(1, &pool).await?;
        assert_eq!(PaginationInfo::new(2, 1), page_info);
        // println!("{page_info:?}");
        //fecth
        let mut tx = pool.begin().await?;

        let rows = UserQuery {
            user_id: 1,
            user_name: "admin".into(),
        }
        .adapter()
        .fetch_all(&mut *tx)
        .await?;
        assert_eq!(2, rows.len());
        //println!("{:?}", rows.len());
        let row = UserQuery {
            user_id: 1,
            user_name: "admin".into(),
        }
        .adapter()
        .fetch_optional(&mut *tx)
        .await?;
        assert!(row.is_some());
        let row = UserQuery {
            user_id: 1,
            user_name: "admin".into(),
        }
        .adapter()
        .fetch_one(&mut *tx)
        .await?;
        assert_eq!(2, row.columns().len());
        // fetch_as
        let users: Vec<User> = user_query.adapter().fetch_all_as(&pool).await?;
        assert_eq!(data, users);
        //println!("{:?}", users);

        let u: Option<User> = UserQuery {
            user_id: 1,
            user_name: "admin".into(),
        }
        .adapter()
        .fetch_optional_as(&mut *tx)
        .await?;
        assert_eq!(data.first(), u.as_ref());
        let u: User = UserQuery {
            user_id: 1,
            user_name: "admin".into(),
        }
        .adapter()
        .fetch_one_as(&mut *tx)
        .await?;
        assert_eq!(data.first(), Some(&u));

        // stream
        let query = user_query.adapter();
        {
            let stream = query.fetch(&mut *tx);
            pin_mut!(stream);
            while let Some(row) = stream.try_next().await? {
                assert_eq!(2, row.columns().len());
            }
        }

        tx.rollback().await?;
        user_query.user_id = 1;
        // test backend db type
        let backend_db = pool.backend_db().await?;
        assert_eq!(db_type, backend_db.0);
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    unsafe {
        std::env::set_var("RUST_LOG", "sqlx_askama_template=DEBUG");
    }
    env_logger::init();
    tokio::spawn(async { println!("started") });
    install_default_drivers();
    let urls = vec![
        (
            DBType::PostgreSQL,
            "postgres://postgres:postgres@localhost/postgres",
        ),
        (DBType::SQLite, "sqlite://db.file?mode=memory"),
        (DBType::MySQL, "mysql://root:root@localhost/mysql"),
    ];
    simple_query(urls).await?;

    Ok(())
}

```

## Core Features

### Template Syntax

| Syntax             | Example                   | Description                  |
|---------------------|---------------------------|------------------------------|
| Single Parameter    | `{{e(user_id)}}`          | Binds a single parameter     |
| List Expansion      | `{{el(ids)}}`             | Expands to `(?, ?)`       |


### Parameter Encoding Methods

| Method  | Description                   | Example               |
|---------|-------------------------------|-----------------------|
| `e()`   | Encodes a single value        | `{{e(user_id)}}`      |
| `el()`  | Encodes a list (`$1, $2...`)  | `{{el(ids)}}`         |


## Multi-Database Support

| Database    | Parameter Style | Example            |
|-------------|-----------------|--------------------|
| PostgreSQL  | `$1, $2`        | `WHERE id = $1`    |
| MySQL       | `?`             | `WHERE id = ?`     |
| SQLite      | `?`             | `WHERE id = ?`     |

## Macro Attributes

### `#[template]` - Core Template Attribute

```rust
#[derive(SqlTemplate)]
#[template(
    source = "SQL template content",  // Required
    ext = "txt",                      // Askama file extension
    print = "all",                    // Optional debug output (none, ast, code, all)
    config = "path"                   // Optional custom Askama config path
)]
```

**Parameters**:
- `source`: Inline SQL template content (supports Askama syntax)
- `ext`: Askama file extension
- `print`: Debug mode for Askama
- `config`: Path to a custom Askama configuration file

### `#[add_type]` - Add Additional Type Constraints

Used to add `Encode + Type` constraints for non-field types in templates (e.g., `Vec<T>`, `HashMap<K, V>`).

```rust
#[derive(SqlTemplate)]
#[add_type(chrono::NaiveDate, Option<&'q str>)]  // Add extra type support
```

**Features**:
- Adds type constraints for template-local variables
- Supports comma-separated types

### `#[ignore_type]` - Skip Field Type Constraints

```rust
#[derive(SqlTemplate)]
struct Query {
    #[ignore_type]  // Skip type checks for this field
    metadata: JsonValue
}
```

**Use Cases**:
- Skip fields that do not require SQLx parameter binding
- Avoid unnecessary trait constraints for complex types

## Full Example

```rust
 use sqlx::Arguments;
use sqlx_askama_template::SqlTemplate;
use std::collections::HashMap;

#[derive(SqlTemplate)]
#[template(
    source = r#"
    {%- let v="abc".to_string() %}
    SELECT {{e(v)}} as v,t.* FROM table t
    WHERE arg1 = {{e(arg1)}}
      AND arg2 = {{e(arg2)}}
      AND arg3 = {{e(arg3)}}
      AND arg4 = {{e(arg4.first())}}
      AND arg5 = {{e(arg5.get(&0))}}
      {%- let v2=3_i64 %}
      AND arg6 = {{e(v2)}}
      {%- let v3="abc".to_string() %}
      AND arg7 = {{e(v3)}}
      AND arg_list1 in {{el(arg4)}}
      {%- let list=["abc".to_string()] %}
      AND arg_temp_list1 in {{el(list.iter())}}
      AND arg_list2 in {{el(arg5.values())}}
      {%- if let Some(first) = arg4.first() %}
        AND arg_option = {{e(first)}}
      {%- endif %}
      {%- if let Some(first) = arg5.get(&0) %}
        AND arg_option1 = {{e(first)}}
      {%- endif %}
"#,
    print = "all"
)]
#[add_type(Option<&'a i64>,bool)]
pub struct QueryData<'a, T>
where
    T: Sized + Send + Sync,
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
    {%- let status_list = ["active", "pending"] %}
    SELECT
        u.id,
        u.name,
        COUNT(o.id) AS order_count
    FROM users u
    LEFT JOIN orders o ON u.id = o.user_id
    WHERE 1=1
    {%- if let Some(min_age) = min_age %}
        AND age >= {{e(min_age)}}
    {%- endif %}
    {%- if filter_names.len()>0 %}
        AND name IN {{el(filter_names)}}
    {%- endif %}
    AND status IN {{el(*status_list)}}
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
        <&QueryData<'_, i32> as SqlTemplate<'_, sqlx::Postgres>>::render(&data).unwrap();

    assert_eq!(arg.unwrap().len(), 18);
    println!("----{sql}----");
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
        <&QueryData<'_, i32> as SqlTemplate<'_, sqlx::Postgres>>::render(&data).unwrap();

    assert_eq!(arg.unwrap().len(), 18);
    println!("----{sql}----");

    let data = ComplexQuery {
        filter_names: vec!["name1", "name2"],
        limit: 10,
        min_age: Some(18),
        order_field: "id",
    };

    let (sql, arg) = <&ComplexQuery<'_> as SqlTemplate<'_, sqlx::Postgres>>::render(&data).unwrap();

    assert_eq!(arg.unwrap().len(), 6);

    println!("----{sql}----");
}

```

## Best Practices

```markdown
1. Use `{% if %}` blocks for dynamic SQL
2. Use `add_type` to add type constraints for template-local variables
3. Use `ignore_type` to skip serialization for specific fields
4. Set `print = "none"` in production
```

## License

Licensed under either of

-   Apache License, Version 2.0
    ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
-   MIT license
    ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
