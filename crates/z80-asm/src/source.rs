//! Source gathering: reading files and expanding `INCLUDE` directives.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// One physical line of source, tagged with where it came from so that
/// errors and listings can point back at the right file.
#[derive(Clone, Debug)]
pub struct SourceLine {
    pub file: String,
    pub number: usize,
    pub text: String,
}

/// How the assembler reads included files. Tests and the GUI supply their
/// own so that assembly does not have to touch the filesystem.
pub trait SourceLoader {
    /// Resolve `path` relative to `from`, returning its contents and the
    /// name to report it under.
    fn load(&self, from: &str, path: &str) -> Result<(String, String), String>;
}

/// Reads includes from the filesystem, relative to the including file.
pub struct FileLoader;

impl SourceLoader for FileLoader {
    fn load(&self, from: &str, path: &str) -> Result<(String, String), String> {
        let base = Path::new(from).parent().unwrap_or_else(|| Path::new("."));
        let resolved: PathBuf = base.join(path);
        let text = std::fs::read_to_string(&resolved)
            .map_err(|error| format!("cannot read '{}': {error}", resolved.display()))?;
        Ok((text, resolved.to_string_lossy().into_owned()))
    }
}

/// A loader that refuses every include; useful for assembling a single
/// buffer that is known not to need any.
pub struct NoIncludes;

impl SourceLoader for NoIncludes {
    fn load(&self, _from: &str, path: &str) -> Result<(String, String), String> {
        Err(format!("INCLUDE '{path}' is not available here"))
    }
}

/// Expand a source file into a flat list of lines, following `INCLUDE` and
/// `#INCLUDE` directives. Include errors are returned as `Err` entries
/// attached to the line that caused them.
pub fn flatten(
    name: &str,
    text: &str,
    loader: &dyn SourceLoader,
) -> (Vec<SourceLine>, Vec<(SourceLine, String)>) {
    let mut lines = Vec::new();
    let mut errors = Vec::new();
    let mut active = HashSet::new();
    expand(name, text, loader, &mut lines, &mut errors, &mut active);
    (lines, errors)
}

fn expand(
    name: &str,
    text: &str,
    loader: &dyn SourceLoader,
    lines: &mut Vec<SourceLine>,
    errors: &mut Vec<(SourceLine, String)>,
    active: &mut HashSet<String>,
) {
    active.insert(name.to_string());

    for (index, raw) in text.lines().enumerate() {
        let line = SourceLine {
            file: name.to_string(),
            number: index + 1,
            text: raw.to_string(),
        };

        match include_target(raw) {
            None => lines.push(line),
            Some(path) => {
                // The line itself is kept in the listing so the include is
                // visible where it happens.
                lines.push(line.clone());
                match loader.load(name, &path) {
                    Ok((_, resolved)) if active.contains(&resolved) => {
                        errors.push((line, format!("'{path}' includes itself")));
                    }
                    Ok((included, resolved)) => {
                        expand(&resolved, &included, loader, lines, errors, active);
                        active.remove(&resolved);
                    }
                    Err(message) => errors.push((line, message)),
                }
            }
        }
    }
}

/// The quoted or bare path of an `INCLUDE` line, if this is one.
fn include_target(raw: &str) -> Option<String> {
    let without_comment = raw.split(';').next().unwrap_or("");
    let trimmed = without_comment.trim();
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let directive = parts.next()?.trim_start_matches('.').to_ascii_uppercase();
    if directive != "INCLUDE" && directive != "#INCLUDE" {
        return None;
    }
    let path = parts.next()?.trim();
    Some(path.trim_matches(['"', '\'']).to_string())
}
