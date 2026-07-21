//! ContextDigest 构建:各分区 token 用量、内容 id 列表、分区哈希、检索 query。

use observability::{ContextDigest, RetrievalQueryRecord};

/// ContextDigest 构建器(随 ASSEMBLE 过程逐步填充)。
#[derive(Debug, Default)]
pub struct DigestBuilder {
    partition_tokens: Vec<(String, usize)>,
    content_ids: Vec<String>,
    partition_hashes: Vec<(String, String)>,
    retrieval_queries: Vec<RetrievalQueryRecord>,
}

impl DigestBuilder {
    /// 记录某分区的 token 用量。
    pub fn record_tokens(&mut self, partition: &str, tokens: usize) {
        self.partition_tokens.push((partition.to_string(), tokens));
    }

    /// 记录内容 id(记忆 id / 知识切片 id / Skill name)。
    pub fn record_content_ids(&mut self, ids: Vec<String>) {
        self.content_ids.extend(ids);
    }

    /// 记录分区哈希。
    pub fn record_hash(&mut self, partition: &str, hash: String) {
        self.partition_hashes.push((partition.to_string(), hash));
    }

    /// 记录检索 query(供 eval 归因)。
    pub fn record_retrieval(&mut self, record: RetrievalQueryRecord) {
        self.retrieval_queries.push(record);
    }

    /// 构建最终 ContextDigest。
    pub fn build(self) -> ContextDigest {
        ContextDigest {
            partition_tokens: self.partition_tokens,
            content_ids: self.content_ids,
            partition_hashes: self.partition_hashes,
            retrieval_queries: self.retrieval_queries,
        }
    }
}

/// 计算文本的简单哈希(用于分区变更检测,非密码学安全)。
pub fn content_hash(text: &str) -> String {
    // FNV-1a 32-bit(简单、快速、足够用于变更检测)
    let mut hash: u32 = 0x811c_9dc5;
    for byte in text.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    format!("{hash:08x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_builder_accumulates() {
        let mut builder = DigestBuilder::default();
        builder.record_tokens("persona", 100);
        builder.record_tokens("history", 5000);
        builder.record_content_ids(vec!["mem_1".to_string(), "mem_2".to_string()]);
        builder.record_hash("persona", content_hash("hello"));

        let digest = builder.build();
        assert_eq!(digest.partition_tokens.len(), 2);
        assert_eq!(digest.content_ids.len(), 2);
        assert_eq!(digest.partition_hashes.len(), 1);
    }

    #[test]
    fn content_hash_deterministic() {
        assert_eq!(content_hash("hello"), content_hash("hello"));
        assert_ne!(content_hash("hello"), content_hash("world"));
    }

    #[test]
    fn content_hash_empty() {
        let h = content_hash("");
        assert_eq!(h.len(), 8); // 8 hex chars
    }
}
