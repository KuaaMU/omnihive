use crate::engine;
use crate::models::*;
use std::path::PathBuf;
use tauri::command;

#[command]
pub fn read_consensus(project_dir: String) -> Result<ConsensusState, String> {
    let path = PathBuf::from(&project_dir);
    engine::memory::read_consensus(&path)
}

#[command]
pub fn update_consensus(project_dir: String, content: String) -> Result<bool, String> {
    let path = PathBuf::from(&project_dir);
    engine::memory::update_consensus(&path, &content)?;
    Ok(true)
}

#[command]
pub fn backup_consensus(project_dir: String) -> Result<String, String> {
    let path = PathBuf::from(&project_dir);
    engine::memory::backup_consensus(&path)
}
