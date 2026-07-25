//! Filesystem-backed [`ModuleResolver`] for the CLI. This is where all the
//! `std::fs` coupling that used to live in the kernel loader now lives:
//! canonicalization (the module id), existence probing across include paths,
//! and reading source text.

use std::path::{Path, PathBuf};

use caelum_kernel::diagnostics::{CaelumError, Result};
use caelum_kernel::loader::{ModuleId, ModuleResolver};

pub struct StdFsResolver {
    include_paths: Vec<PathBuf>,
}

impl StdFsResolver {
    pub fn new(include_paths: Vec<PathBuf>) -> Self {
        Self { include_paths }
    }

    /// Canonicalize a path into a [`ModuleId`] (a canonical path string). Fails
    /// with `ReadFile` if the path does not exist / cannot be resolved.
    pub fn canonical_id(path: &Path) -> Result<ModuleId> {
        validate_extension(path)?;
        let canonical = path.canonicalize().map_err(|source| CaelumError::ReadFile {
            path: path.display().to_string(),
            message: source.to_string(),
        })?;
        Ok(canonical.display().to_string())
    }
}

impl ModuleResolver for StdFsResolver {
    fn resolve(&self, importer: &ModuleId, import: &str) -> Result<ModuleId> {
        let import_path = Path::new(import);
        validate_extension(import_path)?;

        let mut candidates = Vec::new();
        if import_path.is_absolute() {
            candidates.push(import_path.to_path_buf());
        } else {
            if let Some(parent) = Path::new(importer).parent() {
                candidates.push(parent.join(import_path));
            }
            for include_path in &self.include_paths {
                candidates.push(include_path.join(import_path));
            }
        }

        for candidate in &candidates {
            if candidate.exists() {
                return StdFsResolver::canonical_id(candidate);
            }
        }

        Err(CaelumError::Import {
            message: format!("could not resolve import `{import}` from {importer}"),
        })
    }

    fn read(&self, id: &ModuleId) -> Result<String> {
        std::fs::read_to_string(id).map_err(|source| CaelumError::ReadFile {
            path: id.clone(),
            message: source.to_string(),
        })
    }
}

fn validate_extension(path: &Path) -> Result<()> {
    if path.extension().and_then(|ext| ext.to_str()) == Some("lum") {
        Ok(())
    } else {
        Err(CaelumError::InvalidExtension {
            path: path.display().to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use caelum_kernel::loader::load_spec_with;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn loads_imported_items_before_root_items() {
        let dir = tempdir().expect("tempdir");
        let common = dir.path().join("common.lum");
        let root = dir.path().join("main.lum");
        fs::write(&common, "const max = 3\n").expect("write common");
        fs::write(&root, "import \"common.lum\"\nlet x: 0..max\n").expect("write root");

        let resolver = StdFsResolver::new(Vec::new());
        let root_id = StdFsResolver::canonical_id(&root).expect("canonicalize root");
        let spec = load_spec_with(&root_id, &resolver).expect("load spec");

        assert_eq!(spec.source.items.len(), 2);
        assert_eq!(spec.files.len(), 2);
    }

    #[test]
    fn detects_import_cycles() {
        let dir = tempdir().expect("tempdir");
        let a = dir.path().join("a.lum");
        let b = dir.path().join("b.lum");
        fs::write(&a, "import \"b.lum\"\n").expect("write a");
        fs::write(&b, "import \"a.lum\"\n").expect("write b");

        let resolver = StdFsResolver::new(Vec::new());
        let a_id = StdFsResolver::canonical_id(&a).expect("canonicalize a");
        let err = load_spec_with(&a_id, &resolver).expect_err("cycle should fail");

        assert!(err.to_string().contains("import cycle detected"));
    }
}
