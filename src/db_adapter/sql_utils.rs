use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Keyword {
    word: &'static str,
    is_truncate: bool,
}
const KEYWORDS: &[Keyword] = &[
    Keyword {
        word: "ORDER",
        is_truncate: true,
    }, // ORDER BY
    Keyword {
        word: "HAVING",
        is_truncate: false,
    }, // HAVING
    Keyword {
        word: "GROUP",
        is_truncate: false,
    }, // GROUP BY
    Keyword {
        word: "AND",
        is_truncate: false,
    }, // AND
    Keyword {
        word: "OR",
        is_truncate: false,
    }, // OR
    Keyword {
        word: "WHERE",
        is_truncate: false,
    }, // WHERE
    Keyword {
        word: "FROM",
        is_truncate: false,
    }, // FROM
];

static MAX_KEYWORD_LEN: LazyLock<usize> =
    LazyLock::new(|| KEYWORDS.iter().map(|k| k.word.len()).max().unwrap_or(0));
pub fn truncate_sql_at_outer_order_by(sql: &str) -> &str {
    // 定义要检测的关键字及其逆序形式，并标记是否为停止关键字

    // 计算最长逆序关键字的长度
    let max_keyword_len = *MAX_KEYWORD_LEN;

    let mut depth = 0i32;
    let mut in_string = false;
    let mut in_comment = false;
    let mut prev_char = None; // 用于检测注释边界
    let mut buffer = String::with_capacity(max_keyword_len);
    // let mut truncate_pos = None;

    // 保存字符及其原始位置的逆序迭代器
    // let rev_chars: Vec<char> = sql.chars().rev().collect();
    let mut char_indices = sql.char_indices();
    while let Some((i, c)) = char_indices.next_back() {
        // 处理多行注释
        if !in_string {
            if in_comment {
                // 检查是否退出注释（遇到原顺序的 /*，逆序的 */）
                if let Some(pc) = prev_char {
                    if c == '/' && pc == '*' {
                        in_comment = false;
                    }
                }
            } else {
                // 检查是否进入注释（遇到原顺序的 */，逆序的 /*）
                if let Some(pc) = prev_char {
                    if c == '*' && pc == '/' {
                        in_comment = true;
                    }
                }
            }
        }

        // 处理字符串
        if !in_comment && c == '\'' {
            in_string = !in_string;
        }

        // 处理括号深度
        if !in_comment && !in_string {
            match c {
                ')' => depth += 1,
                '(' => depth -= 1,
                _ => {}
            }
        }

        // 更新前一个字符
        prev_char = Some(c);

        // 构建缓冲区（仅当在有效状态时处理）
        let valid_state = !in_comment && !in_string && depth == 0;
        if valid_state {
            if buffer.len() == max_keyword_len {
                buffer.pop(); // 保持缓冲区长度不超过最大关键字长度
            }
            buffer.insert(0, c.to_ascii_uppercase());
        } else {
            buffer.clear();
        }

        // 检查关键字匹配
        if valid_state && !buffer.is_empty() {
            for keyword in KEYWORDS {
                if buffer.starts_with(keyword.word) {
                    // 检查前后边界

                    if sql.len() < i {
                        return sql;
                    }
                    // 检查前边界（原字符串中的后字符）
                    let prev_is_boundary = sql[i - 1..i].chars().fold(true, |_flag, c| {
                        println!("pre {}: {}", &sql[i - 1..i], c);
                        is_separator(c)
                    });

                    // 检查后边界（原字符串中的前字符）
                    if keyword.word.len() + 1 > buffer.len() {
                        return sql;
                    }
                    let next_is_boundary = buffer[keyword.word.len()..keyword.word.len() + 1]
                        .chars()
                        .fold(true, |_flag, c| {
                            println!(
                                "next {}: {}",
                                &buffer[keyword.word.len()..keyword.word.len() + 1],
                                c
                            );
                            is_separator(c)
                        });

                    if prev_is_boundary && next_is_boundary {
                        if keyword.is_truncate {
                            return &sql[..i];
                        } else {
                            return sql;
                        }
                    }
                }
            }
        }
    }
    sql
}

fn is_separator(c: char) -> bool {
    //!c.is_ascii_alphanumeric() && c != '_'
    c.is_whitespace()
}

// 测试用例
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_order_by() {
        let sql = "SELECT * FROM table\tORDER\nBY col /* ORDER BY */";
        assert_eq!(truncate_sql_at_outer_order_by(sql), "SELECT * FROM table\t");

        let sql = "SELECT * FROM (SELECT * FROM t ORDER BY a) ORDER BY b";
        assert_eq!(
            truncate_sql_at_outer_order_by(sql),
            "SELECT * FROM (SELECT * FROM t ORDER BY a) "
        );

        let sql = "SELECT * FROM t WHERE 'ORDER BY' = 'test' /* ORDER BY */ ORDER BY col";
        assert_eq!(
            truncate_sql_at_outer_order_by(sql),
            "SELECT * FROM t WHERE 'ORDER BY' = 'test' /* ORDER BY */ "
        );

        let sql = "SELECT * FROM t GROUP BY col";
        assert_eq!(
            truncate_sql_at_outer_order_by(sql),
            "SELECT * FROM t GROUP BY col"
        );

        let sql = "SELECT * FROM t HAVING count(1) > 0 ORDER  BY col";
        assert_eq!(
            truncate_sql_at_outer_order_by(sql),
            "SELECT * FROM t HAVING count(1) > 0 "
        );

        let sql = "SELECT * FROM t where id > 10 OrDeR bY name,id desc";
        assert_eq!(
            truncate_sql_at_outer_order_by(sql),
            "SELECT * FROM t where id > 10 "
        );
    }
}
