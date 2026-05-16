//! SQL parser — 移植自 sql6/src/parser/parser.rs
//!
//! 需完整移植後才能使用完整 SQL 語法。

use crate::sql::parser::ast::*;
use crate::sql::parser::lexer::Lexer;
use crate::sql::parser::lexer::Token;
use std::io;

pub fn parse(sql: &str) -> io::Result<Vec<Statement>> {
    let mut lexer = Lexer::new(sql);
    let mut stmts = Vec::new();
    loop {
        let tok = lexer.next_token();
        if tok == Token::Eof {
            break;
        }
        let stmt = parse_statement(&mut lexer, tok)?;
        stmts.push(stmt);
    }
    Ok(stmts)
}

fn parse_statement(lexer: &mut Lexer, tok: Token) -> io::Result<Statement> {
    match tok {
        Token::Select => parse_select(lexer).map(Statement::Select),
        Token::Insert => parse_insert(lexer).map(Statement::Insert),
        Token::Update => parse_update(lexer).map(Statement::Update),
        Token::Delete => parse_delete(lexer).map(Statement::Delete),
        Token::Create => parse_create(lexer),
        Token::Drop => parse_drop(lexer).map(Statement::DropTable),
        _ => Err(io::Error::new(io::ErrorKind::InvalidInput, format!("unexpected token: {:?}", tok))),
    }
}

fn parse_select(lexer: &mut Lexer) -> io::Result<SelectStmt> {
    let columns = parse_columns(lexer)?;
    let from = if lexer.next_token() == Token::From {
        let t = lexer.next_token();
        if let Token::Ident(s) = t {
            Some(s.to_string())
        } else {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "expected table name"));
        }
    } else {
        None
    };
    Ok(SelectStmt {
        columns,
        from,
        where_clause: None,
        group_by: None,
        order_by: None,
        limit: None,
    })
}

fn parse_columns(lexer: &mut Lexer) -> io::Result<Vec<Expr>> {
    let mut cols = Vec::new();
    loop {
        let tok = lexer.next_token();
        match tok {
            Token::Star => cols.push(Expr::Wildcard),
            Token::Ident(s) => cols.push(Expr::Column(s.to_string())),
            Token::Eof | Token::From | Token::Where | Token::Order | Token::Group
                | Token::Limit | Token::Semicolon => {
                break;
            }
            _ => {}
        }
        let peek = lexer.next_token();
        if peek != Token::Comma {
            break;
        }
    }
    Ok(cols)
}

fn parse_insert(_lexer: &mut Lexer) -> io::Result<InsertStmt> {
    todo!("完整實作請移植 sql6/src/parser/parser.rs")
}

fn parse_update(_lexer: &mut Lexer) -> io::Result<UpdateStmt> {
    todo!("完整實作請移植 sql6/src/parser/parser.rs")
}

fn parse_delete(_lexer: &mut Lexer) -> io::Result<DeleteStmt> {
    todo!("完整實作請移植 sql6/src/parser/parser.rs")
}

fn parse_create(lexer: &mut Lexer) -> io::Result<Statement> {
    let tok = lexer.next_token();
    match tok {
        Token::Table => parse_create_table(lexer).map(Statement::CreateTable),
        Token::Virtual => parse_create_virtual_table(lexer).map(Statement::CreateVirtualTable),
        _ => Err(io::Error::new(io::ErrorKind::InvalidInput, format!("unexpected: {:?}", tok))),
    }
}

fn parse_create_table(_lexer: &mut Lexer) -> io::Result<CreateTableStmt> {
    todo!("完整實作請移植 sql6/src/parser/parser.rs")
}

fn parse_create_virtual_table(lexer: &mut Lexer) -> io::Result<CreateVirtualTableStmt> {
    // CREATE VIRTUAL TABLE <name> USING fts5(<cols>, tokenize='cjk')
    let name = match lexer.next_token() {
        Token::Ident(s) => s.to_string(),
        _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "expected table name")),
    };

    let tok = lexer.next_token();
    if tok != Token::Using {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "expected USING"));
    }

    let tok = lexer.next_token();
    if tok != Token::Fts5 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "expected FTS5"));
    }

    Ok(CreateVirtualTableStmt {
        name,
        columns: vec![],
        tokenize: "cjk".to_string(),
    })
}

fn parse_drop(_lexer: &mut Lexer) -> io::Result<DropTableStmt> {
    todo!("完整實作請移植 sql6/src/parser/parser.rs")
}