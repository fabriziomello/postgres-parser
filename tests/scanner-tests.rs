use postgres_parser::{ScannedStatement, SqlStatementScanner};

#[test]
fn test_no_statements() {
    let mut statements = SqlStatementScanner::new("").into_iter();
    assert!(statements.next().is_none())
}

#[test]
fn test_only_whitespace() {
    let mut statements = SqlStatementScanner::new("   \n\r\n\t   ").into_iter();
    assert!(statements.next().is_none())
}

#[test]
fn test_null_statement() {
    let mut statements = SqlStatementScanner::new(";").into_iter();
    let first = statements.next().expect("no statements");
    assert!(first.parsetree.is_ok());
    assert!(first.parsetree.unwrap().is_none())
}

#[test]
fn test_single_statement() {
    let scanner = SqlStatementScanner::new("SELECT 1;");
    let statements: Vec<ScannedStatement> = scanner.into_iter().collect();

    assert_eq!(statements.len(), 1);

    let first = statements.get(0).unwrap();
    assert_eq!(first.sql, "SELECT 1;");
    assert!(first.payload.is_none());
    assert!(first.parsetree.is_ok());
}

#[test]
fn test_two_statements() {
    let mut scanner = SqlStatementScanner::new("SELECT 1;\nSELECT 2;").into_iter();

    let first = scanner.next().expect("no first query");
    assert_eq!(first.sql, "SELECT 1;\n"); // note trailing \n -- trailing whitespace after ';' is included
    assert!(first.payload.is_none());
    assert!(first.parsetree.is_ok());

    let second = scanner.next().expect("no second query");
    assert_eq!(second.sql, "SELECT 2;");
    assert!(second.payload.is_none());
    assert!(second.parsetree.is_ok());

    assert!(scanner.next().is_none());
}

#[test]
fn test_no_trailing_semicolon() {
    let stmt = SqlStatementScanner::new("SELECT 1")
        .into_iter()
        .next()
        .expect("failed to parse");
    assert_eq!(stmt.sql, "SELECT 1");
}

#[test]
fn test_single_quotes() {
    let stmt = SqlStatementScanner::new("SELECT 'single ;quotes';")
        .into_iter()
        .next()
        .expect("failed to parse");
    assert_eq!(stmt.sql, "SELECT 'single ;quotes';");
}

#[test]
fn test_double_quotes() {
    let stmt = SqlStatementScanner::new("SELECT \"double ;quotes\";")
        .into_iter()
        .next()
        .expect("failed to parse");
    assert_eq!(stmt.sql, "SELECT \"double ;quotes\";");
}

#[test]
fn test_dollar_quotes() {
    let stmt = SqlStatementScanner::new("SELECT $$dollar ;quotes$$;")
        .into_iter()
        .next()
        .expect("failed to parse");
    assert_eq!(stmt.sql, "SELECT $$dollar ;quotes$$;");
}

#[test]
fn test_named_dollar_quotes() {
    let stmt = SqlStatementScanner::new("SELECT $a$dollar ;quotes$a$;")
        .into_iter()
        .next()
        .expect("failed to parse");
    assert_eq!(stmt.sql, "SELECT $a$dollar ;quotes$a$;");
}

#[test]
fn test_single_line_comment() {
    let stmt = SqlStatementScanner::new("-- comment\nSELECT 1;")
        .into_iter()
        .next()
        .expect("failed to parse");
    assert_eq!(stmt.sql, "-- comment\nSELECT 1;");
}

#[test]
fn test_multi_line_comment1() {
    let stmt = SqlStatementScanner::new("/* comment */SELECT 1;")
        .into_iter()
        .next()
        .expect("failed to parse");
    assert_eq!(stmt.sql, "/* comment */SELECT 1;");
}

#[test]
fn test_multi_line_comment2() {
    let stmt = SqlStatementScanner::new("/* \ncomment\n */SELECT 1;")
        .into_iter()
        .next()
        .expect("failed to parse");
    assert_eq!(stmt.sql, "/* \ncomment\n */SELECT 1;");
}

#[test]
fn test_utf8() {
    let stmt = SqlStatementScanner::new("SELECT 'aⓐ' ~ U&'a\\24D0' AS t;")
        .into_iter()
        .next()
        .expect("failed to parse");
    assert_eq!(stmt.sql, "SELECT 'aⓐ' ~ U&'a\\24D0' AS t;");
}

#[test]
fn test_copy_data() {
    let stmt = SqlStatementScanner::new("COPY foo FROM STDIN;1\n2\n3\n\\.")
        .into_iter()
        .next()
        .expect("failed to parse");
    assert_eq!(stmt.sql, "COPY foo FROM STDIN;");
    assert!(stmt.payload.is_some());
    assert_eq!(stmt.payload.unwrap(), "1\n2\n3\n\\.");
}

/// this was a thing I ran into during development
/// where Postgres' "ErrorData" wasn't being reset
#[test]
fn test_5_errors() {
    let statements: Vec<ScannedStatement> = SqlStatementScanner::new(
        "one;
    two;
    three;
    four;
    five;
    SELECT 6;",
    )
    .into_iter()
    .collect();

    assert_eq!(statements.len(), 6);
    for (i, s) in statements.into_iter().enumerate() {
        if i == 5 {
            assert!(s.parsetree.is_ok())
        } else {
            assert!(s.parsetree.is_err())
        }
    }
}

#[test]
fn test_quoted_quotes() {
    let statements: Vec<ScannedStatement> =
        SqlStatementScanner::new("select '(\",a)'::textrange;select '(,,a)'::textrange;")
            .into_iter()
            .collect();

    assert_eq!(statements.len(), 2);
}

#[test]
fn test_escaped_single_quotes() {
    let statements: Vec<ScannedStatement> =
        SqlStatementScanner::new("select 'is''t';select 'that';select 'special';")
            .into_iter()
            .collect();

    assert_eq!(statements.len(), 3);
}
