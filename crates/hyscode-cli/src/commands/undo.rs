//! Comando `hyscode undo` — restaura o arquivo mais recente modificado pelo agente.

use std::path::PathBuf;
use tokio::io::AsyncBufReadExt;

fn undo_log_path() -> PathBuf {
    dirs::data_local_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".local").join("share")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hyscode")
        .join("undo.jsonl")
}

pub async fn run(steps: usize) -> anyhow::Result<()> {
    let path = undo_log_path();

    if !path.exists() {
        println!("Nenhuma operação para desfazer.");
        return Ok(());
    }

    // Lê todas as entradas do log
    let file = tokio::fs::File::open(&path).await?;
    let reader = tokio::io::BufReader::new(file);
    let mut lines_reader = reader.lines();

    let mut entries: Vec<serde_json::Value> = Vec::new();
    while let Some(line) = lines_reader.next_line().await? {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
            entries.push(val);
        }
    }

    if entries.is_empty() {
        println!("Nenhuma operação para desfazer.");
        return Ok(());
    }

    // Pega as últimas N entradas (em ordem reversa = mais recente primeiro)
    let to_undo: Vec<&serde_json::Value> = entries.iter().rev().take(steps).collect();

    let mut kept_entries = entries.clone();

    for entry in &to_undo {
        let original = entry["original"].as_str().unwrap_or_default();
        let backup = entry["backup"].as_str().unwrap_or_default();

        if original.is_empty() || backup.is_empty() {
            eprintln!("Entrada inválida no undo log, ignorando.");
            continue;
        }

        let backup_path = PathBuf::from(backup);
        let original_path = PathBuf::from(original);

        if !backup_path.exists() {
            eprintln!(
                "Backup não encontrado em '{}'. Ignorando desfazer de '{}'.",
                backup, original
            );
            continue;
        }

        // Restaura o backup
        tokio::fs::copy(&backup_path, &original_path).await?;
        println!("Restaurado: '{}' <- '{}'", original, backup);

        // Remove o backup restaurado
        let _ = tokio::fs::remove_file(&backup_path).await;

        // Remove esta entrada do log
        kept_entries.retain(|e| e["backup"].as_str() != Some(backup));
    }

    // Reescreve o undo log sem as entradas desfeitas
    let mut new_content = String::new();
    for e in &kept_entries {
        new_content.push_str(&serde_json::to_string(e).unwrap_or_default());
        new_content.push('\n');
    }
    tokio::fs::write(&path, new_content).await?;

    Ok(())
}
