use anyhow::Result;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub triggers: Vec<String>,
    pub content: String,
    pub path: PathBuf,
}

#[derive(Default, Clone)]
pub struct SkillRegistry { skills: Vec<Skill> }

impl SkillRegistry {
    pub fn scan(root: &Path) -> Result<Self> {
        let mut out = Vec::new();
        for entry in walkdir(root)? {
            if entry.file_name().and_then(|n| n.to_str()) == Some("SKILL.md") {
                let content = std::fs::read_to_string(&entry)?;
                out.push(parse_skill(&entry, &content));
            }
        }
        Ok(Self { skills: out })
    }

    pub fn list(&self) -> Vec<Skill> { self.skills.clone() }

    pub fn match_for_message(&self, message: &str) -> Vec<Skill> {
        let msg = message.to_lowercase();
        self.skills.iter().filter(|s| s.triggers.iter().any(|t| msg.contains(&t.to_lowercase()))).cloned().collect()
    }

    pub fn inject_prompt(&self, base: &str, message: &str) -> String {
        let matched = self.match_for_message(message);
        if matched.is_empty() { return base.to_string(); }
        let mut out = base.to_string();
        for s in matched { out.push_str(&format!("\n\n[SKILL:{}]\n{}", s.name, s.content)); }
        out
    }
}

fn walkdir(root: &Path) -> Result<Vec<PathBuf>> {
    fn rec(acc: &mut Vec<PathBuf>, dir: &Path) -> Result<()> {
        for e in std::fs::read_dir(dir)? {
            let p = e?.path();
            if p.is_dir() { rec(acc, &p)?; } else { acc.push(p); }
        }
        Ok(())
    }
    let mut v = Vec::new();
    if root.exists() { rec(&mut v, root)?; }
    Ok(v)
}

fn parse_skill(path: &Path, content: &str) -> Skill {
    let mut name = path.parent().and_then(|p| p.file_name()).and_then(|s| s.to_str()).unwrap_or("skill").to_string();
    let mut description = String::new();
    let mut triggers = Vec::new();
    for line in content.lines().take(30) {
        let l = line.trim();
        if let Some(v) = l.strip_prefix("name:") { name = v.trim().to_string(); }
        if let Some(v) = l.strip_prefix("description:") { description = v.trim().to_string(); }
        if let Some(v) = l.strip_prefix("triggers:") { triggers = v.split(',').map(|x| x.trim().to_string()).filter(|x|!x.is_empty()).collect(); }
    }
    Skill { name, description, triggers, content: content.to_string(), path: path.to_path_buf() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scan_and_match() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path().join("s1");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("SKILL.md"), "name: shell\ndescription: run shell\ntriggers: bash,terminal\nUse safe shell").unwrap();
        let reg = SkillRegistry::scan(tmp.path()).unwrap();
        assert_eq!(reg.list().len(), 1);
        assert_eq!(reg.match_for_message("open terminal").len(), 1);
    }
}
