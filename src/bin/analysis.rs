use syn::{parse_file, visit::Visit, Expr, Member, ImplItemFn};
use syn::spanned::Spanned;
use quote::ToTokens;
use std::fs;

#[derive(Default)]
struct DataFlowAnalyzer {
    definitions: Vec<Definition>,
    uses: Vec<Usage>,
    current_method: Option<String>,
    current_impl: Option<String>,
    target_fields: Vec<String>,
}

#[derive(Debug)]
enum UsagePattern {
    // 定义相关
    Definition(String),         // 结构体字段定义
    
    // 传递相关
    StructInit(String),         // 结构体初始化
    Assignment(String),         // 赋值
    Clone(String),             // 克隆
    
    // 使用相关
    BorrowMutArrayAssign(String), // 可变借用后的数组赋值
    BorrowMutArray(String),    // 可变借用后的数组访问
    BorrowArray(String),       // 不可变借用后的数组访问
    BorrowMut(String),         // 可变借用
    Borrow(String),            // 不可变借用
    ArrayAccess(String),       // 数组访问
    MethodCall(String, String), // 方法调用

    // 函数参数相关
    FunctionArg {
        func_name: String,      
        arg_position: usize,    
        arg_expr: String,       
    },
    MethodArg {
        method_name: String,    
        arg_position: usize,    
        arg_expr: String,       
    },
}

struct Definition {
    name: String,
    location: Location,
    typ: String,
    context: String,
}

struct Usage {
    name: String,
    location: Location,
    impl_block: String,    
    method_name: String,   
    pattern: UsagePattern, 
    context: String,       
}

struct Location {
    file: String,
    line: String,
}

impl DataFlowAnalyzer {
    fn new() -> Self {
        let target_fields = vec![
            "static_ltree".to_string(),
            "static_dtree".to_string(),
            "bltree".to_string(),
            "dyn_ltree".to_string(),
            "dyn_dtree".to_string(),
            "bl_tree".to_string(),
        ];
        DataFlowAnalyzer {
            definitions: Vec::new(),
            uses: Vec::new(),
            current_method: None,
            current_impl: None,
            target_fields,
        }
    }
}

impl<'ast> Visit<'ast> for DataFlowAnalyzer {
    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        let impl_name = if let Some((_, path, _)) = &node.trait_ {
            path.to_token_stream().to_string()
        } else {
            node.self_ty.to_token_stream().to_string()
        };
        self.current_impl = Some(impl_name);
        syn::visit::visit_item_impl(self, node);
        self.current_impl = None;
    }

    fn visit_impl_item_fn(&mut self, node: &'ast ImplItemFn) {
        let prev_method = self.current_method.clone();
        self.current_method = Some(node.sig.ident.to_string());
        syn::visit::visit_impl_item_fn(self, node);
        self.current_method = prev_method;
    }

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        for field in &node.fields {
            if let Some(ident) = &field.ident {
                let field_name = ident.to_string();
                if self.target_fields.contains(&field_name) {
                    self.add_usage(
                        field_name,
                        field.span(),
                        UsagePattern::Definition(field.ty.to_token_stream().to_string()),
                        field.to_token_stream().to_string(),
                    );
                }
            }
        }
    }

    fn visit_expr(&mut self, node: &'ast syn::Expr) {
        match node {
            // 结构体初始化
            Expr::Struct(expr) => {
                for field in &expr.fields {
                    let field_name = field.member.to_token_stream().to_string().replace(' ', "");
                    if self.target_fields.contains(&field_name) {
                        self.add_usage(
                            field_name.clone(),
                            field.span(),
                            UsagePattern::StructInit(field.to_token_stream().to_string()),
                            expr.to_token_stream().to_string(),
                        );
                        self.check_field_in_expr(&field.expr);
                    }
                }
            },

            // 赋值表达式
            Expr::Assign(expr) => {
                if let Expr::Field(field) = &*expr.left {
                    if let Expr::Index(index) = &*field.base {
                        if let Expr::MethodCall(method_call) = &*index.expr {
                            if method_call.method.to_string() == "borrow_mut" {
                                if let Expr::Field(inner_field) = &*method_call.receiver {
                                    let field_name = inner_field.member.to_token_stream().to_string().replace(' ', "");
                                    if self.target_fields.contains(&field_name) {
                                        self.add_usage(
                                            field_name,
                                            expr.span(),
                                            UsagePattern::BorrowMutArrayAssign(expr.to_token_stream().to_string()),
                                            expr.to_token_stream().to_string(),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                self.check_field_in_expr(&expr.right);
            },

            // 函数调用
            Expr::Call(call_expr) => {
                let func_name = match &*call_expr.func {
                    Expr::Path(path) => path.path.segments.last()
                        .map(|seg| seg.ident.to_string())
                        .unwrap_or_else(|| "unknown".to_string()),
                    _ => "unknown".to_string(),
                };

                for (pos, arg) in call_expr.args.iter().enumerate() {
                    self.check_field_in_complex_arg(arg, &func_name, pos);
                }
            },

            // 方法调用
            Expr::MethodCall(method_call) => {
                let method_name = method_call.method.to_string();
                
                if let Expr::Field(field) = &*method_call.receiver {
                    let field_name = field.member.to_token_stream().to_string().replace(' ', "");
                    if self.target_fields.contains(&field_name) {
                        let pattern = match method_name.as_str() {
                            "clone" => UsagePattern::Clone(method_call.to_token_stream().to_string()),
                            "borrow_mut" => UsagePattern::BorrowMut(method_call.to_token_stream().to_string()),
                            "borrow" => UsagePattern::Borrow(method_call.to_token_stream().to_string()),
                            _ => UsagePattern::MethodCall(method_name.clone(), method_call.to_token_stream().to_string()),
                        };
                        self.add_usage(
                            field_name,
                            method_call.span(),
                            pattern,
                            method_call.to_token_stream().to_string(),
                        );
                    }
                }

                for (pos, arg) in method_call.args.iter().enumerate() {
                    self.check_field_in_method_arg(arg, &method_name, pos);
                }
                
                self.check_field_in_expr(&method_call.receiver);
            },

            _ => syn::visit::visit_expr(self, node),
        }
    }
}

impl DataFlowAnalyzer {
    fn check_field_in_expr(&mut self, expr: &syn::Expr) {
        match expr {
            Expr::Field(field) => {
                let field_name = field.member.to_token_stream().to_string().replace(' ', "");
                if self.target_fields.contains(&field_name) {
                    self.add_usage(
                        field_name,
                        field.span(),
                        UsagePattern::MethodCall(
                            "field_access".to_string(),
                            expr.to_token_stream().to_string(),
                        ),
                        expr.to_token_stream().to_string(),
                    );
                }
                self.check_field_in_expr(&field.base);
            },
            Expr::MethodCall(method_call) => {
                if let Expr::Field(field) = &*method_call.receiver {
                    let field_name = field.member.to_token_stream().to_string().replace(' ', "");
                    if self.target_fields.contains(&field_name) {
                        let method_name = method_call.method.to_string();
                        let pattern = match method_name.as_str() {
                            "clone" => UsagePattern::Clone(expr.to_token_stream().to_string()),
                            "borrow_mut" => UsagePattern::BorrowMut(expr.to_token_stream().to_string()),
                            "borrow" => UsagePattern::Borrow(expr.to_token_stream().to_string()),
                            _ => UsagePattern::MethodCall(method_name, expr.to_token_stream().to_string()),
                        };
                        self.add_usage(
                            field_name,
                            method_call.span(),
                            pattern,
                            expr.to_token_stream().to_string(),
                        );
                    }
                }
                // 递归检查
                for arg in &method_call.args {
                    self.check_field_in_expr(arg);
                }
                self.check_field_in_expr(&method_call.receiver);
            },
            _ => syn::visit::visit_expr(self, expr),
        }
    }

    fn check_field_in_complex_arg(&mut self, arg: &syn::Expr, func_name: &str, arg_pos: usize) {
        match arg {
            Expr::MethodCall(method_call) => {
                if let Expr::Field(field) = &*method_call.receiver {
                    let field_name = field.member.to_token_stream().to_string().replace(' ', "");
                    if self.target_fields.contains(&field_name) {
                        self.add_usage(
                            field_name,
                            arg.span(),
                            UsagePattern::FunctionArg {
                                func_name: func_name.to_string(),
                                arg_position: arg_pos,
                                arg_expr: arg.to_token_stream().to_string(),
                            },
                            arg.to_token_stream().to_string(),
                        );
                    }
                }
                // 递归检查
                self.check_field_in_expr(arg);
            },
            Expr::Field(field) => {
                let field_name = field.member.to_token_stream().to_string().replace(' ', "");
                if self.target_fields.contains(&field_name) {
                    self.add_usage(
                        field_name,
                        field.span(),
                        UsagePattern::FunctionArg {
                            func_name: func_name.to_string(),
                            arg_position: arg_pos,
                            arg_expr: arg.to_token_stream().to_string(),
                        },
                        field.to_token_stream().to_string(),
                    );
                }
                self.check_field_in_expr(arg);
            },
            _ => self.check_field_in_expr(arg),
        }
    }

    fn check_field_in_method_arg(&mut self, arg: &syn::Expr, method_name: &str, arg_pos: usize) {
        match arg {
            Expr::MethodCall(method_call) => {
                if let Expr::Field(field) = &*method_call.receiver {
                    let field_name = field.member.to_token_stream().to_string().replace(' ', "");
                    if self.target_fields.contains(&field_name) {
                        self.add_usage(
                            field_name,
                            arg.span(),
                            UsagePattern::MethodArg {
                                method_name: method_name.to_string(),
                                arg_position: arg_pos,
                                arg_expr: arg.to_token_stream().to_string(),
                            },
                            arg.to_token_stream().to_string(),
                        );
                    }
                }
                // 递归检查
                self.check_field_in_expr(arg);
            },
            _ => self.check_field_in_expr(arg),
        }
    }

    fn add_usage(&mut self, name: String, span: proc_macro2::Span, pattern: UsagePattern, context: String) {
        self.uses.push(Usage {
            name,
            location: Location {
                file: "trees_back.rs".to_string(),
                line: format!("{:?}", span),
            },
            impl_block: self.current_impl.clone().unwrap_or_else(|| "unknown".to_string()),
            method_name: self.current_method.clone().unwrap_or_else(|| "unknown".to_string()),
            pattern,
            context,
        });
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string("tree_analysis/trees_back.rs")?;
    let ast = parse_file(&content)?;

    let mut analyzer = DataFlowAnalyzer::new();
    syn::visit::visit_file(&mut analyzer, &ast);

    // 为每个目标字段分别打印分析结果
    for target_field in &analyzer.target_fields {
        println!("\nData Flow Analysis for {}:\n", target_field);
        
        println!("1. Definitions:");
        for usage in &analyzer.uses {
            if usage.name == *target_field {
                if let UsagePattern::Definition(_) = &usage.pattern {
                    println!("  - {}", usage.context);
                    println!("    at {}:{}", usage.location.file, usage.location.line);
                }
            }
        }

        println!("\n2. Value Transfers:");
        for usage in &analyzer.uses {
            if usage.name == *target_field {
                match &usage.pattern {
                    UsagePattern::StructInit(_) | UsagePattern::Assignment(_) | UsagePattern::Clone(_) => {
                        println!("  - In {}::{}", usage.impl_block, usage.method_name);
                        println!("    Pattern: {:?}", usage.pattern);
                        println!("    at {}:{}", usage.location.file, usage.location.line);
                        println!("    Context: {}", usage.context);
                    }
                    _ => {}
                }
            }
        }

        println!("\n3. Complex Operations:");
        for usage in &analyzer.uses {
            if usage.name == *target_field {
                match &usage.pattern {
                    UsagePattern::BorrowMutArrayAssign(_) | 
                    UsagePattern::BorrowMutArray(_) |
                    UsagePattern::BorrowArray(_) => {
                        println!("  - In {}::{}", usage.impl_block, usage.method_name);
                        println!("    Pattern: {:?}", usage.pattern);
                        println!("    at {}:{}", usage.location.file, usage.location.line);
                        println!("    Context: {}", usage.context);
                    }
                    _ => {}
                }
            }
        }

        println!("\n4. Simple Operations:");
        for usage in &analyzer.uses {
            if usage.name == *target_field {
                match &usage.pattern {
                    UsagePattern::BorrowMut(_) | UsagePattern::Borrow(_) | 
                    UsagePattern::ArrayAccess(_) | UsagePattern::MethodCall(_, _) => {
                        println!("  - In {}::{}", usage.impl_block, usage.method_name);
                        println!("    Pattern: {:?}", usage.pattern);
                        println!("    at {}:{}", usage.location.file, usage.location.line);
                        println!("    Context: {}", usage.context);
                    }
                    _ => {}
                }
            }
        }

        println!("\n5. Function Arguments:");
        for usage in &analyzer.uses {
            if usage.name == *target_field {
                match &usage.pattern {
                    UsagePattern::FunctionArg { func_name, arg_position, arg_expr } => {
                        println!("  - Function: {}", func_name);
                        println!("    Argument Position: {}", arg_position);
                        println!("    Expression: {}", arg_expr);
                        println!("    at {}:{}", usage.location.file, usage.location.line);
                        println!("    In {}::{}", usage.impl_block, usage.method_name);
                    },
                    UsagePattern::MethodArg { method_name, arg_position, arg_expr } => {
                        println!("  - Method: {}", method_name);
                        println!("    Argument Position: {}", arg_position);
                        println!("    Expression: {}", arg_expr);
                        println!("    at {}:{}", usage.location.file, usage.location.line);
                        println!("    In {}::{}", usage.impl_block, usage.method_name);
                    },
                    _ => {}
                }
            }
        }

        // 打印该字段的使用统计
        let field_uses = analyzer.uses.iter()
            .filter(|usage| usage.name == *target_field)
            .count();
        println!("\nTotal usages found for {}: {}", target_field, field_uses);
    }

    // 打印总计数
    let total_uses = analyzer.uses.len();
    println!("\nTotal usages found across all fields: {}", total_uses);

    Ok(())
}