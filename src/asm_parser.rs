use std::cmp::{min, max};
use std::collections::HashMap;

use crate::asm_lexer::Token;
use crate::opcodes::{
    Instr,
    AdrMode, INSTR
};

// https://famicom.party/book/05-6502assembly/
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    DIRECTIVE(Directive),
    ASSIGN(String, MathExpr), // lit, value
    LABEL(String),
    INSTR(Instr, AdrMode, Operand)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operand {
    NONE,               // implied
    LABEL(String),
    VALUE(NumericValue)  // label, variable, 1 or 2 bytes hex/dec/bin
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MathExpr {
    BIN(Token, Box<MathExpr>, Box<MathExpr>),
    PLACEHOLDER(String), NUM(NumericValue)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Directive {
    // TODO
    // EXPORT, INCLUDE(String),
    // ENDMACRO, MACRO(String, Vec<String>)   // .macro NAME arg1 arg2 ... argN (.*)\n endmacro
    /// .proc main 
    ENDPROC, PROC(String),
    /// .segment "NAME"
    SEGMENT(String),
    /// (.db | .byte) 1, 2, 3, ... 8 bit, can be strings
    BYTE(Vec<NumericValue>),
    /// .dw 1, 2, 3, ... (16 bits)
    DWORD(Vec<NumericValue>),
    /// .res N_BYTES
    RESERVE(usize)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericValue {
    pub value: u16,
    pub size: usize
}

fn canonicalize_number(n: &Token) -> Result<NumericValue, String> {
    match n {
        Token::BIN(bin) => {
            let value: u16 = u16::from_str_radix(bin, 2).unwrap();
            if bin.len() > 8 {
                return Ok(NumericValue { value, size: 16 })
            }
            Ok(NumericValue { value, size: 8 })
        },
        Token::DEC(dec) => {
            let value: u16 = u16::from_str_radix(dec, 10).unwrap();
            // ex: 256 or 00001 shall be considered as 16 bits
            if value > 255 || dec.len() > 3 {
                return Ok(NumericValue { value, size: 16 })
            }
            Ok(NumericValue { value, size: 8 })
        },
        Token::HEX(hex) => {
            let value: u16 = u16::from_str_radix(hex, 16).unwrap();
            if hex.len() > 2 {
                return Ok(NumericValue { value, size: 16 })
            }
            Ok(NumericValue { value, size: 8 })
        },
        Token::CHAR(ch) => {
            let value: u16 = ch.chars().next().unwrap() as u16;
            Ok(NumericValue { value, size: 8 })
        },
        token => {
            Err(format!("operand next {:?} is not a number", token))
        }
    }
}


fn get_instr(s: &String) -> Result<Instr, String> {
    match INSTR.get(&s.to_uppercase()) {
        Some(i) => Ok(i.to_owned()),
        None => Err(format!("{:?} is not a valid instruction", s))
    }
}

fn is_branching(i: &Instr) -> bool {
    let list = [
        Instr::BPL, Instr::BMI, Instr::BVC,
        Instr::BVS, Instr::BCC, Instr::BCS,
        Instr::BNE, Instr::BEQ
    ];
    for item in list {
        if item.to_string().eq(&i.to_string()) {
            return true;
        }
    }
    false
}


pub struct AsmParser<'a> {
    tokens: &'a Vec<Token>,
    cursor: usize,
    variables: HashMap<String, MathExpr> 
}

impl<'a> AsmParser<'a> {
    pub fn new(tokens: &'a Vec<Token>) -> Self {
        Self {
            tokens,
            cursor: 0,
            variables: HashMap::new()
        }
    }

    pub fn parse(&mut self) -> Result<Vec<Expr>, String> {
        let mut prog = Vec::new();
        self.cursor = 0;
        loop {
            // println!("{:?}", self.curr().clone());
            // cleanup
            if self.is_eof() {
                break;
            }
            if self.is_endline() || self.is_comment() {
                self.next();
                continue;
            }

            // assign
            if *self.peek_next() == Token::EQUAL {
                match self.curr() {
                    Token::LITERAL(_) => {
                        prog.push(self.state_assign()?);
                        continue;
                    },
                    _ => {
                        return Err("assign expression expects a literal (lhs) / expression(rhs)".to_string())    
                    }
                }
            }

            // label declaration
            if *self.peek_next() == Token::COLON {
                match self.curr() {
                    Token::LITERAL(_) => {
                        prog.push(self.state_label()?);
                        continue;
                    },
                    _ => {
                        return Err("label expression expects a literal (lhs) / expression(rhs)".to_string())    
                    }
                }
            }

            // directive and comment
            match self.curr().clone() {
                Token::DIRECTIVE(name) => {
                    match name.as_str() {
                        "byte" | "BYTE" | "db" | "DB" => {
                            self.next();
                            let seq = self.consume_sequence(8)?;
                            prog.push(Expr::DIRECTIVE(Directive::BYTE(seq)));
                        },
                        "dword" | "DWORD" | "dw" | "DW" => {
                            self.next();
                            let seq = self.consume_sequence(16)?;
                            prog.push(Expr::DIRECTIVE(Directive::DWORD(seq)));
                        },
                        "segment" => {
                            self.next();
                            let segname: String = self.consume_string_and_lift()?;
                            prog.push(Expr::DIRECTIVE(Directive::SEGMENT(segname)));
                        },
                        "proc" => {
                            self.next();
                            let procname: String = self.consume_literal_and_lift()?;
                            prog.push(Expr::DIRECTIVE(Directive::PROC(procname)));
                        },
                        "endproc" => {
                            self.next();
                            prog.push(Expr::DIRECTIVE(Directive::ENDPROC));
                        },
                        "res" => {
                            self.next();
                            match self.curr() {
                                Token::DEC(n) => {
                                    let size = usize::from_str_radix(&n, 10).unwrap();
                                    self.next();
                                    prog.push(Expr::DIRECTIVE(Directive::RESERVE(size)));
                                },
                                tk => {
                                    return Err(format!("decimal number was expected, got {:?}", tk));
                                }
                            }
                        },
                        _ => {
                            return Err(self.curr_unexpected());
                        }
                    }
                    continue;
                },
                Token::COMMENT(..) => {
                    self.next();
                    continue;
                },
                _ => {}
            }
            // instruction
            prog.push(self.state_instr()?);
            self.next();
        }
        Ok(prog)
    }

    fn curr(&self) -> &Token {
        self
            .tokens
            .get(self.cursor)
            .unwrap_or(&Token::EOF)
    }

    fn peek_next(&self) -> &Token {
        self
            .tokens
            .get(self.cursor + 1)
            .unwrap_or(&Token::EOF)
    }

    fn is_eof(&self) -> bool {
        *self.curr() == Token::EOF 
    }

    fn is_endline(&self) -> bool {
        *self.curr() == Token::NEWLINE
    }

    fn is_comment(&self) -> bool {
        match *self.curr() {
            Token::COMMENT(..) => true,
            _ => false
        }
    }

    fn next(&mut self) -> &Token {
        self.cursor = min(self.tokens.len(), self.cursor + 1);
        return self.curr();
    }

    fn curr_unexpected(&self) -> String {
        format!("token {:?} unexpected", self.curr())
    }

    fn consume(&mut self, token: Token) -> Result<Token, String> {
        if *self.curr() == token {
            self.next();
            return Ok(token);
        }
        Err(format!("{:?} was expected, got {:?} instead", token, *self.curr()))
    }

    fn consume_literal_and_lift(&mut self) -> Result<String, String> {
        let curr = self.curr().clone();
        match curr {
            Token::LITERAL(lit) => {
                self.next();
                Ok(lit)
            },
            token => Err(format!("literal was expected, got {:?} instead",token))
        }
    }

    fn consume_string_and_lift(&mut self) -> Result<String, String> {
        let curr = self.curr().clone();
        match curr {
            Token::STR(s) => {
                self.next();
                Ok(s)
            },
            token => Err(format!("string was expected, got {:?} instead", token))
        }
    }

    fn consume_literal(&mut self, s: &str) -> Result<Token, String> {
        let curr = self.curr().clone();
        match &curr {
            Token::LITERAL(lit) => {
                if lit.ne(s) {
                    return Err(format!("literal {:?} was expected, got {:?} instead", s, curr))
                }
                let curr = self.curr().clone();
                self.next();
                Ok(curr)
            },
            token => Err(format!("literal {:?} was expected, got {:?} instead", s, token))
        }
    }

    fn consume_sequence(&mut self, size: usize) -> Result<Vec<NumericValue>, String>  {
        if size != 8 && size != 16 {
            return Err(format!("size must be 8 or 16, {} was given", size));
        }
        let mut seq: Vec<NumericValue> = vec![];
        while !self.is_eof() && !self.is_endline() && !self.is_comment() {
            match self.curr() {
                Token::STR(s) => {
                    if size == 16 {
                        // consume per block of 2 chars
                        if s.len() % 2 != 0 {
                            return Err(format!("length of {:?} must be a multiple of 2 to form a 2 byte word", s));
                        }
                        let list: Vec<char> = s.chars().collect();
                        let mut pos = 0;
                        while pos < list.len() {
                            let hi = list[pos] as u16;
                            let lo = list[pos + 1] as u16;
                            let value = (hi << 8) | lo;
                            seq.push(NumericValue {value, size});
                            pos += 2;
                        }
                    } else {
                        // == 8
                        for ch in s.chars() {
                            let value = ch as u16;
                            seq.push(NumericValue { value, size: 8 });
                        }
                    }
                    self.next();
                },
                _ => {
                    // Note: char is also a valid math operand
                    let expr = self.consume_math_expr()?;
                    let mut value = self.eval_math(&expr)?;
                    if value.size > size {
                        let pos = seq.len();
                        return Err(format!(
                            "{}-th value is {} bytes, {} was expected", 
                            pos, 
                            max(1, value.size / 8), 
                            size / 8
                        ));
                    }
                    // promote for values if given size is bigger
                    value.size = max(value.size, size);
                    seq.push(value);
                }
            }

            if *self.curr() == Token::COMMA {
                self.consume(Token::COMMA)?;
            }
        }

        Ok(seq)
    }

    // expr      ::= term (+| -) expr | term
    fn consume_math_expr(&mut self) -> Result<MathExpr, String> {
        let expr = self.consume_math_term()?;
        let bin = vec![Token::PLUS, Token::MINUS];
        for op in bin {
            if *self.curr() == op {
                let op_token = self.consume(op)?;
                let right = self.consume_math_expr()?;
                return Ok(MathExpr::BIN(op_token, Box::new(expr), Box::new(right)));
            }
        }
        Ok(expr)
    }

    fn try_expand_math(&mut self) -> Result<NumericValue, String> {
        match self.consume_math_expr() {
            Ok(expr) => self.eval_math(&expr),
            Err(_) => {
                let ret = canonicalize_number(self.curr())?;
                self.next();
                Ok(ret)
            }
        }
    }

    // term      ::= factor (* | /) term | factor
    fn consume_math_term(&mut self) -> Result<MathExpr, String> {
        let expr = self.consume_math_factor()?;
        let bin = vec![Token::MULT, Token::DIV];
        for op in bin {
            if *self.curr() == op {
                let op_token = self.consume(op)?;
                let right = self.consume_math_term()?;
                return Ok(MathExpr::BIN(op_token, Box::new(expr), Box::new(right)));
            }
        }
        Ok(expr)
    }

    // factor    ::= (expr) | unary
    fn consume_math_factor(&mut self) -> Result<MathExpr, String> {
        if *self.curr() == Token::PARENTOPEN {
            self.consume(Token::PARENTOPEN)?;
            let expr = self.consume_math_expr()?;
            self.consume(Token::PARENTCLOSE)?;
            return Ok(expr);
        }
        self.consume_math_unary()
    }

    // unary     ::= <literal> | hex | dec | bin
    fn consume_math_unary(&mut self) -> Result<MathExpr, String> {
        match canonicalize_number(&self.curr()) {
            Ok(number) => {
                self.next();
                Ok(MathExpr::NUM(number))
            },
            Err(e) => {
                match &self.curr() {
                    Token::LITERAL(s) => {
                        let out = MathExpr::PLACEHOLDER(s.to_string());
                        self.next();
                        Ok(out)
                    },
                    _ => {
                        Err(e)
                    }
                }
            }
        }
    }

    // expr should guarantee to be not recursive
    pub fn eval_math(&self, expr: &MathExpr) -> Result<NumericValue, String> {
        match expr {
            MathExpr::BIN(op, lvalue, rvalue) => {
                let left = self.eval_math(&lvalue)?;
                let right = self.eval_math(&rvalue)?;
                let value = match op {
                    Token::PLUS => {
                        if left.value.checked_add(right.value).is_none() {
                            return Err(format!("add overflow: left {}, right {}", left.value, right.value));
                        }
                        Ok(left.value + right.value)
                    },
                    Token::MULT => {
                        if left.value.checked_mul(right.value).is_none() {
                            return Err(format!("multiplication overflow: left {}, right {}", left.value, right.value));
                        }
                        Ok(left.value * right.value)
                    },
                    Token::MINUS => {
                        if left.value.checked_sub(right.value).is_none() {
                            return Err(format!("substraction overflow: left {}, right {}", left.value, right.value));
                        }
                        Ok(left.value - right.value)
                    },
                    Token::DIV => {
                        if left.value.checked_div(right.value).is_none() {
                            return Err(format!("cannot divide {} by zero", left.value));
                        }
                        Ok(left.value / right.value)
                    }
                    token => Err(format!("binary operator {:?} not implemented", token))
                }?;
                Ok(NumericValue { value, size: max(left.size, right.size)})
            },
            MathExpr::NUM(n) => Ok(n.clone()),
            MathExpr::PLACEHOLDER(s) => {
                let nested = self.variables.get(s);
                if nested.is_some() {
                    return self.eval_math(nested.unwrap());
                }
                Err(format!("variable {:?} is undefined", s))
            },
        }
    }

    pub fn validate_factors(&self, expr: &MathExpr, assignee: &Option<String>) -> Result<bool, String> {
        match expr {
            MathExpr::NUM(_) => Ok(true),
            MathExpr::BIN(_, lvalue, rvalue) => {
                let left = self.validate_factors(&lvalue, assignee)?;
                let right = self.validate_factors(&rvalue, assignee)?;
                Ok(left && right)
            },
            MathExpr::PLACEHOLDER(s) => {
                if assignee.is_some() && s.to_owned() == assignee.clone().unwrap() {
                    return Err(format!("variable {:?} has recursive definition", s))
                }
                let nested = self.variables.get(s);
                if nested.is_some() {
                    return self.validate_factors(nested.unwrap(), assignee);
                }
                Err(format!("variable {:?} is undefined", s))
            },
        }
    }

    fn state_assign(&mut self) -> Result<Expr, String> {
        let symbol = self.consume_literal_and_lift()?;
        self.consume(Token::EQUAL)?;
        let number = self.consume_math_expr()?;
        if !self.validate_factors(&number, &Some(symbol.clone()))? {
            return Err(format!("{} rhs is not valid", symbol));
        }
        self.variables.insert(symbol.clone(), number.clone());
        Ok(Expr::ASSIGN(symbol, number))
    }

    fn state_label(&mut self) -> Result<Expr, String> {
        let name = self.consume_literal_and_lift()?;
        self.consume(Token::COLON)?;
        Ok(Expr::LABEL(name))
    }

    /// Follow the grammar \
    /// [none ::= implied, accumulator] \
    /// operand ::= none | imm | abs | ind | rel | zp \
    /// imm     ::= #$BB\
    /// ind     ::= '(' $LLHH ')' | '(' $BB ',' 'x' ')' | '(' $BB  ')' ',' 'y' \
    /// rel     ::= $BB                                  (context bound: only for jumps BXX) \
    /// zp      ::= $BB | $BB ',' ('x'|'y') \
    /// abs     ::= $LLHH | $LLHH ',' ('x'|'y') \
    fn state_instr(&mut self) -> Result<Expr, String> {
        let instr = match self.curr().clone() {
            Token::LITERAL(i) => Ok(get_instr(&i)?),
            token => Err(format!("{:?} is not a literal", token))
        }?;
        self.next();

        // none
        if self.is_endline() || self.is_eof() || self.is_comment() {
            return Ok(Expr::INSTR(instr, AdrMode::IMPL, Operand::NONE));
        }

        // branching BXX
        if is_branching(&instr) {
            match canonicalize_number(self.curr()) {
                Ok(number) => {
                    let op = Operand::VALUE(number);
                    return Ok(Expr::INSTR(instr, AdrMode::REL, op));
                },
                Err(e) => {
                    match self.curr() {
                        Token::LITERAL(s) => {
                            let op = Operand::LABEL(s.clone());
                            return Ok(Expr::INSTR(instr, AdrMode::REL, op));
                        },
                        _ => { return Err(e) }
                    }
                }
            }
        }

        // immidiate
        if *self.curr() == Token::HASH {
            self.consume(Token::HASH)?;
            let expr = &self.consume_math_expr()?;
            let number = self.eval_math(expr)?;
            let op = Operand::VALUE(number);
            return Ok(Expr::INSTR(instr, AdrMode::IMM, op));
        }

        // ind, indx, indy
        if *self.curr() == Token::PARENTOPEN {
            // indirect
            self.consume(Token::PARENTOPEN)?;
            let number = self.try_expand_math()?;
            if number.size > 8 {
                let op = Operand::VALUE(number);
                self.consume(Token::PARENTCLOSE)?;
                return Ok(Expr::INSTR(instr, AdrMode::IND, op));
            } else {
                let op = Operand::VALUE(number);
                if *self.curr() == Token::COMMA {
                    // indirect x
                    self.consume(Token::COMMA)?;
                    self.consume_literal("x")?;
                    self.consume(Token::PARENTCLOSE)?;
                    return Ok(Expr::INSTR(instr, AdrMode::INDX, op));
                } else {
                    // indirect y
                    self.consume(Token::PARENTCLOSE)?;
                    self.consume(Token::COMMA)?;
                    self.consume_literal("y")?;
                    return Ok(Expr::INSTR(instr, AdrMode::INDY, op));
                }
            }
        }

        // abs and zp
        let number = self.try_expand_math()?;
        if number.size > 8 {
            // abs
            let op = Operand::VALUE(number);
            let mut mode = AdrMode::ABS;
            if *self.curr() == Token::COMMA {
                self.consume(Token::COMMA)?;
                match self.consume_literal("x") {
                    Ok(_) =>  { mode = AdrMode::ABSX },
                    Err(_) => {
                        self.consume_literal("y")?;
                        mode = AdrMode::ABSY;
                    }
                };
            }
            return Ok(Expr::INSTR(instr, mode, op));
        } else {
            // zp
            let op = Operand::VALUE(number);
            let mut mode = AdrMode::ZP;
            if *self.curr() == Token::COMMA {
                self.consume(Token::COMMA)?;
                match self.consume_literal("x") {
                    Ok(_) =>  { mode = AdrMode::ZPX },
                    Err(_) => {
                        self.consume_literal("y")?;
                        mode = AdrMode::ZPY;
                    }
                };
            }
            return Ok(Expr::INSTR(instr, mode, op));
        }
    }
}