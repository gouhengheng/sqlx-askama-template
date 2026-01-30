# SQLx Askama Template

[![Crates.io](https://img.shields.io/crates/v/sqlx-askama-template)](https://crates.io/crates/sqlx-askama-template)
[![Documentation](https://docs.rs/sqlx-askama-template/badge.svg)](https://docs.rs/sqlx-askama-template)
[![GitHub License](https://img.shields.io/github/license/gouhengheng/sqlx-askama-template)](https://github.com/gouhengheng/sqlx-askama-template)
[![GitHub Stars](https://img.shields.io/github/stars/gouhengheng/sqlx-askama-template)](https://github.com/gouhengheng/sqlx-askama-template/stargazers)
[![GitHub Issues](https://img.shields.io/github/issues/gouhengheng/sqlx-askama-template)](https://github.com/gouhengheng/sqlx-askama-template/issues)
[![CI Status](https://github.com/gouhengheng/sqlx-askama-template/actions/workflows/ci.yml/badge.svg)](https://github.com/gouhengheng/sqlx-askama-template/actions)  

一个基于 Askama 模板引擎的 SQLx 查询构建器，提供类型安全的 SQL 模板和参数绑定。

## 特性

- 🚀 **零成本抽象** - 编译时生成高效 SQL  
- 🔒 **类型安全** - 自动验证 SQL 参数类型  
- 📦 **多数据库支持** - PostgreSQL/MySQL/SQLite/Any  
- 💡 **智能参数绑定** - 自动处理列表参数展开  
- 🎨 **模板语法** - 支持完整的 Askama 模板功能  

## 要求
sqlx > 0.9.0-alpha.1

## 安装

在 `Cargo.toml` 中添加：


```toml  
[dependencies]  
sqlx-askama-template = "0.4.0-alpha.1"
tokio = { version = "1.0", features = ["full"] }
sqlx = { version = "0.9.0-alpha.1", default-features = false, features = [
    "all-databases",
    "runtime-tokio",
    "macros",
] }
env_logger="0.11"
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

    let mut user_query = UserQuery {
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
        user_query.user_id = 9999;
        let page_info = user_query
            .adapter_render()
            .count_page(1, &mut *conn)
            .await?;
        user_query.user_id = 1;

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
    tokio::spawn(async { println!("started") });
    install_default_drivers();
    let urls = vec![
        (
            DBType::PostgreSQL,
            "postgres://postgres:postgres@localhost/postgres",
        ),
        (DBType::SQLite, "sqlite://db.file?mode=memory"),
        //(DBType::MySQL, "mysql://root:root@localhost/mysql"),
    ];
    simple_query(urls).await?;

    Ok(())
}

```  


## 核心功能

### 模板语法

| 语法           | 示例                     | 描述               |
|----------------|--------------------------|--------------------|
| 单参数绑定     | `{{e(user_id)}}`     | 绑定单个参数       |
| 列表展开       | `{{el(ids)}}`        | 展开为 IN (?,?) 条件     |


### 参数编码方法

| 方法   | 描述                      | 示例             |
|--------|---------------------------|------------------|
| `e()`  | 编码单个值                | `{{e(user_id)}}` |
| `el()` | 编码一个列表($1,$2..$n)    | `{{el(ids)}}` |


## 多数据库支持

| 数据库       | 参数样式  | 示例               |
|-------------|-----------|--------------------|
| PostgreSQL  | $1, $2    | WHERE id = $1      |
| MySQL       | ?         | WHERE id = ?       |
| SQLite      | ?         | WHERE id = ?       |



## 宏属性说明

### `#[template]` - 核心模板属性

```rust
#[derive(SqlTemplate)]
#[template(
    source = "SQL模板内容",  // 必需
    ext = "txt",           // askam文件扩展名
    print = "all",         // 可选，调试输出(none,ast,code,all)
    config = "path"        // 可选，自定义Askama配置路径
)]
```

**参数说明**：
- `source`: 直接内联的SQL模板内容（支持Askama语法）
- `ext`: askam文件扩展名
- `print`: askama调试模式
- `config`: 指向自定义Askama配置文件的路径

### `#[addtype]` - 添加额外类型约束，一般用于给Vec<T>,HashMap<K,V>,模板内部声明变量等情况添加数据库Enocde约束

```rust
#[derive(SqlTemplate)]
#[addtype(chrono::NaiveDate, Option<&'q str>)] // 为模板添加额外类型支持
```

**功能**：
- 为模板中使用的非字段类型添加`Encode + Type`约束
- 支持逗号分隔的多个类型

### `#[ignore_type]` - 忽略字段类型,不会添加数据库Enocde约束

```rust
#[derive(SqlTemplate)]
struct Query {
    #[ignore_type]  // 跳过该字段的类型检查
    metadata: JsonValue 
}
```

**使用场景**：
- 跳过不需要SQLx参数绑定的字段
- 避免为复杂类型生成不必要的trait约束

## 完整使用示例

```rust
use std::collections::HashMap;  

use sqlx_askama_template::SqlTemplate;  

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
```

## 最佳实践

```markdown
1. 对动态SQL部分使用`{% if %}`条件块
2. 用`addtype`添加模板局部变量类型
3. 用`ignore_type`跳过序列化字段
4. 生产环境设置`print = "none"`
```

## 许可证

-   Apache License, Version 2.0
    ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
-   MIT license
    ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
