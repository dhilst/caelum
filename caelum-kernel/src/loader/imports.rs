//! Import resolution and module merging, abstracted over a [`ModuleResolver`]
//! so the kernel never touches the filesystem. The CLI supplies a `std::fs`
//! resolver; wasm supplies an in-memory map. All the interesting logic here —
//! cycle detection, dedup, depth-first import ordering — is pure and shared.

use std::collections::HashSet;
use std::path::Path;

use crate::diagnostics::{CaelumError, Result};
use crate::syntax::{parse_source_file, SourceFile};

/// Opaque, hashable identity for a module. The CLI uses a canonical filesystem
/// path; wasm uses a virtual map key. The kernel only compares and clones it.
pub type ModuleId = String;

/// Environment-specific hooks the loader needs: turn an import string into a
/// canonical module id, and fetch a module's source text.
pub trait ModuleResolver {
    /// Resolve the raw `import` string (as written in `importer`) to a stable
    /// canonical [`ModuleId`]. Implementations own any search-path policy and
    /// existence checking.
    fn resolve(&self, importer: &ModuleId, import: &str) -> Result<ModuleId>;

    /// Fetch the source text for a canonical [`ModuleId`].
    fn read(&self, id: &ModuleId) -> Result<String>;
}

#[derive(Debug, Clone)]
pub struct LoadedSpec {
    pub root: ModuleId,
    pub files: Vec<ModuleId>,
    pub source: SourceFile,
}

/// Load `root` and everything it (transitively) imports, merging all items into
/// a single [`SourceFile`] with imported items ordered before their importer's.
pub fn load_spec_with(root: &ModuleId, resolver: &dyn ModuleResolver) -> Result<LoadedSpec> {
    let mut loader = Loader::default();
    let source = loader.load_file(root, resolver)?;
    Ok(LoadedSpec {
        root: root.clone(),
        files: loader.files,
        source,
    })
}

#[derive(Default)]
struct Loader {
    loaded: HashSet<ModuleId>,
    files: Vec<ModuleId>,
    stack: Vec<ModuleId>,
}

impl Loader {
    fn load_file(&mut self, id: &ModuleId, resolver: &dyn ModuleResolver) -> Result<SourceFile> {
        if let Some(position) = self.stack.iter().position(|entry| entry == id) {
            let mut chain = self.stack[position..].to_vec();
            chain.push(id.clone());
            return Err(CaelumError::Import {
                message: format!("import cycle detected: {}", chain.join(" -> ")),
            });
        }

        if !self.loaded.insert(id.clone()) {
            return Ok(SourceFile {
                module: None,
                imports: Vec::new(),
                items: Vec::new(),
            });
        }

        self.stack.push(id.clone());

        let source = resolver.read(id)?;
        let parsed = parse_source_file(Path::new(id), &source)?;

        let mut combined = SourceFile {
            module: parsed.module.clone(),
            imports: parsed.imports.clone(),
            items: Vec::new(),
        };

        for import in &parsed.imports {
            let imported_id = resolver.resolve(id, &import.path)?;
            let mut imported = self.load_file(&imported_id, resolver)?;
            combined.items.append(&mut imported.items);
        }

        combined.items.extend(parsed.items);
        self.files.push(id.clone());
        self.stack.pop();

        Ok(combined)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    /// In-memory resolver: import strings are treated as module ids directly.
    struct MapResolver {
        files: HashMap<String, String>,
    }

    impl ModuleResolver for MapResolver {
        fn resolve(&self, _importer: &ModuleId, import: &str) -> Result<ModuleId> {
            Ok(import.to_string())
        }
        fn read(&self, id: &ModuleId) -> Result<String> {
            self.files
                .get(id)
                .cloned()
                .ok_or_else(|| CaelumError::ReadFile {
                    path: id.clone(),
                    message: "not found".into(),
                })
        }
    }

    #[test]
    fn loads_imported_items_before_root_items() {
        let files = HashMap::from([
            ("common.lum".to_string(), "const max = 3\n".to_string()),
            (
                "main.lum".to_string(),
                "import \"common.lum\"\nlet x: 0..max\n".to_string(),
            ),
        ]);
        let resolver = MapResolver { files };

        let spec = load_spec_with(&"main.lum".to_string(), &resolver).expect("load spec");

        assert_eq!(spec.source.items.len(), 2);
        assert_eq!(spec.files.len(), 2);
    }

    #[test]
    fn detects_import_cycles() {
        let files = HashMap::from([
            ("a.lum".to_string(), "import \"b.lum\"\n".to_string()),
            ("b.lum".to_string(), "import \"a.lum\"\n".to_string()),
        ]);
        let resolver = MapResolver { files };

        let err = load_spec_with(&"a.lum".to_string(), &resolver).expect_err("cycle should fail");

        assert!(err.to_string().contains("import cycle detected"));
    }
}
