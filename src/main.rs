// This file is part of the No# (Nosharp) programming language and is licensed under MIT License; see LICENSE.txt for details

mod commands;
mod typechecker;
mod syntax;
mod wasm;

use wasm::WasmCompiler;
use std::collections::HashMap;

use commands::lower_program;
use logos::Logos;
use serde::Serialize;
use syntax::{Token, parse_code, render_diagnostic};

#[derive(Serialize)]
struct CliResult {
    success: bool,
    commands: Option<Vec<commands::Command>>,
    errors: Vec<CliDiagnostic>,
}

#[derive(Serialize)]
struct CliDiagnostic {
    message: String,
    start: usize,
    end: usize,
    help: Option<String>,
    rendered: Option<String>,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let file_path = args.get(1).map(|s| s.as_str()).unwrap_or("test.nos");
    let source_code = std::fs::read_to_string(file_path).unwrap_or_else(|_| panic!("Failed to read file: {file_path}"));

    let mut lex_errors = Vec::new();
    let tokens: Vec<Token> = Token::lexer(&source_code)
        .spanned()
        .filter_map(|(res, span)| match res {
            Ok(token) => Some(token),
            Err(_) => {
                let help = "Remove or replace this character.";
                lex_errors.push(CliDiagnostic {
                    message: "unknown token".to_string(),
                    start: span.start,
                    end: span.end,
                    help: Some(help.to_string()),
                    rendered: render_diagnostic(
                        &source_code,
                        "lex error",
                        span.start,
                        span.end,
                        "this character is not valid No# syntax",
                        Some(help),
                    ),
                });

                None
            }
        }).collect();

    if !lex_errors.is_empty() {
        print_result(CliResult {
            success: false,
            commands: None,
            errors: lex_errors,
        });

        return;
    }

    let ast = match parse_code(&tokens, &source_code) {
        Ok(ast) => ast,
        Err(errors) => {
            print_result(CliResult {
                success: false,
                commands: None,
                errors: errors
                    .into_iter()
                    .map(|error| CliDiagnostic {
                        message: error.message,
                        start: error.start,
                        end: error.end,
                        help: error.help,
                        rendered: error.rendered,
                    }).collect(),
            });

            return;
        }
    };

    let commands = lower_program(&ast);
    let errors = typechecker::check_program(&ast, HashMap::new())
        .into_iter()
        .map(|error| CliDiagnostic {
            message: error.message,
            start: error.start,
            end: error.end,
            help: error.help,
            rendered: error.rendered,
        }).collect::<Vec<_>>();

    if errors.is_empty() {
        let wasm_bytes = WasmCompiler::compile_program(&commands);
        std::fs::write("output.wasm", &wasm_bytes).expect("Failed to write output.wasm");
        println!("Successfully generated output.wasm! ({} bytes)", wasm_bytes.len());
    }

    print_result(CliResult {
        success: errors.is_empty(),
        commands: Some(commands),
        errors,
    });
}

fn print_result(result: CliResult) {
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
}
