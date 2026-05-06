use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::diagnostics::{Result, TplError};
use crate::syntax::{parse_source_file, ImportDecl, SourceFile};

#[derive(Debug, Clone, Default)]
pub struct LoadOptions {
    pub include_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct LoadedSpec {
    pub root: PathBuf,
    pub files: Vec<PathBuf>,
    pub source: SourceFile,
}

#[derive(Default)]
struct Loader {
    options: LoadOptions,
    loaded: HashSet<PathBuf>,
    files: Vec<PathBuf>,
    stack: Vec<PathBuf>,
}

pub fn load_spec(path: &Path, options: &LoadOptions) -> Result<LoadedSpec> {
    validate_tpl_extension(path)?;
    let root = canonicalize_existing(path)?;
    let mut loader = Loader {
        options: options.clone(),
        ..Loader::default()
    };
    let source = loader.load_file(&root)?;

    Ok(LoadedSpec {
        root,
        files: loader.files,
        source,
    })
}

impl Loader {
    fn load_file(&mut self, path: &Path) -> Result<SourceFile> {
        let canonical = canonicalize_existing(path)?;

        if let Some(position) = self.stack.iter().position(|entry| entry == &canonical) {
            let mut chain = self.stack[position..].to_vec();
            chain.push(canonical);
            return Err(TplError::Import {
                message: format!("import cycle detected: {}", format_chain(&chain)),
            });
        }

        if !self.loaded.insert(canonical.clone()) {
            return Ok(SourceFile {
                module: None,
                imports: Vec::new(),
                items: Vec::new(),
            });
        }

        self.stack.push(canonical.clone());

        let source = fs::read_to_string(&canonical).map_err(|source| TplError::ReadFile {
            path: canonical.clone(),
            source,
        })?;
        let parsed = parse_source_file(&canonical, &source)?;

        let mut combined = SourceFile {
            module: parsed.module.clone(),
            imports: parsed.imports.clone(),
            items: Vec::new(),
        };

        for import in &parsed.imports {
            let imported_path = self.resolve_import(&canonical, import)?;
            let mut imported = self.load_file(&imported_path)?;
            combined.items.append(&mut imported.items);
        }

        combined.items.extend(parsed.items);
        self.files.push(canonical);
        self.stack.pop();

        Ok(combined)
    }

    fn resolve_import(&self, importer: &Path, import: &ImportDecl) -> Result<PathBuf> {
        let import_path = Path::new(&import.path);
        validate_tpl_extension(import_path)?;

        let mut candidates = Vec::new();
        if import_path.is_absolute() {
            candidates.push(import_path.to_path_buf());
        } else {
            if let Some(parent) = importer.parent() {
                candidates.push(parent.join(import_path));
            }
            for include_path in &self.options.include_paths {
                candidates.push(include_path.join(import_path));
            }
        }

        for candidate in &candidates {
            if candidate.exists() {
                return canonicalize_existing(candidate);
            }
        }

        Err(TplError::Import {
            message: format!(
                "could not resolve import `{}` from {}",
                import.path,
                importer.display()
            ),
        })
    }
}

fn validate_tpl_extension(path: &Path) -> Result<()> {
    if path.extension().and_then(|ext| ext.to_str()) == Some("tpl") {
        Ok(())
    } else {
        Err(TplError::InvalidExtension {
            path: path.to_path_buf(),
        })
    }
}

fn canonicalize_existing(path: &Path) -> Result<PathBuf> {
    path.canonicalize().map_err(|source| TplError::ReadFile {
        path: path.to_path_buf(),
        source,
    })
}

fn format_chain(chain: &[PathBuf]) -> String {
    chain
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(" -> ")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn loads_imported_items_before_root_items() {
        let dir = tempdir().expect("tempdir");
        let common = dir.path().join("common.tpl");
        let root = dir.path().join("main.tpl");
        fs::write(&common, "const max = 3\n").expect("write common");
        fs::write(&root, "import \"common.tpl\"\nlet x: 0..max\n").expect("write root");

        let spec = load_spec(&root, &LoadOptions::default()).expect("load spec");

        assert_eq!(spec.source.items.len(), 2);
        assert_eq!(spec.files.len(), 2);
    }

    #[test]
    fn detects_import_cycles() {
        let dir = tempdir().expect("tempdir");
        let a = dir.path().join("a.tpl");
        let b = dir.path().join("b.tpl");
        fs::write(&a, "import \"b.tpl\"\n").expect("write a");
        fs::write(&b, "import \"a.tpl\"\n").expect("write b");

        let err = load_spec(&a, &LoadOptions::default()).expect_err("cycle should fail");

        assert!(err.to_string().contains("import cycle detected"));
    }
}
