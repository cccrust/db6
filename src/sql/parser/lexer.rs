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
    String(&'a str), Number(&'a str), Ident(&'a str), Param(&'a str),
    Asc, Desc, Order, By, Group, Having, Limit, Offset,
    Join, Left, Right, Inner, Outer, On, As,
    Distinct, All, Between, In, Is, Null, True, False,
    Eof,
}

pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Lexer { input, pos: 0 }
    }

    pub fn next_token(&mut self) -> Token<'a> {
        self.skip_whitespace();
        if self.pos >= self.input.len() {
            return Token::Eof;
        }
        let c = self.input[self.pos..].chars().next().unwrap();
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
            '"' | '\'' => self.read_string(c),
            '`' => self.read_ident(),
            _ if c.is_ascii_digit() => self.read_number(),
            _ if c.is_alphabetic() || c == '_' => self.read_ident(),
            _ => { self.pos += 1; self.next_token() }
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len()
            && self.input[self.pos..].chars().next().map(|c| c.is_whitespace()).unwrap_or(false)
        {
            self.pos += 1;
        }
    }

    fn read_string(&mut self, quote: char) -> Token<'a> {
        let start = self.pos + 1;
        self.pos += 1;
        while self.pos < self.input.len()
            && self.input[self.pos..].chars().next().unwrap() != quote
        {
            if self.input[self.pos] == '\\' && self.pos + 1 < self.input.len() {
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
            && (self.input[self.pos].is_alphanumeric() || self.input[self.pos] == b'_')
        {
            self.pos += 1;
        }
        let kw = &self.input[start..self.pos];
        match kw.to_uppercase().as_str() {
            "SELECT" => Token::Select,
            "FROM" => Token::From,
            "WHERE" => Token::Where,
            "AND" => Token::And,
            "OR" => Token::Or,
            "NOT" => Token::Not,
            "INSERT" => Token::Insert,
            "INTO" => Token::Into,
            "VALUES" => Token::Values,
            "UPDATE" => Token::Update,
            "SET" => Token::Set,
            "DELETE" => Token::Delete,
            "CREATE" => Token::Create,
            "TABLE" => Token::Table,
            "VIRTUAL" => Token::Virtual,
            "USING" => Token::Using,
            "DROP" => Token::Drop,
            "ALTER" => Token::Alter,
            "ADD" => Token::Add,
            "COLUMN" => Token::Column,
            "FTS5" => Token::Fts5,
            "MATCH" => Token::Match,
            "ASC" => Token::Asc,
            "DESC" => Token::Desc,
            "ORDER" => Token::Order,
            "BY" => Token::By,
            "GROUP" => Token::Group,
            "HAVING" => Token::Having,
            "LIMIT" => Token::Limit,
            "JOIN" => Token::Join,
            "LEFT" => Token::Left,
            "RIGHT" => Token::Right,
            "INNER" => Token::Inner,
            "OUTER" => Token::Outer,
            "ON" => Token::On,
            "AS" => Token::As,
            "DISTINCT" => Token::Distinct,
            "ALL" => Token::All,
            "BETWEEN" => Token::Between,
            "IN" => Token::In,
            "IS" => Token::Is,
            "NULL" => Token::Null,
            "TRUE" => Token::True,
            "FALSE" => Token::False,
            _ => Token::Ident(kw),
        }
    }
}