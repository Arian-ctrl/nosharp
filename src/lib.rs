// This file is part of the No# (Nosharp) programming language and is licensed under MIT License; see LICENSE.txt for details

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use logos::Logos;
use serde::Serialize;

mod commands;
mod typechecker;
mod syntax;
mod wasm;

use commands::lower_program;
use syntax::{Token, parse_code, render_diagnostic};
use wasm::WasmCompiler;

#[derive(Serialize)]
pub struct Diagnostic {
    pub message: String,
    pub start: usize,
    pub end: usize,
    pub help: Option<String>,
    pub rendered: Option<String>,
}

#[repr(C)]
pub struct WasmCompileResult {
    pub success: bool,
    pub wasm_ptr: *const u8,
    pub wasm_len: usize,
    pub errors_json: *const c_char,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nosharp_compile_to_wasm(source_ptr: *const c_char, globals_ptr: *const c_char,) -> WasmCompileResult {
    if source_ptr.is_null() {
        return WasmCompileResult {
            success: false,
            wasm_ptr: std::ptr::null(),
            wasm_len: 0,
            errors_json: CString::new("[{\"message\":\"Null source pointer\"}]").unwrap().into_raw(),
        };
    }

    let c_str = unsafe { CStr::from_ptr(source_ptr) };
    let code = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => {
            return WasmCompileResult {
                success: false,
                wasm_ptr: std::ptr::null(),
                wasm_len: 0,
                errors_json: CString::new("[{\"message\":\"Invalid UTF-8 string\"}]").unwrap().into_raw(),
            };
        }
    };

	let globals: HashMap<String, serde_json::Value> = if globals_ptr.is_null() {
        HashMap::new()
    } else {
        let globals_str = unsafe { CStr::from_ptr(globals_ptr) };

        match globals_str.to_str()
            .ok()
            .and_then(|s| serde_json::from_str(s).ok())
        {
            Some(globals) => globals,
            None => {
                let json = CString::new(
                    r#"[{"message":"Invalid globals JSON"}]"#
                ).unwrap();

                return WasmCompileResult {
                    success: false,
                    wasm_ptr: std::ptr::null(),
                    wasm_len: 0,
                    errors_json: json.into_raw(),
                };
            }
        }
    };

    let mut lex_errors = Vec::new();
    let tokens: Vec<Token> = Token::lexer(code)
        .spanned()
        .filter_map(|(res, span)| match res {
            Ok(token) => Some(token),
            Err(_) => {
                let help = "Remove or replace this character.";
                lex_errors.push(Diagnostic {
                    message: "unknown token".to_string(),
                    start: span.start,
                    end: span.end,
                    help: Some(help.to_string()),
                    rendered: render_diagnostic(
                        code,
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
        let json = serde_json::to_string(&lex_errors).unwrap_or_default();
        return WasmCompileResult {
            success: false,
            wasm_ptr: std::ptr::null(),
            wasm_len: 0,
            errors_json: CString::new(json).unwrap().into_raw(),
        };
    }

    let ast = match parse_code(&tokens, code) {
        Ok(ast) => ast,
        Err(errors) => {
            let diag_errors: Vec<_> = errors
                .into_iter()
                .map(|e| Diagnostic {
                    message: e.message,
                    start: e.start,
                    end: e.end,
                    help: e.help,
                    rendered: e.rendered,
                }).collect();

            let json = serde_json::to_string(&diag_errors).unwrap_or_default();
            return WasmCompileResult {
                success: false,
                wasm_ptr: std::ptr::null(),
                wasm_len: 0,
                errors_json: CString::new(json).unwrap().into_raw(),
            };
        }
    };

    let check_errors = typechecker::check_program(&ast, globals)
        .into_iter()
        .map(|error| Diagnostic {
            message: error.message,
            start: error.start,
            end: error.end,
            help: error.help,
            rendered: error.rendered,
        }).collect::<Vec<_>>();

    if !check_errors.is_empty() {
        let json = serde_json::to_string(&check_errors).unwrap_or_default();
        return WasmCompileResult {
            success: false,
            wasm_ptr: std::ptr::null(),
            wasm_len: 0,
            errors_json: CString::new(json).unwrap().into_raw(),
        };
    }

    let commands = lower_program(&ast);
    let wasm_bytes = WasmCompiler::compile_program(&commands);
    let wasm_len = wasm_bytes.len();
    let wasm_ptr = wasm_bytes.as_ptr();
    std::mem::forget(wasm_bytes); // Keep memory alive for C# to read

    WasmCompileResult {
        success: true,
        wasm_ptr,
        wasm_len,
        errors_json: std::ptr::null(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nosharp_free_wasm(ptr: *mut u8, len: usize) {
    if !ptr.is_null() && len > 0 {
        drop(unsafe { Vec::from_raw_parts(ptr, len, len) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nosharp_free_string(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}

