//! `hades --prune`: removes empty (and optionally stale) sessions without starting the TUI.

use std::io::Write;

use hades_core::format_prune_report;
use hades_storage::{PruneCriteria, SessionRepository, StorageError};

/// Builds prune criteria from CLI flags. Returns `None` when pruning was not requested.
///
/// `--prune-older-than` implies `--prune`; empty sessions are always included.
pub fn criteria_from_args(prune: bool, older_than_days: Option<u32>) -> Option<PruneCriteria> {
    (prune || older_than_days.is_some()).then_some(PruneCriteria {
        empty: true,
        older_than_days,
    })
}

/// Prunes sessions from `repository` and writes a report to `out`.
///
/// The session referenced by the repository's active-session pointer is never removed.
pub async fn run<W: Write>(
    repository: &dyn SessionRepository,
    criteria: PruneCriteria,
    out: &mut W,
) -> Result<usize, StorageError> {
    let removed = repository.prune_sessions(criteria, None).await?;
    let _ = writeln!(
        out,
        "{}",
        format_prune_report(&removed, criteria.older_than_days).trim_end()
    );
    Ok(removed.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hades_storage::{FileSessionRepository, Message};
    use tempfile::tempdir;

    #[test]
    fn test_criteria_from_args() {
        assert_eq!(criteria_from_args(false, None), None);
        assert_eq!(
            criteria_from_args(true, None),
            Some(PruneCriteria::empty_sessions())
        );
        assert_eq!(
            criteria_from_args(false, Some(30)),
            Some(PruneCriteria {
                empty: true,
                older_than_days: Some(30),
            })
        );
    }

    #[tokio::test]
    async fn test_run_prunes_empty_sessions_and_reports() {
        let dir = tempdir().expect("temp dir");
        let repo = FileSessionRepository::with_dir(dir.path());

        let empty = repo.create_session(None, None, None).await.unwrap();
        let mut used = repo.create_session(None, None, None).await.unwrap();
        used.add_message(Message::user(&used.metadata.id, "hello"));
        repo.save_session(&used).await.unwrap();
        // `used` is now the active session; make an empty one active to prove it is kept.
        let active_empty = repo.create_session(None, None, None).await.unwrap();

        let mut out = Vec::new();
        let removed = run(&repo, PruneCriteria::empty_sessions(), &mut out)
            .await
            .expect("prune");

        assert_eq!(removed, 1);
        assert!(repo
            .get_session(&empty.metadata.id)
            .await
            .unwrap()
            .is_none());
        assert!(repo.get_session(&used.metadata.id).await.unwrap().is_some());
        assert!(repo
            .get_session(&active_empty.metadata.id)
            .await
            .unwrap()
            .is_some());
        let report = String::from_utf8(out).unwrap();
        assert!(report.starts_with("Pruned 1 session(s)"));
        assert!(report.contains(&empty.metadata.id));
    }
}
