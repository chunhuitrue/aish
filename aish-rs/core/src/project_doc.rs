//! Global AI instructions discovery.
//!
//! Global instructions are stored in files named `AISH.md`.
//! Additional fallback filenames can be configured via `project_doc_fallback_filenames`.
//! Only the global config directory (~/.aish) is checked.

use crate::config::Config;
use crate::skills::SkillMetadata;
use crate::skills::render_skills_section;
use dunce::canonicalize as normalize_path;
use std::path::PathBuf;
use tokio::io::AsyncReadExt;
use tracing::error;

/// Default filename scanned for global instructions.
pub const DEFAULT_INSTRUCTIONS_FILENAME: &str = "AISH.md";

/// When both `Config::instructions` and global instructions are present, they will
/// be concatenated with the following separator.
const INSTRUCTIONS_SEPARATOR: &str = "\n\n--- instructions ---\n\n";

/// Combines `Config::instructions` and `AISH.md` (if present) into a single
/// string of instructions.
pub(crate) async fn get_user_instructions(
    config: &Config,
    skills: Option<&[SkillMetadata]>,
) -> Option<String> {
    let skills_section = skills.and_then(render_skills_section);

    let project_docs = match read_project_docs(config).await {
        Ok(docs) => docs,
        Err(e) => {
            error!("error trying to find project doc: {e:#}");
            return config.user_instructions.clone();
        }
    };

    let combined_project_docs = merge_project_docs_with_skills(project_docs, skills_section);

    let mut parts: Vec<String> = Vec::new();

    if let Some(instructions) = config.user_instructions.clone() {
        parts.push(instructions);
    }

    if let Some(project_doc) = combined_project_docs {
        if !parts.is_empty() {
            parts.push(INSTRUCTIONS_SEPARATOR.to_string());
        }
        parts.push(project_doc);
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.concat())
    }
}

/// Attempt to locate and load the project documentation.
///
/// On success returns `Ok(Some(contents))` where `contents` is the
/// concatenation of all discovered docs. If no documentation file is found the
/// function returns `Ok(None)`. Unexpected I/O failures bubble up as `Err` so
/// callers can decide how to handle them.
pub async fn read_project_docs(config: &Config) -> std::io::Result<Option<String>> {
    let max_total = config.project_doc_max_bytes;

    if max_total == 0 {
        return Ok(None);
    }

    let paths = discover_project_doc_paths(config)?;
    if paths.is_empty() {
        return Ok(None);
    }

    let mut remaining: u64 = max_total as u64;
    let mut parts: Vec<String> = Vec::new();

    for p in paths {
        if remaining == 0 {
            break;
        }

        let file = match tokio::fs::File::open(&p).await {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        };

        let size = file.metadata().await?.len();
        let mut reader = tokio::io::BufReader::new(file).take(remaining);
        let mut data: Vec<u8> = Vec::new();
        reader.read_to_end(&mut data).await?;

        if size > remaining {
            tracing::warn!(
                "Project doc `{}` exceeds remaining budget ({} bytes) - truncating.",
                p.display(),
                remaining,
            );
        }

        let text = String::from_utf8_lossy(&data).to_string();
        if !text.trim().is_empty() {
            parts.push(text);
            remaining = remaining.saturating_sub(data.len() as u64);
        }
    }

    if parts.is_empty() {
        Ok(None)
    } else {
        Ok(Some(parts.join("\n\n")))
    }
}

/// Discover the list of AISH.md files in the global config directory (~/.aish).
/// Symlinks are allowed. When `project_doc_max_bytes` is zero, returns an empty list.
pub fn discover_project_doc_paths(config: &Config) -> std::io::Result<Vec<PathBuf>> {
    let mut dir = config.codex_home.clone();
    if let Ok(canon) = normalize_path(&dir) {
        dir = canon;
    }

    // Only check the global config directory.
    let mut found: Vec<PathBuf> = Vec::new();
    let candidate_filenames = candidate_filenames(config);
    for name in &candidate_filenames {
        let candidate = dir.join(name);
        match std::fs::symlink_metadata(&candidate) {
            Ok(md) => {
                let ft = md.file_type();
                // Allow regular files and symlinks; opening will later fail for dangling links.
                if ft.is_file() || ft.is_symlink() {
                    found.push(candidate);
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        }
    }

    Ok(found)
}

fn candidate_filenames<'a>(config: &'a Config) -> Vec<&'a str> {
    let mut names: Vec<&'a str> =
        Vec::with_capacity(1 + config.project_doc_fallback_filenames.len());
    names.push(DEFAULT_INSTRUCTIONS_FILENAME);
    for candidate in &config.project_doc_fallback_filenames {
        let candidate = candidate.as_str();
        if candidate.is_empty() {
            continue;
        }
        if !names.contains(&candidate) {
            names.push(candidate);
        }
    }
    names
}

fn merge_project_docs_with_skills(
    project_doc: Option<String>,
    skills_section: Option<String>,
) -> Option<String> {
    match (project_doc, skills_section) {
        (Some(doc), Some(skills)) => Some(format!("{doc}\n\n{skills}")),
        (Some(doc), None) => Some(doc),
        (None, Some(skills)) => Some(skills),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigBuilder;
    use crate::skills::load_skills;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Helper that returns a `Config` with codex_home pointing at `codex_home_dir`
    /// and using `limit` as the maximum number of bytes to embed from AISH.md.
    /// The caller can optionally specify a custom `instructions` string – when `None`
    /// the value is cleared to mimic a scenario where no system instructions have
    /// been configured.
    async fn make_config(
        codex_home_dir: &TempDir,
        limit: usize,
        instructions: Option<&str>,
    ) -> Config {
        let mut config = ConfigBuilder::default()
            .codex_home(codex_home_dir.path().to_path_buf())
            .build()
            .await
            .expect("defaults for test should always succeed");

        config.project_doc_max_bytes = limit;
        config.user_instructions = instructions.map(ToOwned::to_owned);
        config
    }

    async fn make_config_with_fallback(
        codex_home_dir: &TempDir,
        limit: usize,
        instructions: Option<&str>,
        fallbacks: &[&str],
    ) -> Config {
        let mut config = make_config(codex_home_dir, limit, instructions).await;
        config.project_doc_fallback_filenames = fallbacks
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        config
    }

    /// AISH.md missing – should yield `None`.
    #[tokio::test]
    async fn no_doc_file_returns_none() {
        let tmp = tempfile::tempdir().expect("tempdir");

        let res = get_user_instructions(&make_config(&tmp, 4096, None).await, None).await;
        assert!(
            res.is_none(),
            "Expected None when AISH.md is absent and no system instructions provided"
        );
        assert!(res.is_none(), "Expected None when AISH.md is absent");
    }

    /// Small file within the byte-limit is returned unmodified.
    #[tokio::test]
    async fn doc_smaller_than_limit_is_returned() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(tmp.path().join("AISH.md"), "hello world").unwrap();

        let res = get_user_instructions(&make_config(&tmp, 4096, None).await, None)
            .await
            .expect("doc expected");

        assert_eq!(
            res, "hello world",
            "The document should be returned verbatim when it is smaller than the limit and there are no existing instructions"
        );
    }

    /// Oversize file is truncated to `project_doc_max_bytes`.
    #[tokio::test]
    async fn doc_larger_than_limit_is_truncated() {
        const LIMIT: usize = 1024;
        let tmp = tempfile::tempdir().expect("tempdir");

        let huge = "A".repeat(LIMIT * 2); // 2 KiB
        fs::write(tmp.path().join("AISH.md"), &huge).unwrap();

        let res = get_user_instructions(&make_config(&tmp, LIMIT, None).await, None)
            .await
            .expect("doc expected");

        assert_eq!(res.len(), LIMIT, "doc should be truncated to LIMIT bytes");
        assert_eq!(res, huge[..LIMIT]);
    }

    /// Explicitly setting the byte-limit to zero disables project docs.
    #[tokio::test]
    async fn zero_byte_limit_disables_docs() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(tmp.path().join("AISH.md"), "something").unwrap();

        let res = get_user_instructions(&make_config(&tmp, 0, None).await, None).await;
        assert!(
            res.is_none(),
            "With limit 0 the function should return None"
        );
    }

    /// When both system instructions *and* a project doc are present the two
    /// should be concatenated with the separator.
    #[tokio::test]
    async fn merges_existing_instructions_with_project_doc() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(tmp.path().join("AISH.md"), "proj doc").unwrap();

        const INSTRUCTIONS: &str = "base instructions";

        let res = get_user_instructions(&make_config(&tmp, 4096, Some(INSTRUCTIONS)).await, None)
            .await
            .expect("should produce a combined instruction string");

        let expected = format!("{INSTRUCTIONS}{INSTRUCTIONS_SEPARATOR}{}", "proj doc");

        assert_eq!(res, expected);
    }

    /// If there are existing system instructions but the project doc is
    /// missing we expect the original instructions to be returned unchanged.
    #[tokio::test]
    async fn keeps_existing_instructions_when_doc_missing() {
        let tmp = tempfile::tempdir().expect("tempdir");

        const INSTRUCTIONS: &str = "some instructions";

        let res =
            get_user_instructions(&make_config(&tmp, 4096, Some(INSTRUCTIONS)).await, None).await;

        assert_eq!(res, Some(INSTRUCTIONS.to_string()));
    }

    /// When AISH.md is absent but a configured fallback exists, the fallback is used.
    #[tokio::test]
    async fn uses_configured_fallback_when_agents_missing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(tmp.path().join("EXAMPLE.md"), "example instructions").unwrap();

        let cfg = make_config_with_fallback(&tmp, 4096, None, &["EXAMPLE.md"]).await;

        let res = get_user_instructions(&cfg, None)
            .await
            .expect("fallback doc expected");

        assert_eq!(res, "example instructions");
    }

    /// AISH.md remains preferred when both AISH.md and fallbacks are present.
    #[tokio::test]
    async fn agents_md_preferred_over_fallbacks() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(tmp.path().join("AISH.md"), "primary").unwrap();
        fs::write(tmp.path().join("EXAMPLE.md"), "secondary").unwrap();

        let cfg = make_config_with_fallback(&tmp, 4096, None, &["EXAMPLE.md", ".example.md"]).await;

        let res = get_user_instructions(&cfg, None)
            .await
            .expect("AISH.md should win");

        assert_eq!(res, "primary");

        let discovery = discover_project_doc_paths(&cfg).expect("discover paths");
        assert_eq!(discovery.len(), 1);
        assert!(
            discovery[0]
                .file_name()
                .unwrap()
                .to_string_lossy()
                .eq(DEFAULT_INSTRUCTIONS_FILENAME)
        );
    }

    #[tokio::test]
    async fn skills_are_appended_to_project_doc() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(tmp.path().join("AISH.md"), "base doc").unwrap();

        let cfg = make_config(&tmp, 4096, None).await;
        create_skill(
            cfg.codex_home.clone(),
            "pdf-processing",
            "extract from pdfs",
        );

        let skills = load_skills(&cfg);
        let res = get_user_instructions(
            &cfg,
            skills.errors.is_empty().then_some(skills.skills.as_slice()),
        )
        .await
        .expect("instructions expected");
        let expected_path = dunce::canonicalize(
            cfg.codex_home
                .join("skills/pdf-processing/SKILL.md")
                .as_path(),
        )
        .unwrap_or_else(|_| cfg.codex_home.join("skills/pdf-processing/SKILL.md"));
        let expected_path_str = expected_path.to_string_lossy().replace('\\', "/");
        let usage_rules = "- Discovery: Available skills are listed in project docs and may also appear in a runtime \"## Skills\" section (name + description + file path). These are the sources of truth; skill bodies live on disk at the listed paths.\n- Trigger rules: If the user names a skill (with `$SkillName` or plain text) OR the task clearly matches a skill's description, you must use that skill for that turn. Multiple mentions mean use them all. Do not carry skills across turns unless re-mentioned.\n- Missing/blocked: If a named skill isn't in the list or the path can't be read, say so briefly and continue with the best fallback.\n- How to use a skill (progressive disclosure):\n  1) After deciding to use a skill, open its `SKILL.md`. Read only enough to follow the workflow.\n  2) If `SKILL.md` points to extra folders such as `references/`, load only the specific files needed for the request; don't bulk-load everything.\n  3) If `scripts/` exist, prefer running or patching them instead of retyping large code blocks.\n  4) If `assets/` or templates exist, reuse them instead of recreating from scratch.\n- Description as trigger: The YAML `description` in `SKILL.md` is the primary trigger signal; rely on it to decide applicability. If unsure, ask a brief clarification before proceeding.\n- Coordination and sequencing:\n  - If multiple skills apply, choose the minimal set that covers the request and state the order you'll use them.\n  - Announce which skill(s) you're using and why (one short line). If you skip an obvious skill, say why.\n- Context hygiene:\n  - Keep context small: summarize long sections instead of pasting them; only load extra files when needed.\n  - Avoid deeply nested references; prefer one-hop files explicitly linked from `SKILL.md`.\n  - When variants exist (frameworks, providers, domains), pick only the relevant reference file(s) and note that choice.\n- Safety and fallback: If a skill can't be applied cleanly (missing files, unclear instructions), state the issue, pick the next-best approach, and continue.";
        let expected = format!(
            "base doc\n\n## Skills\nThese skills are discovered at startup from multiple local sources. Each entry includes a name, description, and file path so you can open the source for full instructions.\n- pdf-processing: extract from pdfs (file: {expected_path_str})\n{usage_rules}"
        );
        assert_eq!(res, expected);
    }

    #[tokio::test]
    async fn skills_render_without_project_doc() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let cfg = make_config(&tmp, 4096, None).await;
        create_skill(cfg.codex_home.clone(), "linting", "run clippy");

        let skills = load_skills(&cfg);
        let res = get_user_instructions(
            &cfg,
            skills.errors.is_empty().then_some(skills.skills.as_slice()),
        )
        .await
        .expect("instructions expected");
        let expected_path =
            dunce::canonicalize(cfg.codex_home.join("skills/linting/SKILL.md").as_path())
                .unwrap_or_else(|_| cfg.codex_home.join("skills/linting/SKILL.md"));
        let expected_path_str = expected_path.to_string_lossy().replace('\\', "/");
        let usage_rules = "- Discovery: Available skills are listed in project docs and may also appear in a runtime \"## Skills\" section (name + description + file path). These are the sources of truth; skill bodies live on disk at the listed paths.\n- Trigger rules: If the user names a skill (with `$SkillName` or plain text) OR the task clearly matches a skill's description, you must use that skill for that turn. Multiple mentions mean use them all. Do not carry skills across turns unless re-mentioned.\n- Missing/blocked: If a named skill isn't in the list or the path can't be read, say so briefly and continue with the best fallback.\n- How to use a skill (progressive disclosure):\n  1) After deciding to use a skill, open its `SKILL.md`. Read only enough to follow the workflow.\n  2) If `SKILL.md` points to extra folders such as `references/`, load only the specific files needed for the request; don't bulk-load everything.\n  3) If `scripts/` exist, prefer running or patching them instead of retyping large code blocks.\n  4) If `assets/` or templates exist, reuse them instead of recreating from scratch.\n- Description as trigger: The YAML `description` in `SKILL.md` is the primary trigger signal; rely on it to decide applicability. If unsure, ask a brief clarification before proceeding.\n- Coordination and sequencing:\n  - If multiple skills apply, choose the minimal set that covers the request and state the order you'll use them.\n  - Announce which skill(s) you're using and why (one short line). If you skip an obvious skill, say why.\n- Context hygiene:\n  - Keep context small: summarize long sections instead of pasting them; only load extra files when needed.\n  - Avoid deeply nested references; prefer one-hop files explicitly linked from `SKILL.md`.\n  - When variants exist (frameworks, providers, domains), pick only the relevant reference file(s) and note that choice.\n- Safety and fallback: If a skill can't be applied cleanly (missing files, unclear instructions), state the issue, pick the next-best approach, and continue.";
        let expected = format!(
            "## Skills\nThese skills are discovered at startup from multiple local sources. Each entry includes a name, description, and file path so you can open the source for full instructions.\n- linting: run clippy (file: {expected_path_str})\n{usage_rules}"
        );
        assert_eq!(res, expected);
    }

    fn create_skill(codex_home: PathBuf, name: &str, description: &str) {
        let skill_dir = codex_home.join(format!("skills/{name}"));
        fs::create_dir_all(&skill_dir).unwrap();
        let content = format!("---\nname: {name}\ndescription: {description}\n---\n\n# Body\n");
        fs::write(skill_dir.join("SKILL.md"), content).unwrap();
    }
}
