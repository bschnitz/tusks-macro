use proc_macro::TokenStream;
use syn::Ident;
use syn::parse::Parse;
use syn::{parse_macro_input, ItemMod, parse::ParseStream, LitBool, Token};
use quote::quote;
use tusks_lib::TusksModule;
use tusks_lib::AttributeCheck;


struct TusksAttr {
    debug: bool,
    root: bool,
    derive_debug_for_parameters: bool
}

impl Parse for TusksAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut debug = false;
        let mut root = false;
        let mut derive_debug_for_parameters = false;
        
        // Parse comma-separated list of attributes
        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            
            match ident.to_string().as_str() {
                "debug" => {
                    // Check if followed by = true/false
                    if input.peek(Token![=]) {
                        input.parse::<Token![=]>()?;
                        let value: LitBool = input.parse()?;
                        debug = value.value;
                    } else {
                        // Just "debug" means true
                        debug = true;
                    }
                }
                "root" => {
                    // Check if followed by = true/false
                    if input.peek(Token![=]) {
                        input.parse::<Token![=]>()?;
                        let value: LitBool = input.parse()?;
                        root = value.value;
                    } else {
                        // Just "root" means true
                        root = true;
                    }
                }
                "derive_debug_for_parameters" => {
                    // Check if followed by = true/false
                    if input.peek(Token![=]) {
                        input.parse::<Token![=]>()?;
                        let value: LitBool = input.parse()?;
                        derive_debug_for_parameters = value.value;
                    } else {
                        // Just "derive_debug_for_parameters" means true
                        root = true;
                    }
                }
                other => {
                    return Err(syn::Error::new(
                        ident.span(),
                        format!("unknown tusks attribute: {}", other)
                    ));
                }
            }
            
            // Parse comma if not at end
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        
        Ok(TusksAttr { debug, root, derive_debug_for_parameters })
    }
}

#[proc_macro_attribute]
pub fn tusks(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // 1. Validate that it's called on a module
    let mut module = parse_macro_input!(item as ItemMod);

    let mut args = parse_macro_input!(_attr as TusksAttr);

    args.debug = args.debug || cfg!(feature = "debug");
    
    // 2. Parse with TusksModule::from_module
    let mut tusks_module = match TusksModule::from_module(module.clone(), args.root, true) {
        Ok(Some(tm)) => tm,
        Ok(None) => return TokenStream::from(quote! {#module}),
        Err(e) => return e.to_compile_error().into(),
    };

    // Add missing Parameters structs and connect them via super_ field
    if let Err(e) = tusks_module.supplement_parameters(
        &mut module,
        args.root,
        args.derive_debug_for_parameters
    ) {
        return e.to_compile_error().into();
    }
    
    // 3. Clean the original module from #[arg] and #[parameters] attributes
    let cleaned_module = clean_attributes_from_module(module);
    
    // 4. Insert __internal_tusks_module with cli
    let extended_module = insert_internal_module(cleaned_module, &tusks_module, &args);
    
    if args.debug {
        eprintln!("Parsed TusksModule: {:#?}", tusks_module);
    }
    
    // Return the final module
    TokenStream::from(quote! {
        #extended_module
    })
}

/// Remove #[arg] and #[parameters] attributes from a module and all its items
fn clean_attributes_from_module(mut module: ItemMod) -> ItemMod {
    // Don't clean module-level attributes
    
    // Clean attributes in module content
    clean_module_attributes(&mut module);
    if let Some((brace, ref mut items)) = module.content {
        for item in items.iter_mut() {
            clean_item_attributes(item);
        }
        module.content = Some((brace, items.clone()));
    }
    
    module
}

fn clean_module_attributes(module: &mut ItemMod) {
    module.attrs.retain(
        |attr|
            !attr.path().is_ident("command")
            && !attr.path().is_ident("subcommands")
            && !attr.path().is_ident("external_subcommands")
    );
}

/// Recursively clean attributes from an item
fn clean_item_attributes(item: &mut syn::Item) {
    match item {
        syn::Item::Struct(s) => {
            if s.has_attr("skip") {
                s.attrs.retain(|attr| !attr.path().is_ident("skip"));
            }
            else {
                // Clean #[arg] from field attributes
                for field in s.fields.iter_mut() {
                    field.attrs.retain(|attr| !attr.path().is_ident("arg"));
                }
            }
        }
        syn::Item::Fn(f) => {
            if f.has_attr("skip") {
                f.attrs.retain(|attr| !attr.path().is_ident("skip"));
            }
            else {
                f.attrs.retain(|attr| !attr.path().is_ident("command"));

                // Clean #[arg] from parameter attributes
                for input in f.sig.inputs.iter_mut() {
                    if let syn::FnArg::Typed(pat_type) = input {
                        pat_type.attrs.retain(|attr| !attr.path().is_ident("arg"));
                    }
                }
            }
        }
        syn::Item::Mod(m) => {
            if m.has_attr("skip") {
                m.attrs.retain(|attr| !attr.path().is_ident("skip"));
            }
            else {
                clean_module_attributes(m);

                // Recursively clean submodules
                if let Some((brace, ref mut items)) = m.content {
                    for subitem in items.iter_mut() {
                        clean_item_attributes(subitem);
                    }
                    m.content = Some((brace, items.clone()));
                }
            }
        }
        syn::Item::Use(u) => {
            if u.has_attr("skip") {
                u.attrs.retain(|attr| !attr.path().is_ident("skip"));
            }
            else {
                u.attrs.retain(|attr| !attr.path().is_ident("command"));
            }
        }
        _ => {
            // Don't clean other items
        }
    }
}

/// Insert the __internal_tusks_module with cli into the cleaned module
fn insert_internal_module(
    mut module: ItemMod,
    tusks_module: &TusksModule,
    attr: &TusksAttr
) -> ItemMod {
    // Generate the cli module content
    let cli_content = tusks_module.build_cli(Vec::new(), attr.debug);
    let handle_matches = tusks_module.build_handle_matches(attr.root);

    let exec_cli = match attr.root {
        false => quote! {},
        true => quote! {
            pub fn exec_cli() {
                use tusks::clap::Parser;

                let cli = cli::Cli::parse();
                handle_matches(&cli);
            }
        }
    };
    
    // Build the __internal_tusks_module
    let internal_module = quote! {
        pub mod __internal_tusks_module {
            // -----------------------------
            // CLI-Struktur
            // -----------------------------
            pub mod cli {
                #cli_content
            }
            
            #handle_matches

            #exec_cli
        }
    };
    
    // Parse the internal module as an Item
    let internal_item: syn::Item = syn::parse2(internal_module)
        .expect("Failed to parse internal module");
    
    // Add it to the module content
    if let Some((brace, ref mut items)) = module.content {
        items.push(internal_item);
        module.content = Some((brace, items.clone()));
    }
    
    module
}
