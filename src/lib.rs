use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{ItemMod, parse_macro_input};
use tusks_lib::TusksNode;

#[proc_macro_attribute]
pub fn tusks(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut module = parse_macro_input!(item as ItemMod);
    
    // Parse the path_separator attribute, default to "."
    let path_separator = parse_path_separator(attr);
    
    let tusks_tree = match TusksNode::from_module(&module, Vec::new()) {
        Ok(tree) => tree,
        Err(err) => return err.to_compile_error().into(),
    };
    
    // Remove all #[defaults(...)] attributes after parsing
    if let Some((_, items)) = &mut module.content {
        cleanup_defaults_attributes(items);
        
        // Add the internal module to the module's items
        let internal_module = create_internal_tusks_module(&tusks_tree, &path_separator);
        let internal_item: syn::Item = syn::parse2(internal_module).expect("Failed to parse internal module");
        items.push(internal_item);
    }
    
    let expanded = quote! {
        #module
    };
    
    TokenStream::from(expanded)
}

fn parse_path_separator(attr: TokenStream) -> String {
    if attr.is_empty() {
        return ".".to_string();
    }
    
    // Parse as TokenStream2 and try to extract string
    let tokens: TokenStream2 = attr.into();
    let tokens_str = tokens.to_string();
    
    // Expected format: path_separator = "."
    if let Some(start) = tokens_str.find('"') {
        if let Some(end) = tokens_str[start + 1..].find('"') {
            return tokens_str[start + 1..start + 1 + end].to_string();
        }
    }
    
    ".".to_string()
}

fn create_internal_tusks_module(tusks_tree: &TusksNode, path_separator: &str) -> TokenStream2 {
    let tree_code = tusks_tree.to_tokens(&[]);
    let mirror_code = tusks_tree.create_mirror(&[]);
    let cli_build_code = tusks_tree.build_cli("command", "path_prefix", path_separator);
    
    quote! {
        pub mod __tusks_internal_module {
            use tusks::{TusksNode, Tusk, Argument, LinkNode};
            use std::collections::HashMap;
            
            pub fn get_tusks_tree() -> TusksNode {
                #tree_code
            }
            
            pub mod mirror_module {
                #mirror_code
            }
            
            pub fn execute_cli() {
                let mut command = clap::Command::new("tusks")
                    .version("1.0")
                    .about("Task runner");
                
                command = build_cli(command, Vec::new());
                
                // TODO: Execute the CLI and handle matches
                let _matches = command.get_matches();
            }
            
            pub fn build_cli(mut command: clap::Command, path_prefix: Vec<String>) -> clap::Command {
                #cli_build_code
                command
            }
        }
    }
}

/// Recursively removes all #[defaults(...)] attributes from the module
fn cleanup_defaults_attributes(items: &mut Vec<syn::Item>) {
    for item in items.iter_mut() {
        match item {
            syn::Item::Fn(func) => {
                // Remove defaults attributes from functions
                remove_defaults_attrs(&mut func.attrs);
            }
            syn::Item::Mod(submodule) => {
                // Recurse into submodules
                if let Some((_, subitems)) = &mut submodule.content {
                    cleanup_defaults_attributes(subitems);
                }
            }
            _ => {}
        }
    }
}

/// Removes all #[defaults(...)] attributes from an attribute list
fn remove_defaults_attrs(attrs: &mut Vec<syn::Attribute>) {
    attrs.retain(|attr| {
        // Keep only attributes that are NOT "defaults"
        !attr.path().is_ident("defaults")
    });
}
