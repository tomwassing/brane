use std::collections::HashMap;

use crate::parser::ast::*;
use anyhow::Result;
use brane_bvm::bytecode::{Opcode, ChunkMut, FunctionMut};
use specifications::common::{SpecClass, SpecFunction, Value};

#[derive(Debug, Clone)]
pub struct Local {
    pub name: String,
    pub depth: i32,
}

///
///
///
pub fn compile(program: Program) -> Result<FunctionMut> {
    let mut chunk = ChunkMut::default();
    let mut locals = Vec::new();

    for stmt in program {
        stmt_to_opcodes(stmt, &mut chunk, &mut locals, 0);
    }

    Ok(FunctionMut::main(chunk))
}

///
///
///
pub fn compile_function(
    block: Block,
    scope: i32,
    params: &[Ident],
    name: String,
) -> Result<FunctionMut> {
    let mut locals = Vec::new();
    let mut chunk = ChunkMut::default();

    let local = Local {
        name: String::from("func"),
        depth: scope,
    };
    locals.push(local);

    for Ident(param) in params {
        let local = Local {
            name: param.clone(),
            depth: scope,
        };
        locals.push(local);
    }

    for stmt in block {
        stmt_to_opcodes(stmt, &mut chunk, &mut locals, scope);
    }
    chunk.write_pair(Opcode::UNIT, Opcode::RETURN);

    let function = FunctionMut::new(name, params.len() as u8, chunk);
    Ok(function)
}

///
///
///
pub fn stmt_to_opcodes(
    stmt: Stmt,
    chunk: &mut ChunkMut,
    locals: &mut Vec<Local>,
    scope: i32,
) {
    match stmt {
        Stmt::Import {
            package: Ident(ident), ..
        } => {
            let import = chunk.add_constant(ident.into());
            chunk.write_pair(Opcode::IMPORT, import);
        }
        Stmt::DeclareClass {
            ident: Ident(ident),
            properties,
            methods,
        } => {
            let properties = properties.into_iter().map(|(Ident(k), Ident(v))| (k, v)).collect();
            let methods: HashMap<String, SpecFunction> = methods
                .into_iter()
                .map(|(Ident(k), stmt)| {
                    if let Stmt::DeclareFunc {
                        ident: Ident(ident),
                        params,
                        body,
                    } = stmt
                    {
                        let method: FunctionMut = compile_function(body, 1, &params, ident).unwrap();
                        let method: SpecFunction = method.into();

                        (k, method)
                    } else {
                        unreachable!()
                    }
                })
                .collect();

            let class = Value::Class(SpecClass::new(ident.clone(), properties, methods));

            let class = chunk.add_constant(class);
            chunk.write_pair(Opcode::CLASS, class);

            let ident = chunk.add_constant(ident.into());
            chunk.write_pair(Opcode::DEFINE_GLOBAL, ident);
        }
        Stmt::Assign(Ident(ident), expr) => {
            // ident must be an existing local or global.
            expr_to_opcodes(expr, chunk, locals, scope);

            if let Some(index) = locals.iter().position(|l| l.name == ident) {
                chunk.write_pair(Opcode::SET_LOCAL, index as u8);
            } else {
                let ident = chunk.add_constant(ident.into());
                chunk.write_pair(Opcode::SET_GLOBAL, ident);
            }
        }
        Stmt::LetAssign(Ident(ident), expr) => {
            expr_to_opcodes(expr, chunk, locals, scope);

            // Don't put a local's name in the globals table.
            // Instead, just note that there's a local on the stack.
            if scope > 0 {
                let local = Local {
                    name: ident,
                    depth: scope,
                };
                locals.push(local);
                return;
            }

            let ident = chunk.add_constant(ident.into());
            chunk.write_pair(Opcode::DEFINE_GLOBAL, ident);
        }
        Stmt::Block(block) => {
            // Create a new scope (shadow).
            let scope = scope + 1;

            for stmt in block {
                stmt_to_opcodes(stmt, chunk, locals, scope);
            }

            // Remove any locals created in this scope.
            let mut n = 0;
            while let Some(local) = locals.pop() {
                if local.depth >= scope {
                    n += 1;
                } else {
                    // Oops, one to many, place it back.
                    locals.push(local);
                    break;
                }
            }

            match n {
                0 => {}
                1 => chunk.write(Opcode::POP),
                n => chunk.write_pair(Opcode::POP_N, n),
            }
        }
        Stmt::For {
            initializer,
            condition,
            increment,
            consequent,
        } => {
            let scope = scope + 1;

            stmt_to_opcodes(*initializer, chunk, locals, scope);

            let loop_start = chunk.code.len();

            expr_to_opcodes(condition, chunk, locals, scope);
            // Now the result of the condition is on the stack.

            chunk.write(Opcode::JUMP_IF_FALSE);
            // Placeholders, we'll backpatch this later.
            let plh_pos = chunk.code.len();
            chunk.write_pair(0x00, 0x00);

            chunk.write(Opcode::POP);
            for stmt in consequent {
                stmt_to_opcodes(stmt, chunk, locals, scope);
            }

            // Run incrementer statement
            stmt_to_opcodes(*increment, chunk, locals, scope);

            // Emit loop
            chunk.write(Opcode::JUMP_BACK);
            let jump_back = (chunk.code.len() - loop_start + 2) as u16;
            chunk.write_bytes(&jump_back.to_be_bytes()[..]);

            // How much to jump if condition is false (exit).
            let jump = (chunk.code.len() - plh_pos - 2) as u16;
            let [first, second, ..] = jump.to_be_bytes();
            chunk.code[plh_pos] = first;
            chunk.code[plh_pos + 1] = second;

            chunk.write(Opcode::POP);
        }
        Stmt::While { condition, consequent } => {
            let loop_start = chunk.code.len();

            expr_to_opcodes(condition, chunk, locals, scope);
            // Now the result of the condition is on the stack.

            chunk.write(Opcode::JUMP_IF_FALSE);
            // Placeholders, we'll backpatch this later.
            let plh_pos = chunk.code.len();
            chunk.write_pair(0x00, 0x00);

            chunk.write(Opcode::POP);
            stmt_to_opcodes(Stmt::Block(consequent), chunk, locals, scope);

            // Emit loop
            chunk.write(Opcode::JUMP_BACK);
            let jump_back = (chunk.code.len() - loop_start + 2) as u16;
            chunk.write_bytes(&jump_back.to_be_bytes()[..]);

            // How much to jump?
            let jump = (chunk.code.len() - plh_pos - 2) as u16;
            let [first, second, ..] = jump.to_be_bytes();
            chunk.code[plh_pos] = first;
            chunk.code[plh_pos + 1] = second;

            chunk.write(Opcode::POP);
        }
        Stmt::If {
            condition,
            consequent,
            alternative,
        } => {
            expr_to_opcodes(condition, chunk, locals, scope);
            // Now the result of the condition is on the stack.

            chunk.write(Opcode::JUMP_IF_FALSE);
            // Placeholders, we'll backpatch this later.
            let plh_pos = chunk.code.len();
            chunk.write_pair(0x00, 0x00);

            chunk.write(Opcode::POP);
            stmt_to_opcodes(Stmt::Block(consequent), chunk, locals, scope);

            // For the else branch
            chunk.write(Opcode::JUMP);
            // Placeholders, we'll backpatch this later.
            let else_jump_pos = chunk.code.len();
            chunk.write_pair(0x00, 0x00);

            // How much to jump?
            let jump = (chunk.code.len() - plh_pos - 2) as u16;
            let [first, second, ..] = jump.to_be_bytes();
            chunk.code[plh_pos] = first;
            chunk.code[plh_pos + 1] = second;

            chunk.write(Opcode::POP);

            if let Some(alternative) = alternative {
                stmt_to_opcodes(Stmt::Block(alternative), chunk, locals, scope);
            }

            let jump = (chunk.code.len() - else_jump_pos - 2) as u16;
            let [first, second, ..] = jump.to_be_bytes();
            chunk.code[else_jump_pos] = first;
            chunk.code[else_jump_pos + 1] = second;
        }
        Stmt::Expr(expr) => {
            expr_to_opcodes(expr, chunk, locals, scope);
            chunk.write(Opcode::POP);
        }
        Stmt::Property { .. } => {
            unreachable!()
        }
        Stmt::Return(expr) => {
            if let Some(expr) = expr {
                expr_to_opcodes(expr, chunk, locals, scope);
            } else {
                chunk.write(Opcode::UNIT)
            }

            chunk.write(Opcode::RETURN);
        }
        Stmt::DeclareFunc {
            ident: Ident(ident),
            params,
            body,
        } => {
            let function: FunctionMut = compile_function(body, scope + 1, &params, ident.clone()).unwrap();
            let function: SpecFunction = function.into();

            let function = chunk.add_constant(function.into());
            chunk.write_pair(Opcode::CONSTANT, function);

            let ident = chunk.add_constant(ident.into());
            chunk.write_pair(Opcode::DEFINE_GLOBAL, ident);
        }

        // TODO: merge with block statement?
        Stmt::On { location, block } => {
            // Create a new scope (shadow).
            // let scope = scope + 1;

            expr_to_opcodes(location, chunk, locals, scope);
            chunk.write(Opcode::LOC_PUSH);

            for stmt in block {
                stmt_to_opcodes(stmt, chunk, locals, scope);
            }

            // Remove any locals created in this scope.
            let mut n = 0;
            while let Some(local) = locals.pop() {
                if local.depth >= scope {
                    n += 1;
                } else {
                    // Oops, one to many, place it back.
                    locals.push(local);
                    break;
                }
            }

            match n {
                0 => {}
                1 => chunk.write(Opcode::POP),
                n => chunk.write_pair(Opcode::POP_N, n),
            }

            chunk.write(Opcode::LOC_POP);
        }
        Stmt::Parallel { let_assign, blocks } => {
            let block_n = blocks.len() as u8;
            for block in blocks.into_iter().rev() {
                let function = compile_function(vec![block], scope, &[], String::new()).unwrap();
                let function: SpecFunction = function.into();

                let function = chunk.add_constant(function.into());

                chunk.write_pair(Opcode::CONSTANT, function);
            }

            chunk.write_pair(Opcode::PARALLEL, block_n);

            if let Some(Ident(ident)) = let_assign {
                // Don't put a local's name in the globals table.
                // Instead, just note that there's a local on the stack.
                if scope > 0 {
                    let local = Local {
                        name: ident,
                        depth: scope,
                    };
                    locals.push(local);
                    return;
                }

                let ident = chunk.add_constant(ident.into());
                chunk.write_pair(Opcode::DEFINE_GLOBAL, ident);
            } else {
                chunk.write(Opcode::POP);
            }
        }
    }
}

///
///
///
pub fn expr_to_opcodes(
    expr: Expr,
    chunk: &mut ChunkMut,
    locals: &mut Vec<Local>,
    scope: i32,
) {
    match expr {
        Expr::Binary {
            operator,
            lhs_operand,
            rhs_operand,
        } => {
            // Always evaluate LHS
            expr_to_opcodes(*lhs_operand, chunk, locals, scope);
            let rhs_operand = *rhs_operand;

            if let BinOp::Dot = operator {
                match &rhs_operand {
                    Expr::Ident(Ident(ident)) => {
                        let property = chunk.add_constant(ident.clone().into());
                        chunk.write_pair(Opcode::GET_PROPERTY, property);
                        return;
                    }
                    Expr::Call {
                        function: Ident(ident),
                        arguments,
                    } => {
                        // Put method on the stack.
                        let method = chunk.add_constant(ident.clone().into());
                        chunk.write_pair(Opcode::GET_METHOD, method);

                        // Call method with arguments, implicitly pass self.
                        let arguments_n = arguments.len() as u8 + 1;
                        for argument in arguments.iter().skip(1) {
                            expr_to_opcodes(argument.clone(), chunk, locals, scope);
                        }

                        chunk.write_pair(Opcode::CALL, arguments_n);

                        return;
                    }
                    _ => unreachable!(),
                }
            }

            expr_to_opcodes(rhs_operand, chunk, locals, scope);
            match operator {
                // Arithmetic
                BinOp::Add => chunk.write(Opcode::ADD),
                BinOp::Sub => chunk.write(Opcode::SUBSTRACT),
                BinOp::Mul => chunk.write(Opcode::MULTIPLY),
                BinOp::Div => chunk.write(Opcode::DIVIDE),
                // Equality / Comparison
                BinOp::Eq => chunk.write(Opcode::EQUAL),
                BinOp::Lt => chunk.write(Opcode::LESS),
                BinOp::Gt => chunk.write(Opcode::GREATER),
                BinOp::Le => {
                    // !(lhs > rhs)
                    chunk.write(Opcode::GREATER);
                    chunk.write(Opcode::NOT);
                }
                BinOp::Ge => {
                    // !(lhs < rhs)
                    chunk.write(Opcode::LESS);
                    chunk.write(Opcode::NOT);
                }
                BinOp::Ne => {
                    // !(lhs == rhs)
                    chunk.write(Opcode::EQUAL);
                    chunk.write(Opcode::NOT);
                }

                // Logical
                BinOp::And => chunk.write(Opcode::AND),
                BinOp::Or => chunk.write(Opcode::OR),

                _ => unreachable!(),
            }
        }
        Expr::Unary { operator, operand } => {
            expr_to_opcodes(*operand, chunk, locals, scope);
            match operator {
                UnOp::Neg => chunk.write(Opcode::NEGATE),
                UnOp::Not => chunk.write(Opcode::NOT),
                _ => unreachable!(),
            }
        }
        Expr::Literal(literal) => {
            match literal {
                Lit::Boolean(boolean) => match boolean {
                    true => chunk.write(Opcode::TRUE),
                    false => chunk.write(Opcode::FALSE),
                },
                Lit::Integer(integer) => {
                    let constant = chunk.add_constant(integer.into());
                    chunk.write_pair(Opcode::CONSTANT, constant);
                }
                Lit::Real(real) => {
                    let constant = chunk.add_constant(real.into());
                    chunk.write_pair(Opcode::CONSTANT, constant);
                }
                Lit::String(string) => {
                    let constant = chunk.add_constant(string.into());
                    chunk.write_pair(Opcode::CONSTANT, constant);
                }
                Lit::Unit => {
                    chunk.write(Opcode::UNIT);
                }
            };
        }
        Expr::Ident(Ident(ident)) => {
            if let Some(index) = locals.iter().position(|l| l.name == ident) {
                chunk.write_pair(Opcode::GET_LOCAL, index as u8);
            } else {
                let ident = chunk.add_constant(ident.into());
                chunk.write_pair(Opcode::GET_GLOBAL, ident);
            }
        }
        Expr::Call { function, arguments } => {
            expr_to_opcodes(Expr::Ident(function), chunk, locals, scope);

            let arguments_n = arguments.len() as u8;
            for argument in arguments {
                expr_to_opcodes(argument, chunk, locals, scope);
            }

            chunk.write_pair(Opcode::CALL, arguments_n);
        }
        Expr::Instance { class, properties } => {
            let properties_n = properties.len() as u8;
            for property in properties {
                if let Stmt::Assign(Ident(name), value) = property {
                    expr_to_opcodes(value, chunk, locals, scope);
                    expr_to_opcodes(Expr::Literal(Lit::String(name)), chunk, locals, scope);
                } else {
                    unreachable!();
                }
            }

            expr_to_opcodes(Expr::Ident(class), chunk, locals, scope);
            chunk.write_pair(Opcode::NEW, properties_n);
        }
        Expr::Array(entries) => {
            let entries_n = entries.len() as u8;
            for entry in entries.iter().rev() {
                expr_to_opcodes(entry.clone(), chunk, locals, scope);
            }

            chunk.write_pair(Opcode::ARRAY, entries_n);
        }
        Expr::Index { array, index } => {
            expr_to_opcodes(*array, chunk, locals, scope);
            expr_to_opcodes(*index, chunk, locals, scope);

            chunk.write(Opcode::INDEX);
        }
        Expr::Pattern(_) => {
            // Converted into one or more `Expr::Call` expressions.
            unreachable!()
        }
    }
}
