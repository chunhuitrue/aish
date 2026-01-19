#![cfg(target_os = "macos")]

use std::collections::HashMap;
use std::ffi::CStr;
use std::path::Path;
use std::path::PathBuf;
use tokio::process::Child;

use crate::protocol::SandboxPolicy;
use crate::spawn::AISH_SANDBOX_ENV_VAR;
use crate::spawn::StdioPolicy;
use crate::spawn::spawn_child_async;

const MACOS_SEATBELT_BASE_POLICY: &str = include_str!("seatbelt_base_policy.sbpl");
const MACOS_SEATBELT_NETWORK_POLICY: &str = include_str!("seatbelt_network_policy.sbpl");

/// When working with `sandbox-exec`, only consider `sandbox-exec` in `/usr/bin`
/// to defend against an attacker trying to inject a malicious version on the
/// PATH. If /usr/bin/sandbox-exec has been tampered with, then the attacker
/// already has root access.
pub(crate) const MACOS_PATH_TO_SEATBELT_EXECUTABLE: &str = "/usr/bin/sandbox-exec";

pub async fn spawn_command_under_seatbelt(
    command: Vec<String>,
    command_cwd: PathBuf,
    sandbox_policy: &SandboxPolicy,
    sandbox_policy_cwd: &Path,
    stdio_policy: StdioPolicy,
    mut env: HashMap<String, String>,
) -> std::io::Result<Child> {
    let args = create_seatbelt_command_args(command, sandbox_policy, sandbox_policy_cwd);
    let arg0 = None;
    env.insert(AISH_SANDBOX_ENV_VAR.to_string(), "seatbelt".to_string());
    spawn_child_async(
        PathBuf::from(MACOS_PATH_TO_SEATBELT_EXECUTABLE),
        args,
        arg0,
        command_cwd,
        sandbox_policy,
        stdio_policy,
        env,
    )
    .await
}

pub(crate) fn create_seatbelt_command_args(
    command: Vec<String>,
    sandbox_policy: &SandboxPolicy,
    sandbox_policy_cwd: &Path,
) -> Vec<String> {
    let (file_write_policy, file_write_dir_params) = {
        if sandbox_policy.has_full_disk_write_access() {
            // Allegedly, this is more permissive than `(allow file-write*)`.
            (
                r#"(allow file-write* (regex #"^/"))"#.to_string(),
                Vec::new(),
            )
        } else {
            let writable_roots = sandbox_policy.get_writable_roots_with_cwd(sandbox_policy_cwd);

            let mut writable_folder_policies: Vec<String> = Vec::new();
            let mut file_write_params = Vec::new();

            for (index, wr) in writable_roots.iter().enumerate() {
                // Canonicalize to avoid mismatches like /var vs /private/var on macOS.
                let canonical_root = wr
                    .root
                    .as_path()
                    .canonicalize()
                    .unwrap_or_else(|_| wr.root.to_path_buf());
                let root_param = format!("WRITABLE_ROOT_{index}");
                file_write_params.push((root_param.clone(), canonical_root));

                if wr.read_only_subpaths.is_empty() {
                    writable_folder_policies.push(format!("(subpath (param \"{root_param}\"))"));
                } else {
                    // Add parameters for each read-only subpath and generate
                    // the `(require-not ...)` clauses.
                    let mut require_parts: Vec<String> = Vec::new();
                    require_parts.push(format!("(subpath (param \"{root_param}\"))"));
                    for (subpath_index, ro) in wr.read_only_subpaths.iter().enumerate() {
                        let canonical_ro = ro
                            .as_path()
                            .canonicalize()
                            .unwrap_or_else(|_| ro.to_path_buf());
                        let ro_param = format!("WRITABLE_ROOT_{index}_RO_{subpath_index}");
                        require_parts
                            .push(format!("(require-not (subpath (param \"{ro_param}\")))"));
                        file_write_params.push((ro_param, canonical_ro));
                    }
                    let policy_component = format!("(require-all {} )", require_parts.join(" "));
                    writable_folder_policies.push(policy_component);
                }
            }

            if writable_folder_policies.is_empty() {
                ("".to_string(), Vec::new())
            } else {
                let file_write_policy = format!(
                    "(allow file-write*\n{}\n)",
                    writable_folder_policies.join(" ")
                );
                (file_write_policy, file_write_params)
            }
        }
    };

    let file_read_policy = if sandbox_policy.has_full_disk_read_access() {
        "; allow read-only file operations\n(allow file-read*)"
    } else {
        ""
    };

    // TODO(mbolin): apply_patch calls must also honor the SandboxPolicy.
    let network_policy = if sandbox_policy.has_full_network_access() {
        MACOS_SEATBELT_NETWORK_POLICY
    } else {
        ""
    };

    let full_policy = format!(
        "{MACOS_SEATBELT_BASE_POLICY}\n{file_read_policy}\n{file_write_policy}\n{network_policy}"
    );

    let dir_params = [file_write_dir_params, macos_dir_params()].concat();

    let mut seatbelt_args: Vec<String> = vec!["-p".to_string(), full_policy];
    let definition_args = dir_params
        .into_iter()
        .map(|(key, value)| format!("-D{key}={value}", value = value.to_string_lossy()));
    seatbelt_args.extend(definition_args);
    seatbelt_args.push("--".to_string());
    seatbelt_args.extend(command);
    seatbelt_args
}

/// Wraps libc::confstr to return a String.
fn confstr(name: libc::c_int) -> Option<String> {
    let mut buf = vec![0_i8; (libc::PATH_MAX as usize) + 1];
    let len = unsafe { libc::confstr(name, buf.as_mut_ptr(), buf.len()) };
    if len == 0 {
        return None;
    }
    // confstr guarantees NUL-termination when len > 0.
    let cstr = unsafe { CStr::from_ptr(buf.as_ptr()) };
    cstr.to_str().ok().map(ToString::to_string)
}

/// Wraps confstr to return a canonicalized PathBuf.
fn confstr_path(name: libc::c_int) -> Option<PathBuf> {
    let s = confstr(name)?;
    let path = PathBuf::from(s);
    path.canonicalize().ok().or(Some(path))
}

fn macos_dir_params() -> Vec<(String, PathBuf)> {
    if let Some(p) = confstr_path(libc::_CS_DARWIN_USER_CACHE_DIR) {
        return vec![("DARWIN_USER_CACHE_DIR".to_string(), p)];
    }
    vec![]
}

#[cfg(test)]
mod tests {
    use super::MACOS_SEATBELT_BASE_POLICY;
    use super::create_seatbelt_command_args;
    use super::macos_dir_params;
    use crate::protocol::SandboxPolicy;
    use crate::seatbelt::MACOS_PATH_TO_SEATBELT_EXECUTABLE;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::process::Command;
    use tempfile::TempDir;

    #[test]
    fn create_seatbelt_args_with_read_only_git_and_codex_subpaths() {
        // Create a temporary workspace with .git and .aish directories.
        let tmp = TempDir::new().expect("tempdir");
        let PopulatedTmp {
            vulnerable_root_canonical,
            dot_git_canonical,
            dot_codex_canonical,
            ..
        } = populate_tmpdir(tmp.path());
        let cwd = vulnerable_root_canonical;

        // CurrentDirWrite automatically includes cwd as writable.
        let policy = SandboxPolicy::CurrentDirWrite {
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        // Create the Seatbelt command to wrap a shell command that tries to
        // write to .aish/config.toml in the cwd.
        let shell_command: Vec<String> = [
            "bash",
            "-c",
            "echo 'sandbox_mode = \"danger-full-access\"' > \"$1\"",
            "bash",
            dot_codex_canonical
                .join("config.toml")
                .to_string_lossy()
                .as_ref(),
        ]
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
        let args = create_seatbelt_command_args(shell_command.clone(), &policy, &cwd);

        // Build the expected policy text.
        let cwd_buf = cwd.clone();
        let expected_policy = format!(
            r#"{MACOS_SEATBELT_BASE_POLICY}
; allow read-only file operations
(allow file-read*)
(allow file-write*
(require-all (subpath (param "WRITABLE_ROOT_0")) (require-not (subpath (param "WRITABLE_ROOT_0_RO_0"))) (require-not (subpath (param "WRITABLE_ROOT_0_RO_1"))) )
)
"#,
        );

        let mut expected_args = vec![
            "-p".to_string(),
            expected_policy,
            format!("-DWRITABLE_ROOT_0={}", cwd_buf.to_string_lossy()),
            format!(
                "-DWRITABLE_ROOT_0_RO_0={}",
                dot_git_canonical.to_string_lossy()
            ),
            format!(
                "-DWRITABLE_ROOT_0_RO_1={}",
                dot_codex_canonical.to_string_lossy()
            ),
        ];

        expected_args.extend(
            macos_dir_params()
                .into_iter()
                .map(|(key, value)| format!("-D{key}={value}", value = value.to_string_lossy())),
        );

        expected_args.push("--".to_string());
        expected_args.extend(shell_command);

        assert_eq!(expected_args, args);

        // Verify that .aish/config.toml cannot be modified under the generated
        // Seatbelt policy.
        let config_toml = dot_codex_canonical.join("config.toml");
        let output = Command::new(MACOS_PATH_TO_SEATBELT_EXECUTABLE)
            .args(&args)
            .current_dir(&cwd)
            .output()
            .expect("execute seatbelt command");
        assert_eq!(
            "sandbox_mode = \"read-only\"\n",
            String::from_utf8_lossy(&fs::read(&config_toml).expect("read config.toml")),
            "config.toml should contain its original contents because it should not have been modified"
        );
        assert!(
            !output.status.success(),
            "command to write {} should fail under seatbelt",
            &config_toml.display()
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            format!("bash: {}: Operation not permitted\n", config_toml.display()),
        );
    }

    #[test]
    fn create_seatbelt_args_for_cwd_as_git_repo() {
        // Create a temporary workspace with .git and .aish directories.
        let tmp = TempDir::new().expect("tempdir");
        let PopulatedTmp {
            vulnerable_root,
            vulnerable_root_canonical,
            dot_git_canonical,
            dot_codex_canonical,
            ..
        } = populate_tmpdir(tmp.path());

        // CurrentDirWrite automatically includes cwd as writable.
        let policy = SandboxPolicy::CurrentDirWrite {
            network_access: false,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        };

        let shell_command: Vec<String> = [
            "bash",
            "-c",
            "echo 'sandbox_mode = \"danger-full-access\"' > \"$1\"",
            "bash",
            dot_codex_canonical
                .join("config.toml")
                .to_string_lossy()
                .as_ref(),
        ]
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
        let args =
            create_seatbelt_command_args(shell_command.clone(), &policy, vulnerable_root.as_path());

        let tmpdir_env_var = std::env::var("TMPDIR")
            .ok()
            .map(PathBuf::from)
            .and_then(|p| p.canonicalize().ok())
            .map(|p| p.to_string_lossy().to_string());

        let tempdir_policy_entry = if tmpdir_env_var.is_some() {
            r#" (subpath (param "WRITABLE_ROOT_2"))"#
        } else {
            ""
        };

        // Build the expected policy text.
        let expected_policy = format!(
            r#"{MACOS_SEATBELT_BASE_POLICY}
; allow read-only file operations
(allow file-read*)
(allow file-write*
(require-all (subpath (param "WRITABLE_ROOT_0")) (require-not (subpath (param "WRITABLE_ROOT_0_RO_0"))) (require-not (subpath (param "WRITABLE_ROOT_0_RO_1"))) ) (subpath (param "WRITABLE_ROOT_1")){tempdir_policy_entry}
)
"#,
        );

        let mut expected_args = vec![
            "-p".to_string(),
            expected_policy,
            format!(
                "-DWRITABLE_ROOT_0={}",
                vulnerable_root_canonical.to_string_lossy()
            ),
            format!(
                "-DWRITABLE_ROOT_0_RO_0={}",
                dot_git_canonical.to_string_lossy()
            ),
            format!(
                "-DWRITABLE_ROOT_0_RO_1={}",
                dot_codex_canonical.to_string_lossy()
            ),
            format!(
                "-DWRITABLE_ROOT_1={}",
                PathBuf::from("/tmp")
                    .canonicalize()
                    .expect("canonicalize /tmp")
                    .to_string_lossy()
            ),
        ];

        if let Some(p) = tmpdir_env_var {
            expected_args.push(format!("-DWRITABLE_ROOT_2={p}"));
        }

        expected_args.extend(
            macos_dir_params()
                .into_iter()
                .map(|(key, value)| format!("-D{key}={value}", value = value.to_string_lossy())),
        );

        expected_args.push("--".to_string());
        expected_args.extend(shell_command);

        assert_eq!(expected_args, args);
    }

    struct PopulatedTmp {
        /// Path containing a .git and .aish subfolder.
        /// For the purposes of this test, we consider this a "vulnerable" root
        /// because a bad actor could write to .git/hooks/pre-commit so an
        /// unsuspecting user would run code as privileged the next time they
        /// ran `git commit` themselves, or modified .aish/config.toml to
        /// contain `sandbox_mode = "danger-full-access"` so the agent would
        /// have full privileges the next time it ran in that repo.
        vulnerable_root: PathBuf,
        vulnerable_root_canonical: PathBuf,
        dot_git_canonical: PathBuf,
        dot_codex_canonical: PathBuf,
    }

    fn populate_tmpdir(tmp: &Path) -> PopulatedTmp {
        let vulnerable_root = tmp.join("vulnerable_root");
        fs::create_dir_all(&vulnerable_root).expect("create vulnerable_root");

        // TODO(mbolin): Should also support the case where `.git` is a file
        // with a gitdir: ... line.
        Command::new("git")
            .arg("init")
            .arg(".")
            .current_dir(&vulnerable_root)
            .output()
            .expect("git init .");

        fs::create_dir_all(vulnerable_root.join(".aish")).expect("create .aish");
        fs::write(
            vulnerable_root.join(".aish").join("config.toml"),
            "sandbox_mode = \"read-only\"\n",
        )
        .expect("write .aish/config.toml");

        // Ensure we have canonical paths for -D parameter matching.
        let vulnerable_root_canonical = vulnerable_root
            .canonicalize()
            .expect("canonicalize vulnerable_root");
        let dot_git_canonical = vulnerable_root_canonical.join(".git");
        let dot_codex_canonical = vulnerable_root_canonical.join(".aish");
        PopulatedTmp {
            vulnerable_root,
            vulnerable_root_canonical,
            dot_git_canonical,
            dot_codex_canonical,
        }
    }
}
