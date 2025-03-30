# SQLx Askama Template


[![Crates.io](https://img.shields.io/crates/v/sqlx-askama-template)](https://crates.io/crates/sqlx-askama-template)  
[![Documentation](https://docs.rs/sqlx-askama-template/badge.svg)](https://docs.rs/sqlx-askama-template)  
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![GitHub License](https://img.shields.io/github/license/gouhengheng/sqlx-askama-template)](https://github.com/gouhengheng/sqlx-askama-template)
[![GitHub Stars](https://img.shields.io/github/stars/gouhengheng/sqlx-askama-template)](https://github.com/gouhengheng/sqlx-askama-template/stargazers)
[![GitHub Issues](https://img.shields.io/github/issues/gouhengheng/sqlx-askama-template)](https://github.com/gouhengheng/sqlx-askama-template/issues)
[![CI Status](https://github.com/gouhengheng/sqlx-askama-template/actions/workflows/ci.yml/badge.svg)](https://github.com/gouhengheng/sqlx-askama-template/actions)


A SQLx query builder powered by the Askama template engine, offering type-safe SQL templating and parameter binding.

## Features

- 🚀 **Zero-cost Abstraction** - Compile-time SQL generation  
- 🔒 **Type Safety** - Automatic validation of SQL parameter types  
- 📦 **Multi-Database Support** - PostgreSQL/MySQL/SQLite/Any  
- 💡 **Smart Parameter Binding** - Automatic list parameter expansion  
- 🎨 **Template Syntax** - Full Askama templating capabilities  

## Installation

Add to `Cargo.toml`:

```toml
[dependencies]
sqlx-askama-template = "0.1"
sqlx = { version = "0.8", features = ["postgres", "runtime-tokio"] }
askama = "0.13.0"
tokio = { version = "1.0", features = ["full"] }
```

## Quick Start

### Basic Usage

```rust
use std::collections::HashMap;

use sqlx::any::install_default_drivers;
use sqlx::{AnyPool, Arguments, MySqlPool};
use sqlx::{Executor, FromRow};
use sqlx_askama_template::SqlTemplate;

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
async fn main() -> sqlx::Result<()> {
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
    // PostgreSQL
    unsafe {
        std::env::set_var(
            "DATABASE_URL",
            "postgres://postgres:postgres@localhost/postgres",
        );
    }

    let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").unwrap()).await?;
    let mut render_execute = user_query
        .render_execute()
        .map_err(|e| sqlx::Error::Encode(Box::new(e)))?;
    let execute = render_execute.as_execute();
    let rows = pool.fetch_all(execute).await?;
    let mut db_users = Vec::new();
    for row in &rows {
        db_users.push(User::from_row(row)?);
    }
    assert_eq!(db_users, users);

    // SQLite via `Any` driver
    install_default_drivers();
    let pool = AnyPool::connect("sqlite://db.sqlite").await?;

    let mut render_execute = user_query
        .render_execute()
        .map_err(|e| sqlx::Error::Encode(Box::new(e)))?;
    let execute = render_execute.as_execute();
    let rows = pool.fetch_all(execute).await?;
    let mut db_users = Vec::new();
    for row in &rows {
        db_users.push(User::from_row(row)?);
    }
    assert_eq!(db_users, users);

    // MySQL
    let pool = MySqlPool::connect("mysql://root:root@localhost/mysql").await?;

    let mut render_execute = user_query
        .render_execute()
        .map_err(|e| sqlx::Error::Encode(Box::new(e)))?;
    let execute = render_execute.as_execute();
    let rows = pool.fetch_all(execute).await?;
    let mut db_users = Vec::new();
    for row in &rows {
        db_users.push(User::from_row(row)?);
    }
    assert_eq!(db_users, users);
    Ok(())
}
```

### Advanced Query Example

```rust
#[derive(SqlTemplate)]
#[template(
    source = r#"
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
        AND name IN ({{el(filter_names)}})
    {% endif %}
    AND status IN ({{etl(*status_list)}})
    GROUP BY u.id
    ORDER BY {{e(order_field)}}
    LIMIT {{e(limit)}}
    "#,
    ext = "txt"
)]
#[addtype(i32)]
pub struct ComplexQuery<'a> {
    min_age: Option<i32>,
    #[ignore_type]
    filter_names: Vec<&'a str>,
    order_field: &'a str,
    limit: usize,
}
```

## Core Features

### Template Syntax

| Syntax            | Example                     | Description               |
|-------------------|-----------------------------|---------------------------|
| Single Parameter  | `{{e(user_id)}}`            | Bind a single parameter   |
| List Expansion    | `{{el(ids)}}`               | Expand for IN conditions  |
| Temp Variables    | `{% let limit = 100 %}`     | Define local variables    |
| Conditional Logic | `{% if active %}...{% endif %}` | Dynamic query building |

### Parameter Encoding Methods

| Method  | Description                  | Example             |
|---------|------------------------------|---------------------|
| `e()`   | Encode a single value        | `{{e(user_id)}}`    |
| `el()`  | Expand list as comma-separated | `{{el(ids)}}`     |
| `et()`  | Encode a template-local value | `{{et(limit)}}`  |
| `etl()` | Expand template-local list   | `{{etl(filters)}}` |

## Multi-Database Support

| Database    | Parameter Style | Example            |
|-------------|-----------------|--------------------|
| PostgreSQL  | $1, $2          | WHERE id = $1      |
| MySQL       | ?               | WHERE id = ?       |
| SQLite      | ?               | WHERE id = ?       |

## Macro Attributes

### `#[template]` - Core Template Attribute

```rust
#[derive(SqlTemplate)]
#[template(
    source = "SQL template content",  // Required
    ext = "txt",                      // Askama file extension
    print = "all",                    // Optional debug output (none/ast/code/all)
    config = "path"                   // Optional custom Askama config path
)]
```

**Parameters**:  
- `source`: Inline SQL template content (supports Askama syntax)  
- `ext`: File extension for Askama  
- `print`: Debug mode for Askama  
- `config`: Path to a custom Askama configuration file  

### `#[addtype]` - Add Type Constraints

```rust
#[derive(SqlTemplate)]
#[addtype(chrono::NaiveDate, Option<&'a str>)] // Add type constraints for template variables
```

**Functionality**:  
- Add `Encode + Type` constraints for non-field types used in templates  
- Supports multiple comma-separated types  

### `#[ignore_type]` - Skip Type Constraints

```rust
#[derive(SqlTemplate)]
struct Query {
    #[ignore_type]  // Skip type checking for this field
    metadata: JsonValue 
}
```

**Use Cases**:  
- Skip SQLx parameter binding for specific fields  
- Avoid unnecessary trait constraints for complex types  

## Complete Example

```rust
use std::collections::HashMap;

use sqlx::any::install_default_drivers;
use sqlx::{AnyPool, Arguments, MySqlPool};
use sqlx::{Executor, FromRow};
use sqlx_askama_template::SqlTemplate;

#[derive(SqlTemplate)]
#[addtype(Option<&'a i64>, bool)]
#[template(
    ext = "txt",
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
      AND arg_list1 IN {{el(arg4)}}
      {% let list=["abc".to_string()] %}
      AND arg_temp_list1 IN {{etl(*list)}}
      AND arg_list2 IN {{el(arg5.values())}}
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
    _arg1: i64, // Same type
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
}
```

## Best Practices

```markdown
1. Use `{% if %}` blocks for dynamic SQL segments  
2. Add template-local variable types with `addtype`  
3. Skip serialization for non-SQL fields using `ignore_type`  
4. Set `print = "none"` in production  
```

## License

MIT License © 2023 [Your Name]