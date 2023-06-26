use std::cell::RefCell;

use crate::compiler::{Compiler, CompilerConfig};

#[test]
fn simple_compilation() {
    let source =String::from(r##"
        ; should be ignored
        ignored = 1 + %101 * ($ff - 3)      ; also ignored
        .byte "HELLO WORLD"                 
        also_ignored:                       ; offset = 8 + 2 = 10 = $0a
        .dword "LLHH", $00ff
        LDA ($ff), y                        ;  official
        NOP
        BNE also_ignored
    "##);
    let mut compiler = Compiler::new(None);
    compiler.init_source(&source).unwrap();
    let hex_string = compiler.to_hex_string().unwrap();
    let bytes = compiler.to_byte_code().unwrap();
    assert_eq!(hex_string, "48 45 4c 4c 4f 20 57 4f 52 4c 44 4c 4c 48 48 ff 00 b1 ff ea d0 0a");
    assert_eq!(bytes, vec![
        72, 69, 76, 76, 79, 32, 87, 79, 82, 76, 68, 76, 76, 72, 72, 255, 0, 177, 255, 234, 208, 10
    ]);
}


#[test]
fn compile_illegal() {
    let source =String::from(r##"
        x = %010
        LAX #$a                        ; non official
        LDA ($f0 + (x * 8 - $1)), y    ; official
        NOP                            ; official, but let's emit non-official opcode
    "##);
    let mut compiler = Compiler::new(Some(CompilerConfig {
        allow_illegal: true,
        allow_list: RefCell::new(vec![
            0xDA // non official op
        ])
    }));
    compiler.init_source(&source).unwrap();
    let hex_string = compiler.to_hex_string().unwrap();
    let bytes = compiler.to_byte_code().unwrap();
    assert_eq!(hex_string, "ab 0a b1 ff da");
    assert_eq!(bytes, vec![
        171, 10, 177, 255, 218
    ]);
}

#[test]
fn mode_and_math_expansion() {
    let source =String::from(r##"
        ; zp => ASL $BB => 06 ae
        ASL $aa + 2 * %010

        ; impl => ASL => 0a
        ASL

        ; abs => ASL $LLHH => 0e aa bb
        ASL $bbaa + 2 * %010 - %100

        ; zpx => ASL $BB, x => 16 ae
        ASL $aa + 2 * %010, x

        ; absx => ASL $LLHH,X => 1e ae 00
        ASL $00aa + 2 * %010, x
    "##);
    let mut compiler = Compiler::new(None);
    compiler.init_source(&source).unwrap();
    let hex_string = compiler.to_hex_string().unwrap();
    assert_eq!(hex_string, "06 ae 0a 0e aa bb 16 ae 1e ae 00");

    // this test guarantees that it is possible
    // to differentiate (math_expr) to ((math_expr))
    // when context matters
    let source =String::from(r##"
        ; Note: although similar to the above example except the outer parenthesis, 
        ; this will not work, it implies indirect y
        ; which is is not supported by ASL
        ASL ($aa + 2 * %010), y
    "##);
    let mut compiler = Compiler::new(None);
    compiler.init_source(&source).unwrap();
    match compiler.to_hex_string() {
        Ok(_) => panic!("error was expected"),
        Err(s) => assert_eq!(s, "instruction (ASL, INDY) does not exist")
    }
}