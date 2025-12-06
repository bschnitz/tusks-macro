use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemMod, parse_macro_input};

use tusks_lib::TusksNode;

#[proc_macro_attribute]
pub fn tusks(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse das Modul
    let mut module = parse_macro_input!(item as ItemMod);
    
    // Erstelle den TusksNode-Baum (vor dem Cleanup)
    let tusks_tree = match TusksNode::from_module(&module) {
        Ok(tree) => tree,
        Err(err) => return err.to_compile_error().into(),
    };
    
    // Debug-Ausgabe zur Compile-Zeit
    eprintln!("Tusks Tree: {:#?}", tusks_tree);
    
    // Entferne alle #[defaults(...)] Attribute nach dem Parsen
    if let Some((_, items)) = &mut module.content {
        cleanup_defaults_attributes(items);
    }
    
    let expanded = quote! {
        #module
    };
    
    TokenStream::from(expanded)
}

/// Entfernt rekursiv alle #[defaults(...)] Attribute aus dem Modul
fn cleanup_defaults_attributes(items: &mut Vec<syn::Item>) {
    for item in items.iter_mut() {
        match item {
            syn::Item::Fn(func) => {
                // Entferne defaults-Attribute von Funktionen
                remove_defaults_attrs(&mut func.attrs);
            }
            syn::Item::Mod(submodule) => {
                // Rekursiv in Submodule gehen
                if let Some((_, subitems)) = &mut submodule.content {
                    cleanup_defaults_attributes(subitems);
                }
            }
            _ => {}
        }
    }
}

/// Entfernt alle #[defaults(...)] Attribute aus einer Attribut-Liste
fn remove_defaults_attrs(attrs: &mut Vec<syn::Attribute>) {
    attrs.retain(|attr| {
        // Behalte nur Attribute, die NICHT "defaults" sind
        !attr.path().is_ident("defaults")
    });
}
