use syn::{parse_file, visit::Visit};
use syn::__private::ToTokens;  // 添加这行
use std::fs;

#[derive(Default)]
struct StructAnalyzer {
    fields: Vec<(String, String)>,
    methods: Vec<String>,
}

impl<'ast> Visit<'ast> for StructAnalyzer {
    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        for field in &node.fields {
            if let Some(ident) = &field.ident {
                self.fields.push((
                    ident.to_string(),
                    field.ty.to_token_stream().to_string(),  // 现在可以使用 to_token_stream
                ));
            }
        }
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        for item in &node.items {
            if let syn::ImplItem::Fn(method) = item {
                self.methods.push(method.sig.ident.to_string());
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 读取源文件
    let content = fs::read_to_string("tree_analysis/trees_back.rs")?;
    let ast = parse_file(&content)?;

    // 创建分析器
    let mut analyzer = StructAnalyzer::default();
    syn::visit::visit_file(&mut analyzer, &ast);

    // 输出结果
    println!("Trees Structure Analysis:\n");
    
    println!("Fields:");
    for (name, type_) in &analyzer.fields {
        println!("- {}: {}", name, type_);
    }

    println!("\nMethods:");
    for method in &analyzer.methods {
        println!("- {}", method);
    }

    Ok(())
}