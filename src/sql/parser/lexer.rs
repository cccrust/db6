//! SQL lexer（移植自 sql6/src/parser/lexer.rs）

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Token<'a> {
    Select, From, Where, And, Or, Not,
    Insert, Into, Values, Update, Set, Delete,
    Create, Table, Virtual, Using,
    Drop, Alter, Add, Column,
    Fts5, Match,
    OpenParen, CloseParen, Comma, Semicolon, Star, Dot,
    Eq, Ne, Lt, Le, Gt, Ge, Plus, Minus, Slash, Percent,
    String(&'a [u8]), Number(&'a [u8]), Ident(&'a [u8]), Param(&'a [u8]),
    Asc, Desc, Order, By, Group, Having, Limit, Offset,
    Join, Left, Right, Inner, Outer, On, As,
    Distinct, All, Between, In, Is, Null, True, False,
    Eof,
}

pub struct Lexer<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Lexer { input: input.as_bytes(), pos: 0 }
    }

    pub fn next_token(&mut self) -> Token<'a> {
        self.skip_whitespace();
        if self.pos >= self.input.len() {
            return Token::Eof;
        }
        let c = self.input[self.pos] as char;
        match c {
            '(' => { self.pos += 1; Token::OpenParen }
            ')' => { self.pos += 1; Token::CloseParen }
            ',' => { self.pos += 1; Token::Comma }
            ';' => { self.pos += 1; Token::Semicolon }
            '*' => { self.pos += 1; Token::Star }
            '.' => { self.pos += 1; Token::Dot }
            '=' => { self.pos += 1; Token::Eq }
            '<' => { self.pos += 1; Token::Lt }
            '>' => { self.pos += 1; Token::Gt }
            '+' => { self.pos += 1; Token::Plus }
            '-' => { self.pos += 1; Token::Minus }
            '/' => { self.pos += 1; Token::Slash }
            '%' => { self.pos += 1; Token::Percent }
            '"' | '\'' => self.read_string(c as u8),
            '`' => self.read_ident(),
            _ if c.is_ascii_digit() => self.read_number(),
            _ if c.is_alphabetic() || c == '_' => self.read_ident(),
            _ => { self.pos += 1; self.next_token() }
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len()
            && (self.input[self.pos] as char).is_whitespace()
        {
            self.pos += 1;
        }
    }

    fn read_string(&mut self, quote: u8) -> Token<'a> {
        let start = self.pos + 1;
        self.pos += 1;
        while self.pos < self.input.len()
            && self.input[self.pos] != quote
        {
            if self.input[self.pos] == b'\\' && self.pos + 1 < self.input.len() {
                self.pos += 2;
            } else {
                self.pos += 1;
            }
        }
        self.pos += 1;
        Token::String(&self.input[start..self.pos - 1])
    }

    fn read_number(&mut self) -> Token<'a> {
        let start = self.pos;
        while self.pos < self.input.len()
            && (self.input[self.pos].is_ascii_digit()
                || self.input[self.pos] == b'.'
                || self.input[self.pos] == b'e'
                || self.input[self.pos] == b'E'
                || self.input[self.pos] == b'-'
                || self.input[self.pos] == b'+')
        {
            self.pos += 1;
        }
        Token::Number(&self.input[start..self.pos])
    }

    fn read_ident(&mut self) -> Token<'a> {
        let start = self.pos;
        while self.pos < self.input.len()
            && (self.input[self.pos].is_ascii_alphanumeric() || self.input[self.pos] == b'_')
        {
            self.pos += 1;
        }
        let kw = &self.input[start..self.pos];
        match std::str::from_utf8(kw).map(|s| s.to_uppercase()).as_deref() {
            Ok("SELECT") => Token::Select,
            Ok("FROM") => Token::From,
            Ok("WHERE") => Token::Where,
            Ok("AND") => Token::And,
            Ok("OR") => Token::Or,
            Ok("NOT") => Token::Not,
            Ok("INSERT") => Token::Insert,
            Ok("INTO") => Token::Into,
            Ok("VALUES") => Token::Values,
            Ok("UPDATE") => Token::Update,
            Ok("SET") => Token::Set,
            Ok("DELETE") => Token::Delete,
            Ok("CREATE") => Token::Create,
            Ok("TABLE") => Token::Table,
            Ok("VIRTUAL") => Token::Virtual,
            Ok("USING") => Token::Using,
            Ok("DROP") => Token::Drop,
            Ok("ALTER") => Token::Alter,
            Ok("ADD") => Token::Add,
            Ok("COLUMN") => Token::Column,
            Ok("FTS5") => Token::Fts5,
            Ok("MATCH") => Token::Match,
            Ok("ASC") => Token::Asc,
            Ok("DESC") => Token::Desc,
            Ok("ORDER") => Token::Order,
            Ok("BY") => Token::By,
            Ok("GROUP") => Token::Group,
            Ok("HAVING") => Token::Having,
            Ok("LIMIT") => Token::Limit,
            Ok("JOIN") => Token::Join,
            Ok("LEFT") => Token::Left,
            Ok("RIGHT") => Token::Right,
            Ok("INNER") => Token::Inner,
            Ok("OUTER") => Token::Outer,
            Ok("ON") => Token::On,
            Ok("AS") => Token::As,
            Ok("DISTINCT") => Token::Distinct,
            Ok("ALL") => Token::All,
            Ok("BETWEEN") => Token::Between,
            Ok("IN") => Token::In,
            Ok("IS") => Token::Is,
            Ok("NULL") => Token::Null,
            Ok("TRUE") => Token::True,
            Ok("FALSE") => Token::False,
            _ => Token::Ident(kw),
        }
    }
}