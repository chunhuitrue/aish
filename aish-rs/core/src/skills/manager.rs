use std::path::PathBuf;
use std::sync::RwLock;

use crate::skills::SkillLoadOutcome;
use crate::skills::loader::load_skills_from_roots;
use crate::skills::loader::skill_roots_for_home;
use crate::skills::system::install_system_skills;

pub struct SkillsManager {
    codex_home: PathBuf,
    cache: RwLock<Option<SkillLoadOutcome>>,
}

impl SkillsManager {
    pub fn new(codex_home: PathBuf) -> Self {
        if let Err(err) = install_system_skills(&codex_home) {
            tracing::error!("failed to install system skills: {err}");
        }

        Self {
            codex_home,
            cache: RwLock::new(None),
        }
    }

    pub fn skills(&self) -> SkillLoadOutcome {
        self.skills_with_options(false)
    }

    pub fn skills_with_options(&self, force_reload: bool) -> SkillLoadOutcome {
        let cached = match self.cache.read() {
            Ok(cache) => cache.clone(),
            Err(err) => err.into_inner().clone(),
        };
        if !force_reload && let Some(outcome) = cached {
            return outcome;
        }

        let roots = skill_roots_for_home(&self.codex_home);
        let outcome = load_skills_from_roots(roots);
        match self.cache.write() {
            Ok(mut cache) => {
                *cache = Some(outcome.clone());
            }
            Err(err) => {
                *err.into_inner() = Some(outcome.clone());
            }
        }
        outcome
    }
}
