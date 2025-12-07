use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{ItemMod, parse_macro_input};

use tusks_lib::TusksNode;

#[proc_macro_attribute]
pub fn tusks(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut module = parse_macro_input!(item as ItemMod);
    
    let tusks_tree = match TusksNode::from_module(&module) {
        Ok(tree) => tree,
        Err(err) => return err.to_compile_error().into(),
    };
    
    // Remove all #[defaults(...)] attributes after parsing
    if let Some((_, items)) = &mut module.content {
        cleanup_defaults_attributes(items);
        
        // Add the internal module to the module's items
        let internal_module = create_internal_tusks_module(&tusks_tree);
        let internal_item: syn::Item = syn::parse2(internal_module).expect("Failed to parse internal module");
        items.push(internal_item);
    }
    
    let expanded = quote! {
        #module
    };
    
    TokenStream::from(expanded)
}

fn create_internal_tusks_module(tusks_tree: &TusksNode) -> TokenStream2 {
    let tree_code = tusks_tree.to_tokens(&[]); // Start mit leerem Pfad
    let mirror_code = tusks_tree.create_mirror(&[]);
    
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
