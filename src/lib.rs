use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, Item, ItemFn, ItemMod, UseTree, Visibility
};

/// Repräsentiert eine einzelne öffentliche Funktion
#[derive(Debug)]
struct Tusk {
    function_name: String,
}

/// This is just a reference to a module defined elsewhere
#[derive(Debug)]
struct LinkNode {
    module_path: Vec<String>,
}

/// Repräsentiert das gesamte Modul mit
/// allen öffentlichen Funktionen (tusks)
/// und öffentlichen Untermodulen (childs)
#[derive(Debug)]
struct TusksNode {
    module_name: String,
    tusks: Vec<Tusk>,
    childs: Vec<TusksNode>,
    links: Vec<LinkNode>,
}

impl TusksNode {
    pub fn from_module(module: &ItemMod) -> Result<Self, syn::Error> {
        let module_name = module.ident.to_string();
        
        let items = module.content
            .as_ref()
            .map(|(_, items)| items.as_slice())
            .unwrap_or(&[]);
        
        let mut node = TusksNode {
            module_name,
            tusks: Vec::new(),
            childs: Vec::new(),
            links: Vec::new(),
        };
        
        node.extract_module_items(items);
        
        // Prüfe ob es öffentliche Items gibt (nur auf oberster Ebene)
        if node.tusks.is_empty() && node.childs.is_empty() && node.links.is_empty() {
            return Err(syn::Error::new_spanned(
                module,
                "module must contain at least one public function or public submodule"
            ));
        }
        
        Ok(node)
    }

    fn add_child(&mut self, module: &ItemMod) -> Result<(), syn::Error> {
        let child_node = Self::from_module(module)?;
        
        self.childs.push(child_node);
        
        Ok(())
    }

    fn add_link(&mut self, module_path: Vec<String>) {
        self.links.push(LinkNode { module_path });
    }

    fn add_tusk(&mut self, func: &ItemFn) {
        self.tusks.push(Tusk {
            function_name: func.sig.ident.to_string(),
        });
    }
    
    fn extract_module_items(&mut self, items: &[Item]) {
        for item in items {
            match item {
                Item::Mod(submodule) if matches!(submodule.vis, Visibility::Public(_)) => {
                    // ignore empty submodules, which would err
                    let _ = self.add_child(submodule);
                }
                Item::Fn(func) if matches!(func.vis, Visibility::Public(_)) => {
                    self.add_tusk(func);
                }
                Item::Use(use_item) if matches!(use_item.vis, Visibility::Public(_)) => {
                    // extract link nodes from 'use ...' statements
                    self.extract_use_paths(&use_item.tree, vec![]);
                }
                _ => {}
            }
        }
    }
    
    fn extract_use_paths(&mut self, tree: &UseTree, mut prefix: Vec<String>) {
        match tree {
            UseTree::Path(use_path) => {
                // use foo::<rest>
                prefix.push(use_path.ident.to_string());
                self.extract_use_paths(&use_path.tree, prefix);
            }
            UseTree::Name(use_name) => {
                // use foo
                prefix.push(use_name.ident.to_string());
                self.add_link(prefix);
            }
            UseTree::Rename(use_rename) => {
                // use foo as bar => take bar as path
                let alias = use_rename.rename.to_string();
                self.add_link(vec![alias]);
            }
            UseTree::Glob(_) => {
                // use foo::* => take foo as path
                self.add_link(prefix);
            }
            UseTree::Group(use_group) => {
                // e.g. use foo::{bar, baz};
                for item in &use_group.items {
                    self.extract_use_paths(item, prefix.clone());
                }
            }
        }
    }
}

#[proc_macro_attribute]
pub fn tusks(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse das Modul
    let module = parse_macro_input!(item as ItemMod);
    
    // Erstelle den TusksNode-Baum
    let tusks_tree = match TusksNode::from_module(&module) {
        Ok(tree) => tree,
        Err(err) => return err.to_compile_error().into(),
    };
    
    // Debug-Ausgabe zur Compile-Zeit
    eprintln!("Tusks Tree: {:#?}", tusks_tree);
    
    // Gebe das ursprüngliche Modul zurück
    let expanded = quote! {
        #module
    };
    
    TokenStream::from(expanded)
}
