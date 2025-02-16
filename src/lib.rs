use std::fs;
use std::path::Path;
use std::io::{self, ErrorKind, Write};
use colored::Colorize;
use similar::{ChangeTag, TextDiff};
use std::process::Command;
use git2::Repository;
use parse_changelog::{Parser, Release};
use indexmap::IndexMap;
use chrono::Local;

pub struct Changelog {
    path: Box<Path>,
}

fn infer_git_range_url() -> Option<String> {
    // Try to find git repository
    if let Ok(repo) = Repository::discover(".") {
        // Get the origin remote URL
        if let Ok(remote) = repo.find_remote("origin") {
            if let Some(url) = remote.url() {
                // Convert SSH URLs to HTTPS
                let url = url.replace("git@github.com:", "https://github.com/");
                
                // Extract owner/repo from GitHub URLs
                if url.contains("github.com") {
                    let url = url.trim_end_matches(".git").to_string();
                    return Some(format!("{}/compare/<range>", url));
                }
            }
        }
    }
    None
}

const EDITOR_TEMPLATE: &str = r#"{commits}

# Review commits and add them to the changelog
# Lines starting with '#' will be ignored
# Prefix each commit with one of:
#   added (a), changed (c), deprecated (d), removed (r), fixed (f), security (s)
# You can also edit the commit message - it will be used as the changelog entry
#
# Example:
# added 1234567 Add new feature
# changed 89abcde Update existing functionality
"#;

impl Changelog {
    fn show_diff(&self, version: Option<&str>, old_content: &str, new_content: &str) -> io::Result<()> {
        // Get the old version content
        let parser = Parser::new();
        let old_changelog = parser.parse(old_content)
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
        let new_changelog = parser.parse(new_content)
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;

        let version_key = version.unwrap_or("Unreleased");

        let old_version = old_changelog.get(version_key)
            .map(|r| format!("## {}\n\n{}", r.title, r.notes.trim()))
            .unwrap_or_default();

        let new_version = new_changelog.get(version_key)
            .map(|r| format!("## {}\n\n{}", r.title, r.notes.trim()))
            .unwrap_or_default();

        let diff = TextDiff::from_lines(&old_version, &new_version);

        for change in diff.iter_all_changes() {
            match change.tag() {
                ChangeTag::Delete => {
                    print!("{}", format!("-{}", change).red());
                }
                ChangeTag::Insert => {
                    print!("{}", format!("+{}", change).green());
                }
                ChangeTag::Equal => {
                    print!(" {}", change);
                }
            }
        }
        Ok(())
    }
    fn get_editor() -> io::Result<String> {
        // Try VISUAL, then EDITOR, then fall back to vi/vim/nano
        if let Ok(editor) = std::env::var("VISUAL") {
            return Ok(editor);
        }
        if let Ok(editor) = std::env::var("EDITOR") {
            return Ok(editor);
        }
        for editor in &["vim", "vi", "nano"] {
            if Command::new(editor).arg("--version").output().is_ok() {
                return Ok(editor.to_string());
            }
        }
        Err(io::Error::new(ErrorKind::NotFound, "No editor found"))
    }
    pub fn new() -> Self {
        Changelog {
            path: Path::new("CHANGELOG.md").into(),
        }
    }

    pub fn init(&self) -> io::Result<()> {
        if self.path.exists() {
            eprintln!("CHANGELOG.md already exists");
            return Ok(());
        }

        // Parse empty changelog to get default structure
        let parser = Parser::new();
        let changelog = parser.parse("# Changelog\n## [Unreleased]")
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;

        // Format and write the changelog
        let content = changelog_to_markdown(&changelog, "# Changelog\n\n", None);
        fs::write(&self.path, content)?;
        println!("Created CHANGELOG.md");
        Ok(())
    }

    pub fn add(&self, description: &str, type_: &str, version: Option<&str>, show_diff: bool) -> io::Result<()> {
        if !self.path.exists() {
            return Err(io::Error::new(
                ErrorKind::NotFound,
                "CHANGELOG.md does not exist. Run 'changelog init' first.",
            ));
        }

        let content = fs::read_to_string(&self.path)?;
        let parser = Parser::new();
        let mut changelog = parser.parse(&content)
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;

        // Determine which version to add to
        let version_key = version.unwrap_or("Unreleased");

        // Create or get the version entry
        if !changelog.contains_key(version_key) {
            return Err(io::Error::new(
                ErrorKind::NotFound,
                format!("Version {} not found in changelog", version_key),
            ));
        }

        // Get the release entry
        let release = changelog.get_mut(version_key).unwrap();

        // Find the appropriate section
        let section = match type_.to_lowercase().as_str() {
            "added" | "a" => "added",
            "changed" | "c" => "changed",
            "deprecated" | "d" => "deprecated",
            "removed" | "r" => "removed",
            "fixed" | "f" => "fixed",
            "security" | "s" => "security",
            _ => return Err(io::Error::new(
                ErrorKind::InvalidInput,
                format!("Invalid change type: {}. Must be one of: added (a), changed (c), deprecated (d), removed (r), fixed (f), security (s)", type_),
            )),
        };

        // Add the entry to the appropriate section
        let section_marker = format!("### {}", section[..1].to_uppercase() + &section[1..]);
        let mut lines: Vec<String> = release.notes.lines().map(String::from).collect();

        if let Some(section_idx) = lines.iter().position(|line| line.trim() == section_marker) {
            // Existing section found - insert entry
            let mut insert_idx = section_idx + 1;
            while insert_idx < lines.len() {
                let line = lines[insert_idx].trim();
                if line.is_empty() || line.starts_with('-') {
                    insert_idx += 1;
                } else {
                    break;
                }
            }
            // Remove any extra blank lines before insertion
            while insert_idx > section_idx + 1 && lines[insert_idx - 1].trim().is_empty() {
                lines.remove(insert_idx - 1);
                insert_idx -= 1;
            }
            lines.insert(insert_idx, format!("- {}\n", description));
        } else {
            // Section doesn't exist - create it
            // Find where to insert the new section
            let mut insert_idx = 0;

            // Skip past the version header
            while insert_idx < lines.len() && !lines[insert_idx].starts_with("### ") {
                insert_idx += 1;
            }

            // Insert the new section
            lines.insert(insert_idx, section_marker);
            lines.insert(insert_idx + 1, String::new());
            lines.insert(insert_idx + 2, format!("- {}", description));
            lines.insert(insert_idx + 3, String::new());
        }

        let notes = lines.join("\n");
        release.notes = Box::leak(notes.into_boxed_str());

        // Get old content for diff
        let old_content = fs::read_to_string(&self.path)?;

        // Generate new content
        let new_content = changelog_to_markdown(&changelog, &old_content, None);

        // Write new content
        fs::write(&self.path, &new_content)?;

        if show_diff {
            self.show_diff(version, &old_content, &new_content)?;
        }

        Ok(())
    }

    pub fn fmt(&self) -> io::Result<()> {
        if !self.path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "CHANGELOG.md does not exist. Run 'changelog init' first.",
            ));
        }

        let content = fs::read_to_string(&self.path)?;
        let parser = Parser::new();
        let parsed = parser.parse(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        fs::write(&self.path, changelog_to_markdown(&parsed, &content, infer_git_range_url().as_deref()))?;
        println!("Formatted CHANGELOG.md");
        Ok(())
    }

    fn get_next_version(&self, latest_version: &str, change_type: &str) -> io::Result<String> {
        let version = semver::Version::parse(latest_version)
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;

        let new_version = match change_type.to_lowercase().as_str() {
            "major" => semver::Version::new(version.major + 1, 0, 0),
            "minor" => semver::Version::new(version.major, version.minor + 1, 0),
            "patch" => semver::Version::new(version.major, version.minor, version.patch + 1),
            _ => return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "Change type must be one of: major, minor, patch"
            ))
        };

        Ok(new_version.to_string())
    }

    pub fn release(&self, version_or_type: &str, date: Option<&str>) -> io::Result<()> {
        if !self.path.exists() {
            return Err(io::Error::new(
                ErrorKind::NotFound,
                "CHANGELOG.md does not exist. Run 'changelog init' first.",
            ));
        }

        // Determine the version to release
        let version_str = if ["major", "minor", "patch"].contains(&version_or_type.to_lowercase().as_str()) {
            // Get the latest version and increment it
            let content = fs::read_to_string(&self.path)?;
            let parser = Parser::new();
            let changelog = parser.parse(&content)
                .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;

            let latest_version = changelog.keys()
                .filter(|&k| *k != "Unreleased")
                .next()
                .and_then(|v| v.split_whitespace().next())
                .ok_or_else(|| io::Error::new(ErrorKind::NotFound, "No previous version found"))?;

            self.get_next_version(latest_version, version_or_type)?
        } else {
            // Validate the provided version is a valid semver
            semver::Version::parse(version_or_type)
                .map_err(|_| io::Error::new(
                    ErrorKind::InvalidInput,
                    "Version must be a valid semver or one of: major, minor, patch"
                ))?;
            version_or_type.to_string()
        };

        let content = fs::read_to_string(&self.path)?;
        let parser = Parser::new();
        let mut changelog = parser.parse(&content)
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
        let unreleased = match changelog.shift_remove("Unreleased") {
            Some(r) => r,
            None => return Err(io::Error::new(ErrorKind::NotFound, "No unreleased section found")),
        };
        let new_title = if let Some(d) = date {
            format!("[{}] - {}", version_str, d)
        } else {
            let today = Local::now().format("%Y-%m-%d").to_string();
            format!("[{}] - {}", version_str, today)
        };
        let new_release_key: &'static str = Box::leak(new_title.clone().into_boxed_str());
        let mut released = unreleased;
        released.title = new_release_key;
        let default_unreleased = {
            let dummy = r#"# Changelog
## [Unreleased]
### Added

### Changed

### Deprecated

### Removed

### Fixed

### Security
"#;
            let mut dummy_changelog = Parser::new().parse(dummy)
                .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
            let default_unreleased = dummy_changelog.shift_remove("Unreleased")
                .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "Failed to parse default unreleased section"))?;
            default_unreleased
        };
        let mut new_changelog = indexmap::IndexMap::new();
        new_changelog.insert("Unreleased", default_unreleased);
        let new_release_key: &'static str = Box::leak(new_title.clone().into_boxed_str());
        new_changelog.insert(new_release_key, released);
        for (k, v) in changelog.into_iter() {
            new_changelog.insert(k, v);
        }
        fs::write(&self.path, changelog_to_markdown(&new_changelog, &content, self.git_range_url.as_deref()))?;
        println!("Released version {}", version_str);
        Ok(())
    }

    pub fn version_latest(&self) -> io::Result<()> {
        if !self.path.exists() {
            return Err(io::Error::new(
                ErrorKind::NotFound,
                "CHANGELOG.md does not exist. Run 'changelog init' first.",
            ));
        }

        let content = fs::read_to_string(&self.path)?;
        let parser = Parser::new();
        let changelog = parser.parse(&content)
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;

        // Find first non-Unreleased version
        if let Some(version) = changelog.keys()
            .filter(|&k| *k != "Unreleased")
            .next() {
                // Take first part (the version) before any date
                let version_only = version.split_whitespace()
                    .next()
                    .unwrap_or("");
                println!("{}", version_only);
                Ok(())
        } else {
            Err(io::Error::new(ErrorKind::NotFound, "No released versions found"))
        }
    }

    pub fn version_show(&self, version: &str) -> io::Result<()> {
        if !self.path.exists() {
            return Err(io::Error::new(
                ErrorKind::NotFound,
                "CHANGELOG.md does not exist. Run 'changelog init' first.",
            ));
        }

        let content = fs::read_to_string(&self.path)?;
        let parser = Parser::new();
        let changelog = parser.parse(&content)
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;

        // Handle special cases
        let version_to_show = match version.to_lowercase().as_str() {
            "latest" => changelog.keys()
                .filter(|&k| *k != "Unreleased")
                .next()
                .ok_or_else(|| io::Error::new(ErrorKind::NotFound, "No released versions found"))?,
            "unreleased" => "Unreleased",
            _ => version
        };

        // Find the requested version
        if let Some(release) = changelog.get(version_to_show) {
            println!("## {}", release.title);
            println!("\n{}", release.notes.trim());
            Ok(())
        } else {
            Err(io::Error::new(ErrorKind::NotFound, format!("Version {} not found", version)))
        }
    }

    pub fn version_list(&self) -> io::Result<()> {
        if !self.path.exists() {
            return Err(io::Error::new(
                ErrorKind::NotFound,
                "CHANGELOG.md does not exist. Run 'changelog init' first.",
            ));
        }

        let content = fs::read_to_string(&self.path)?;
        let parser = Parser::new();
        let changelog = parser.parse(&content)
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;

        // Print all non-Unreleased versions
        for version in changelog.keys().filter(|&k| *k != "Unreleased") {
            // Take first part (the version) before any date
            let version_only = version.split_whitespace()
                .next()
                .unwrap_or("");
            println!("{}", version_only);
        }
        Ok(())
    }

    pub fn range(&self, version: Option<&str>) -> io::Result<()> {
        // Validate version format if provided
        if let Some(v) = version {
            if v.starts_with('v') {
                return Err(io::Error::new(
                    ErrorKind::InvalidInput,
                    "Version should not start with 'v' prefix. Use semantic version format (e.g. '1.0.0')",
                ));
            }
        }

        if !self.path.exists() {
            return Err(io::Error::new(
                ErrorKind::NotFound,
                "CHANGELOG.md does not exist. Run 'changelog init' first.",
            ));
        }

        let content = fs::read_to_string(&self.path)?;
        let parser = Parser::new();
        let changelog = parser.parse(&content)
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;

        // Get the revision range
        let end = match version {
            Some(v) => format!("v{}", v),
            None => "HEAD".to_string()
        };

        // Find the previous version
        let start = if let Some(version) = version {
            // For a specific version, find the version after it in changelog
            changelog.keys()
                .filter(|&k| *k != "Unreleased")
                .skip_while(|&v| *v != version)
                .nth(1)  // Get the next version after the specified one
                .map(|v| format!("v{}", v))
        } else {
            // For HEAD, use the most recent version from changelog
            changelog.keys()
                .filter(|&k| *k != "Unreleased")
                .next()
                .map(|v| format!("v{}", v))
        };

        match start {
            Some(start) => println!("{}..{}", start, end),
            None => println!("{}", end)
        };

        Ok(())
    }

    pub fn review(&self, version: Option<&str>) -> io::Result<()> {
        // Find git repository
        let repo = Repository::discover(".")
            .map_err(|e| io::Error::new(ErrorKind::NotFound, format!("Git repository not found: {}", e)))?;

        // Get the content to determine the revision range
        let content = fs::read_to_string(&self.path)?;
        let parser = Parser::new();
        let changelog = parser.parse(&content)
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;

        // Get the revision range
        let end = match version {
            Some(v) => format!("v{}", v),
            None => "HEAD".to_string()
        };

        // Find the previous version
        let start = if let Some(version) = version {
            // For a specific version, find the version after it in changelog
            changelog.keys()
                .filter(|&k| *k != "Unreleased")
                .skip_while(|&v| *v != version)
                .nth(1)  // Get the next version after the specified one
                .map(|v| format!("v{}", v))
        } else {
            // For HEAD, use the most recent version from changelog
            changelog.keys()
                .filter(|&k| *k != "Unreleased")
                .next()
                .map(|v| format!("v{}", v))
        };

        // Get commits in the range
        let mut revwalk = repo.revwalk()
            .map_err(|e| io::Error::new(ErrorKind::Other, e))?;

        // Push the end commit
        if end == "HEAD" {
            revwalk.push_head()
                .map_err(|e| io::Error::new(ErrorKind::Other, e))?;
        } else {
            let obj = repo.revparse_single(&end)
                .map_err(|e| io::Error::new(ErrorKind::Other, e))?;
            revwalk.push(obj.id())
                .map_err(|e| io::Error::new(ErrorKind::Other, e))?;
        }

        // Hide the start commit if it exists
        if let Some(start) = start {
            if let Ok(obj) = repo.revparse_single(&start) {
                revwalk.hide(obj.id())
                    .map_err(|e| io::Error::new(ErrorKind::Other, e))?;
            }
        }

        // Collect commits for selection
        let mut commit_list = Vec::new();
        for oid in revwalk {
            let oid = oid.map_err(|e| io::Error::new(ErrorKind::Other, e))?;
            let commit = repo.find_commit(oid)
                .map_err(|e| io::Error::new(ErrorKind::Other, e))?;

            let short_id = commit.id().to_string()[..7].to_string();
            let message = commit.message().unwrap_or("").lines().next().unwrap_or("").trim();
            commit_list.push((short_id, message.to_string()));
        }

        // Parse conventional commits and pre-select feat/fix
        let mut defaults = vec![false; commit_list.len()];
        for (idx, (_id, msg)) in commit_list.iter().enumerate() {
            if let Ok(conv_commit) = git_conventional::Commit::parse(msg) {
                if conv_commit.type_().to_string() == "feat" || conv_commit.type_().to_string() == "fix" {
                    defaults[idx] = true;
                }
            }
        }

        // Let user select commits
        let selections = dialoguer::MultiSelect::new()
            .with_prompt("Select commits to include in changelog (press 'a' to select all)")
            .items(&commit_list.iter().map(|(id, msg)| format!("{} {}", id, msg)).collect::<Vec<_>>())
            .report(false)
            .defaults(&defaults)
            .interact()
            .map_err(|e| io::Error::new(ErrorKind::Other, e))?;

        if selections.is_empty() {
            return Ok(());
        }

        // Build commit list for editor using only selected commits
        let mut commits = String::new();
        for &idx in selections.iter() {
            let (short_id, message) = &commit_list[idx];
            // Parse commit message to determine type
            let (type_code, display_message) = if let Ok(conv_commit) = git_conventional::Commit::parse(message) {
                let type_str = match conv_commit.type_().to_string().as_str() {
                    "feat" => "added",
                    "fix" => "fixed",
                    _ => "changed"
                };
                // Remove the type prefix from conventional commits
                let msg = conv_commit.description().to_string();
                (type_str, msg)
            } else {
                ("changed", message.to_string()) // default to changed for non-conventional commits
            };
            commits.push_str(&format!("{} {} {}\n", type_code, short_id, display_message));
        }

        // Create temporary directory and file with git-rebase-todo name for proper editor highlighting
        let temp_dir = tempfile::Builder::new()
            .prefix("rebase-merge")
            .tempdir()?;
        let temp_path = temp_dir.path().join("git-rebase-todo");
        let mut temp = std::fs::File::create(&temp_path)?;
        let template = EDITOR_TEMPLATE.replace("{commits}", &commits);
        temp.write_all(template.as_bytes())?;
        temp.flush()?;

        // Open editor
        let editor = Self::get_editor()?;
        let status = Command::new(editor)
            .arg(&temp_path)
            .status()?;

        if !status.success() {
            return Err(io::Error::new(ErrorKind::Other, "Editor returned error"));
        }

        // Read edited content
        let content = fs::read_to_string(&temp_path)?;

        // Get old content before processing
        let old_content = fs::read_to_string(&self.path)?;

        // Process each line
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.splitn(3, ' ').collect();
            if parts.len() != 3 {
                continue;
            }

            let type_str = parts[0];
            let description = parts[2];

            // Normalize single-char types
            let type_ = match type_str {
                "a" => "added",
                "c" => "changed",
                "d" => "deprecated",
                "r" => "removed",
                "f" => "fixed",
                "s" => "security",
                _ => type_str
            };

            // Add the entry without showing individual diffs
            self.add(description, type_, version, false)?;
        }

        // Show the overall diff
        let new_content = fs::read_to_string(&self.path)?;
        self.show_diff(version, &old_content, &new_content)?;

        Ok(())
    }
}

fn changelog_to_markdown(changelog: &IndexMap<&str, Release>, original: &str, git_range_url: Option<&str>) -> String {
    let header = extract_header(original).unwrap_or_else(|| "# Changelog\n\n".to_string());
    let mut output = header;
    let mut version_links = Vec::new();
    output.push_str("\n");
    // Add version sections
    for (_version, release) in changelog {
        if !release.notes.contains("# Changelog") {
            let mut lines: Vec<_> = release.notes.lines().collect();
            if let Some(pos) = lines.iter().position(|line| line.trim().starts_with("## ")) {
                lines.drain(pos..=pos);
                while pos < lines.len() && lines[pos].trim().is_empty() {
                    lines.remove(pos);
                }
            }
            if !output.ends_with("\n\n") {
                output.push_str("\n");
            }
            let title = if git_range_url.is_some() {
                release.title.to_string()
            } else {
                release.title.replace("[", "").replace("]", "")
            };
            output.push_str(&format!("## {}\n\n", title));
            let mut filtered_sections = Vec::new();
            let mut current_section_header = "";
            let mut current_section_lines = Vec::new();
            for line in lines {
                if line.trim().starts_with("### ") {
                    if !current_section_header.is_empty() {
                        let content_exists = current_section_lines.iter().any(|l: &&str| {
                            !l.trim().is_empty() && !l.trim().starts_with('#')
                        });
                        if content_exists {
                            filtered_sections.push(current_section_header.to_string());
                            filtered_sections.extend(current_section_lines.clone().into_iter().map(|s| s.to_string()));
                        }
                    }
                    current_section_header = line;
                    current_section_lines.clear();
                } else {
                    current_section_lines.push(line);
                }
            }
            if !current_section_header.is_empty() {
                let content_exists = current_section_lines.iter().any(|l: &&str| {
                    !l.trim().is_empty() && !l.trim().starts_with('#')
                });
                if content_exists {
                    filtered_sections.push(current_section_header.to_string());
                    filtered_sections.extend(current_section_lines.into_iter().map(|s| s.to_string()));
                }
            }
            if !filtered_sections.is_empty() {
                output.push_str(&filtered_sections.join("\n"));
                output.push_str("\n");
            }

            // Extract version for link
            if let Some(version) = release.title.split_whitespace().next() {
                version_links.push(version.trim_matches(|c| c == '[' || c == ']').to_string());
            }
        }
    }

    // Add version links if git_range_url is provided
    if let Some(url_template) = git_range_url {
        if !version_links.is_empty() {
            output.push_str("\n");

            // Add links for each version
            for (i, version) in version_links.iter().enumerate() {
                let next_ver = version_links.get(i + 1).map(|v| format!("v{}", v)).unwrap_or_else(|| "HEAD".to_string());
                let prev_ver = if version == "Unreleased" {
                    "HEAD".to_string()
                } else {
                    format!("v{}", version)
                };

                let range = format!("{}...{}", next_ver, prev_ver);
                let url = url_template.replace("<range>", &range);
                output.push_str(&format!("[{}]: {}\n", version, url));
            }
        }
    }
    return output
    // // Format the markdown using comrak's format_commonmark formatter
    // let options = ComrakOptions::default();
    // let arena = comrak::Arena::new();
    // let root = comrak::parse_document(&arena, &output, &options);
    // let mut buf = Vec::new();
    // comrak::format_commonmark(root, &options, &mut buf).unwrap();
    // String::from_utf8(buf).unwrap()
}

fn extract_header(original: &str) -> Option<String> {
    original.split("\n##").next().map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    use parse_changelog::Parser;

    #[test]
    fn test_init_creates_changelog() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().join("CHANGELOG.md");

        let changelog = Changelog {
            path: temp_path.into(),
        };

        // First initialization should succeed
        changelog.init().unwrap();
        assert!(changelog.path.exists());

        // Content should match expected template
        let content = fs::read_to_string(&changelog.path).unwrap();
        assert!(content.contains("# Changelog"));
        assert!(content.contains("## Unreleased"));

        // Parse the content to verify structure
        let parser = Parser::new();
        let parsed = parser.parse(&content).unwrap();
        assert!(parsed.contains_key("Unreleased"));

        // Second initialization should not error but should warn
        changelog.init().unwrap();
    }

    #[test]
    fn test_changelog_to_markdown() {
        let content = r#"# Changelog
All notable changes to this project will be documented in this file.

## [Unreleased]

## [1.0.0] - 2025-01-01

### Added

- First release
- Cool new feature
"#;
        let parser = Parser::new();
        let changelog = parser.parse(content).unwrap();

        let markdown = changelog_to_markdown(&changelog, content, None);

        let expected = r#"# Changelog
All notable changes to this project will be documented in this file.

## Unreleased

## 1.0.0 - 2025-01-01

### Added

- First release
- Cool new feature
"#;
        assert_eq!(markdown, expected);
    }

    #[test]
    fn test_fmt_is_idempotent() {
        let initial_content = r#"# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

### Changed

### Deprecated

### Removed

### Fixed

### Security
"#;

        let parser = Parser::new();

        // First format
        let first_parse = parser.parse(initial_content).unwrap();
        let first_format = changelog_to_markdown(&first_parse, initial_content, None);

        // Second format
        let second_parse = parser.parse(&first_format).unwrap();
        let second_format = changelog_to_markdown(&second_parse, initial_content, None);

        // Third format
        let third_parse = parser.parse(&second_format).unwrap();
        let third_format = changelog_to_markdown(&third_parse, initial_content, None);

        // All formats should be identical
        assert_eq!(first_format, second_format);
        assert_eq!(second_format, third_format);

        // Content should be preserved
        // assert!(first_format.contains("### Added"));
        // assert!(first_format.contains("### Changed"));
        // assert!(first_format.contains("### Deprecated"));
        // assert!(first_format.contains("### Removed"));
        // assert!(first_format.contains("### Fixed"));
        // assert!(first_format.contains("### Security"));
    }

    #[test]
    fn test_changelog_format_exact() {
        let input = r#"# Changelog

## [Unreleased]

### Added

- stuff

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [1.0.0]

### Added

- things"#;

        let expected = r#"# Changelog

## Unreleased

### Added

- stuff

## 1.0.0

### Added

- things
"#;

        let parser = Parser::new();
        let changelog = parser.parse(input).unwrap();
        let markdown = changelog_to_markdown(&changelog, input, None);

        assert_eq!(markdown, expected);
    }

    #[test]
    fn test_changelog_format_with_date() {
        let input = r#"# Changelog

## [1.0.0] - 2025-02-06

### Added
- Initial release"#;

        let expected = r#"# Changelog

## 1.0.0 - 2025-02-06

### Added
- Initial release
"#;

        let parser = Parser::new();
        let changelog = parser.parse(input).unwrap();
        let markdown = changelog_to_markdown(&changelog, input, None);

        assert_eq!(markdown, expected);
    }

    #[test]
    fn test_add_entry_to_section() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().join("CHANGELOG.md");

        // Create initial changelog
        fs::write(&temp_path, r#"# Changelog

## [Unreleased]

### Added

- one
- two

### Changed

- changed

## [1.0.0] - 2000-01-01

### Added

- something
"#).unwrap();

        let changelog = Changelog {
            path: temp_path.into(),
        };

        // Add new entry
        changelog.add("three", "added", None, false).unwrap();

        // Verify result
        let content = fs::read_to_string(&changelog.path).unwrap();
        let expected = r#"# Changelog

## Unreleased

### Added

- one
- two
- three

### Changed

- changed

## 1.0.0 - 2000-01-01

### Added

- something
"#;
        assert_eq!(content, expected);
    }

    #[test]
    fn test_changelog_format_with_git_range_url() {
        let input = r#"# Changelog

## 0.6.11 - 2025-01-19

### CLI

- Accept non-UTF-8 changelog path.

- Remove dependency on `anyhow` (previously used in CLI (`default` feature)).

- Diagnostic improvements.
"#;

        let expected = r#"# Changelog

## [0.6.11] - 2025-01-19

### CLI

- Accept non-UTF-8 changelog path.

- Remove dependency on `anyhow` (previously used in CLI (`default` feature)).

- Diagnostic improvements.

[0.6.11]: https://github.com/taiki-e/parse-changelog/compare/v0.6.10...v0.6.11
"#;

        let parser = Parser::new();
        let changelog = parser.parse(input).unwrap();
        let markdown = changelog_to_markdown(&changelog, input, Some("https://github.com/taiki-e/parse-changelog/compare/<range>"));

        assert_eq!(markdown, expected);
    }

    #[test]
    fn test_preserve_original_header_custom() {
        let input = r#"Custom Header Line 1
Custom Header Line 2

## [Unreleased]

### Added

- entry
"#;
        let parser = Parser::new();
        let changelog = parser.parse(input).unwrap();
        let markdown = changelog_to_markdown(&changelog, input, None);
        assert!(markdown.contains("Custom Header Line 1"));
        assert!(markdown.contains("Custom Header Line 2"));
    }

    #[test]
    fn test_add_entry_creates_missing_section() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().join("CHANGELOG.md");

        // Create initial changelog without Added section
        fs::write(&temp_path, r#"# Changelog

## [Unreleased]

### Changed

- something changed

## [1.0.0] - 2000-01-01

### Added

- something
"#).unwrap();

        let changelog = Changelog {
            path: temp_path.into(),
        };

        // Add new entry that requires Added section
        changelog.add("new feature", "added", None, false).unwrap();

        // Verify result
        let content = fs::read_to_string(&changelog.path).unwrap();
        let expected = r#"# Changelog

## Unreleased

### Added

- new feature

### Changed

- something changed

## 1.0.0 - 2000-01-01

### Added

- something
"#;
        assert_eq!(content, expected);
    }
}
