pub(crate) mod analysis;
pub(crate) mod discovery;

impl discovery::RolloutKind {
    pub(crate) fn report_type(&self) -> String {
        match self {
            Self::Root => "root".into(),
            Self::Subagent => "subagent".into(),
            Self::CodeReview => "code_review".into(),
            Self::Compaction => "compaction".into(),
            Self::MemoryConsolidation => "memory_consolidation".into(),
            Self::SecurityReview => "security_review".into(),
            Self::Internal(kind) => format!("internal:{kind}"),
            Self::OtherSubagent(kind) => format!("subagent:{kind}"),
        }
    }
}
