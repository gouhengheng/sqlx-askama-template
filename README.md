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

## Installation  

Add to `Cargo.toml`:  

```toml  
[dependencies]  
sqlx-askama-template = "0.3.1"
sqlx = { version = "0.8", features = ["all-databases", "runtime-tokio"] }
tokio = { version = "1.0", features = ["full"] }
```  

## Quick Start  

### Basic Usage  

```rust 
use sqlx::{AnyPool, Error, any::install_default_drivers};

use sqlx_askama_template::{BackendDB, DBType, SqlTemplate};
#[derive(sqlx::prelude::FromRow, PartialEq, Eq, Debug)]
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
#[add_type(&'q str)]
pub struct UserQuery {
    pub user_id: i64,
    pub user_name: String,
}

async fn simple_query(urls: Vec<(DBType, &str)>) -> Result<(), Error> {
    let users = [
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
    for (db_type, url) in urls {
        let pool = AnyPool::connect(url).await?;
        // pool
        let user: Option<User> = user_query
            .adapter_render()
            .set_page(1, 1)
            .fetch_optional_as(&pool)
            .await?;
        assert_eq!(user.as_ref(), users.first());
        // connection
        let mut conn = pool.acquire().await?;
        let users1: Vec<User> = user_query
            .adapter_render()
            .set_page(1, 2)
            .fetch_all_as(&mut *conn)
            .await?;
        assert_eq!(users1, users[1..]);

        // tx
        let mut conn = pool.begin().await?;
        let (get_db_type, _get_conn) = conn.backend_db().await?;
        assert_eq!(db_type, get_db_type);

        let page_info = user_query
            .adapter_render()
            .count_page(1, &mut *conn)
            .await?;

        assert_eq!(page_info.total, 2);
        assert_eq!(page_info.page_count, 2);
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    unsafe {
        std::env::set_var("RUST_LOG", "sqlx_askama_template=DEBUG");
    }
    env_logger::init();

    install_default_drivers();
    let urls = vec![
        (
            DBType::PostgreSQL,
            "postgres://postgres:postgres@localhost/postgres",
        ),
        (DBType::SQLite, "sqlite://db.file?mode=memory"),
       // (DBType::MySQL, "mysql://root:root@localhost/mysql"),
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
| List Expansion      | `{{el(ids)}}`             | Expands to `IN (?, ?)`       |  
| Temporary Variables | `{% let limit = 100 %}`   | Defines template-local variables |  
| Conditional Logic   | `{% if active %}...{% endif %}` | Dynamic SQL conditions |  

### Parameter Encoding Methods  

| Method  | Description                   | Example               |  
|---------|-------------------------------|-----------------------|  
| `e()`   | Encodes a single value        | `{{e(user_id)}}`      |  
| `el()`  | Encodes a list (`$1, $2...`)  | `{{el(ids)}}`         |  
| `et()`  | Encodes a template-local value | `{{et(limit)}}`      |  
| `etl()` | Encodes a template-local list | `{{etl(filters)}}`   |  

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
#[add_type(chrono::NaiveDate, Option<&'a str>)]  // Add extra type support  
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
use std::collections::HashMap;  

use sqlx_askama_template::SqlTemplate;  

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
        AND name IN {{el(filter_names)}}  
    {% endif %}  
    AND status IN {{etl(*status_list)}}  
    GROUP BY u.id  
    ORDER BY {{order_field}}  
    LIMIT {{e(limit)}}  
    "#,  
    ext = "txt"  
)]  
#[add_type(i32)]  
pub struct ComplexQuery<'a> {  
    min_age: Option<i32>,  
    #[ignore_type]  
    filter_names: Vec<&'a str>,  
    order_field: &'a str,  
    limit: i64,  
}  

#[test]  
fn render_complex_sql() {  
    let data = ComplexQuery {  
        filter_names: vec!["name1", "name2"],  
        limit: 10,  
        min_age: Some(18),  
        order_field: "id",  
    };  

    let (sql, arg) =  
        <&ComplexQuery<'_> as SqlTemplate<'_, sqlx::Postgres>>::render_sql(&data).unwrap();  
    use sqlx::Arguments;  
    assert_eq!(arg.unwrap().len(), 6);  
    println!("----{sql}----");  
}  

#[derive(SqlTemplate)]  
#[add_type(Option<&'a i64>, bool)]  
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
    _arg1: i64,  // Same type  
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

#[test]  
fn render_query_data_sql() {  
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
    use sqlx::Arguments;  
    assert_eq!(arg.unwrap().len(), 18);  
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

This project is licensed under the [Apache License 2.0](LICENSE).  
Copyright © 2025 gouhengheng  

> **Important Notice**:  
> Under the Apache 2.0 License, you may not use this file except in compliance with the License.  
> You may obtain a copy of the License at:  
> <http://www.apache.org/licenses/LICENSE-2.0>  
>  
> Unless required by applicable law or agreed to in writing, software  
> distributed under the License is distributed on an "AS IS" BASIS,  
> WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.  
> See the License for specific governing permissions and limitations.  