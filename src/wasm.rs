use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, DataSection, EntityType,
    ExportKind, ExportSection, Function, FunctionSection,
    ImportSection, Instruction, MemorySection, MemoryType,
    Module, TypeSection, ValType,
};

use std::collections::HashMap;

use crate::commands::{Command, Value, WasmExpr};

pub struct WasmCompiler {
    pub locals: HashMap<String, u32>,
    pub local_idx: u32,
    pub functions: HashMap<String, u32>,
    pub data_segments: Vec<(u32, Vec<u8>)>,
    pub current_memory_offset: u32,
}

impl WasmCompiler {
    fn alloc_string_literal(&mut self, text: &str) -> (u32, u32) {
        let mut bytes = text.as_bytes().to_vec();
        bytes.push(0);

        let len = bytes.len() as u32;
        let offset = self.current_memory_offset;

        self.data_segments.push((offset, bytes));
        self.current_memory_offset += len;

        (offset, len)
    }

    pub fn compile_program(commands: &[Command]) -> Vec<u8> {
        let mut module = Module::new();
        let mut types = TypeSection::new();
		
        types.ty().function(vec![], vec![]); // 0: main
        types.ty().function([ValType::I32], []); // 1: print_i32
        types.ty().function([ValType::I32, ValType::I32], []); // 2: print_str

        // Imports
        let mut imports = ImportSection::new();

        imports.import(
            "std",
            "print_i32",
            EntityType::Function(1),
        );

        imports.import(
            "std",
            "print_str",
            EntityType::Function(2),
        );

        // Main function
        let mut functions = FunctionSection::new();
        functions.function(0);

        let mut exports = ExportSection::new();
        exports.export("main", ExportKind::Func, 2);

        let mut code = CodeSection::new();
        let mut main_func = Function::new([(100, ValType::I32), (100, ValType::F32)]);

        let mut compiler = WasmCompiler {
            locals: HashMap::new(),
            local_idx: 0,
            functions: HashMap::new(),
            data_segments: Vec::new(),
            current_memory_offset: 1024,
        };

        compiler.functions.insert("print_i32".to_string(), 0);
        compiler.functions.insert("print_str".to_string(), 1);

        let mut func_defs = Vec::new();
        let mut main_statements = Vec::new();

        for cmd in commands {
            match cmd {
                Command::DefineFunction { .. } => {
                    func_defs.push(cmd);
                }

                _ => main_statements.push(cmd),
            }
        }

        // Register user function indices
        // 0 = print_i32
        // 1 = print_str
        // 2 = main
        // 3+ = user functions

        for (i, def) in func_defs.iter().enumerate() {
            if let Command::DefineFunction { name, .. } = def {
                compiler.functions.insert(
                    name.clone(),
                    (i + 3) as u32,
                );
            }
        }

        // Compile main
        compiler.locals.clear();
        compiler.local_idx = 0;

        for cmd in main_statements {
            compiler.compile_command(cmd, &mut main_func);
        }

        main_func.instruction(&Instruction::End);
        code.function(&main_func);

        // Compile user functions
        for def in func_defs {
            if let Command::DefineFunction { return_type, params, body, ..} = def {
                compiler.locals.clear();
                compiler.local_idx = 0;

                let mut param_types = Vec::new();

                for (ty_str, param_name) in params {
                    let val_ty = match ty_str.as_str() {
                        "float" => ValType::F32,
                        _ => ValType::I32,
                    };

                    param_types.push(val_ty);
                    compiler.locals.insert(param_name.clone(), compiler.local_idx);
                    compiler.local_idx += 1;
                }

                let ret_types = match return_type.as_str() {
                    "void" => vec![],
                    "float" => vec![ValType::F32],
                    _ => vec![ValType::I32],
                };

                let ty_idx = types.len();

                types.ty().function(param_types, ret_types);
                functions.function(ty_idx);

                let mut user_func =
                    Function::new([(100, ValType::I32), (100, ValType::F32)]);

                for stmt in body {
                    compiler.compile_command(stmt, &mut user_func);
                }

                user_func.instruction(&Instruction::End);
                code.function(&user_func);
            }
        }

        // Memory
        let mut memories = MemorySection::new();

        memories.memory(MemoryType {
            minimum: 1,
            maximum: None,
            memory64: false,
            shared: false,
            page_size_log2: None,
        });

        let mut data = DataSection::new();
        for (offset, bytes) in &compiler.data_segments {
            data.active(
                0,
                &ConstExpr::i32_const(*offset as i32),
                bytes.iter().copied(),
            );
        }

        // Assemble module
        module.section(&types);
        module.section(&imports);
        module.section(&functions);
        module.section(&memories);
        module.section(&exports);
        module.section(&code);
        module.section(&data);

        module.finish()
    }

    fn compile_command(&mut self, cmd: &Command, func: &mut Function) {
        match cmd {
            Command::AssignVar { name, value } => {
                self.compile_expr(value, func);
                let idx = *self.locals.get(name).expect("variable not found");

                func.instruction(&Instruction::LocalSet(idx));
            }

            Command::SetVar { name, value, .. } => {
                self.compile_expr(value, func);
                func.instruction(&Instruction::LocalSet(self.local_idx));

                self.locals.insert(name.clone(), self.local_idx);
                self.local_idx += 1;
            }

            Command::ExprStatement { value } => {
                let val_type = self.compile_expr(value, func);
                if val_type.is_some() {
                    func.instruction(&Instruction::Drop);
                }
            }

            Command::CallFunction { name, args } => {
                for arg in args {
                    self.compile_expr(arg, func);
                }

                let func_idx = *self.functions.get(name).expect("unknown function");
                func.instruction(&Instruction::Call(func_idx));
            }

            Command::If { condition, then_block, else_block } => {
                self.compile_expr(condition, func);
                func.instruction(&Instruction::If(BlockType::Empty));

                for cmd in then_block {
                    self.compile_command(cmd, func);
                }

                if let Some(else_cmds) = else_block {
                    func.instruction(&Instruction::Else);
                    for cmd in else_cmds {
                        self.compile_command(cmd, func);
                    }
                }

                func.instruction(&Instruction::End);
            }

            Command::Return { value } => {
                if let Some(expr) = value {
                    self.compile_expr(expr, func);
                }

                func.instruction(&Instruction::Return);
            }

            Command::For {var_type, name, start, end, step, body} => {
                match var_type.as_str() {
                    "int" => {
                        self.compile_expr(start, func);

                        let loop_var_idx = self.local_idx;
                        func.instruction(&Instruction::LocalSet(loop_var_idx));

                        self.locals.insert(name.clone(), loop_var_idx);
                        self.local_idx += 1;

                        func.instruction(&Instruction::Loop(BlockType::Empty));

                        func.instruction(&Instruction::LocalGet(loop_var_idx));
                        self.compile_expr(end, func);

                        func.instruction(&Instruction::I32LtS);
                        func.instruction(&Instruction::If(BlockType::Empty));

                        for cmd in body {
                            self.compile_command(cmd, func);
                        }

                        func.instruction(&Instruction::LocalGet(loop_var_idx));
                        self.compile_expr(step, func);

                        func.instruction(&Instruction::I32Add);
                        func.instruction(&Instruction::LocalSet(loop_var_idx));

                        func.instruction(&Instruction::Br(1));

                        func.instruction(&Instruction::End);
                        func.instruction(&Instruction::End);
                    }

                    _ => panic!("unsupported loop variable type"),
                }
            }

            Command::DefineFunction { .. } => {
                // Handled in compile_program.
            }
        }
    }

    fn compile_expr(&mut self, expr: &WasmExpr, func: &mut Function,) -> Option<ValType> {
        match expr {
            WasmExpr::Variable { name } => {
                let idx = *self.locals.get(name).expect("unknown variable");
                func.instruction(&Instruction::LocalGet(idx));

                Some(ValType::I32)
            }

            WasmExpr::BinaryOp { op, left, right } => {
                let left_type = self.compile_expr(left, func);
                let right_type = self.compile_expr(right, func);

                let op_str = op.as_str();

                if left_type != right_type {
                    panic!("binary operation type mismatch");
                }

                match (left_type, op_str) {
                    (Some(ValType::I32), "+") => {
                        func.instruction(&Instruction::I32Add);
                    }

                    (Some(ValType::I32), "-") => {
                        func.instruction(&Instruction::I32Sub);
                    }

                    (Some(ValType::I32), "*") => {
                        func.instruction(&Instruction::I32Mul);
                    }

                    (Some(ValType::I32), "/") => {
                        func.instruction(&Instruction::I32DivS);
                    }

                    (Some(ValType::F32), "+") => {
                        func.instruction(&Instruction::F32Add);
                    }

                    (Some(ValType::F32), "-") => {
                        func.instruction(&Instruction::F32Sub);
                    }

                    (Some(ValType::F32), "*") => {
                        func.instruction(&Instruction::F32Mul);
                    }

                    (Some(ValType::F32), "/") => {
                        func.instruction(&Instruction::F32Div);
                    }

                    (t, op) => {
                        panic!("unsupported binary op `{op}` for type `{t:?}`");
                    }
                }

                left_type
            }

            WasmExpr::UnaryOp { op, operand } => {
                let operand_type = self.compile_expr(operand, func);
                let op_str = op.as_str();

                match (operand_type, op_str) {
                    (Some(ValType::F32), "-") => {
                        func.instruction(&Instruction::F32Neg);
                        operand_type
                    }

                    (Some(ValType::I32), "-") => {
                        func.instruction(&Instruction::I32Const(-1));
                        func.instruction(&Instruction::I32Mul);
                        operand_type
                    }

                    (Some(ValType::I32), "not") => {
                        func.instruction(&Instruction::I32Eqz);
                        operand_type
                    }

                    _ => panic!("unsupported unary op"),
                }
            }

            WasmExpr::Literal { value } => {
                match value {
                    Value::Int(n) => {
                        func.instruction(&Instruction::I32Const(*n));
                        Some(ValType::I32)
                    }

                    Value::Float(n) => {
                        func.instruction(&Instruction::F32Const((*n).into()),);
                        Some(ValType::F32)
                    }

                    Value::Bool(b) => {
                        func.instruction(&Instruction::I32Const(if *b { 1 } else { 0 }),);
                        Some(ValType::I32)
                    }

                    Value::String(s) => {
                        let (offset, _) = self.alloc_string_literal(s);
                        func.instruction(
                            &Instruction::I32Const(offset as i32),
                        );

                        Some(ValType::I32)
                    }

                    Value::Null => {
                        func.instruction(&Instruction::I32Const(0));
                        Some(ValType::I32)
                    }
                }
            }

            WasmExpr::Call { name, args } => {
                for arg in args {
                    self.compile_expr(arg, func);
                }

                let func_idx = *self.functions.get(name).unwrap_or_else(|| panic!("unknown function '{name}'"));

                func.instruction(&Instruction::Call(func_idx));

                if name == "print_i32" || name == "print_str" {
                    None
                } else {
                    Some(ValType::I32)
                }
            }
        }
    }
}