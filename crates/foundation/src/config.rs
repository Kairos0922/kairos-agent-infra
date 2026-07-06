//! 配置管理:单一配置入口,实现选择全部走配置。
//!
//! 基于 `serde` + `toml`:配置反序列化到强类型结构体(带 `#[serde(default)]` 默认值)。
//! 多来源分层合并,优先级由高到低:
//!
//! ```text
//! 环境变量  >  .env  >  项目 ./.kairos/config.toml  >  全局 ~/.kairos/config.toml  >  代码默认值
//! ```
//!
//! 沿用 TOML(ADR 0018,Rust 下为一等公民):支持注释、适合手改。各作用域共用同一
//! [`KairosSettings`] 结构——文件只需写要覆盖的字段,字段天然一致;项目级文件覆盖全局级文件。
//!
//! 约定:
//! - 实现选择是配置项(`impl` 字段),不是代码分支;由各模块 factory 读取决定实例化哪个实现。
//! - 密钥永不进配置值,只存环境变量名(`api_key_env`),运行时按名读取。
//! - embedding 维度必须与向量列维度一致,启动时校验,不一致 fail-fast(由消费方执行)。
//!
//! 详见 docs/foundation/foundation.md。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use toml::Value;

use crate::errors::KairosError;

/// 环境变量前缀与嵌套分隔符(如 `KAIROS_VECTOR_STORE__URI` 映射到 vector_store.uri)。
const ENV_PREFIX: &str = "KAIROS_";
const ENV_NESTED_DELIMITER: &str = "__";

/// 向量库配置。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VectorStoreConfig {
    pub r#impl: String,
    pub uri: String,
    /// LRU 索引缓存上限(字节),防止 optimize() 累积的 reader FD 泄漏到 EMFILE(借鉴 EverOS 实测)。
    pub index_cache_size_bytes: u64,
}

impl Default for VectorStoreConfig {
    fn default() -> Self {
        Self {
            r#impl: "lancedb".to_string(),
            uri: "./.kairos/lancedb".to_string(),
            index_cache_size_bytes: 16 * 1024 * 1024,
        }
    }
}

/// embedding 模型配置。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EmbeddingConfig {
    /// openai_compat | sentence_transformer
    pub r#impl: String,
    pub model: String,
    /// 必须与向量列维度一致,启动校验。
    pub dim: u32,
    /// 本地 vLLM/Ollama 也走这里;None 用 provider 默认端点。
    pub base_url: Option<String>,
    /// 只存环境变量名,不存密钥本身。
    pub api_key_env: String,
    pub batch_size: u32,
    pub max_concurrent: u32,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            r#impl: "openai_compat".to_string(),
            model: "BAAI/bge-m3".to_string(),
            dim: 1024,
            base_url: None,
            api_key_env: "KAIROS_EMBED_API_KEY".to_string(),
            batch_size: 32,
            max_concurrent: 8,
        }
    }
}

/// rerank 模型配置。默认关闭,按需开启。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RerankConfig {
    pub enabled: bool,
    /// cross_encoder | http_rerank
    pub r#impl: String,
    pub model: String,
    pub base_url: Option<String>,
    pub api_key_env: String,
}

impl Default for RerankConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            r#impl: "cross_encoder".to_string(),
            model: "BAAI/bge-reranker-v2-m3".to_string(),
            base_url: None,
            api_key_env: "KAIROS_RERANK_API_KEY".to_string(),
        }
    }
}

/// 记忆模块行为配置。
///
/// 字段对齐 ADR 0004-0009 与 docs/modules/memory/memory-types.md;procedural 的 trace
/// 提炼/评估门控在模块外(harness/distill,ADR 0008),其参数不在此。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryConfig {
    /// 写入冲突去重:LLM 驱动 ADD/UPDATE/DELETE 前,向量检索 top-K 候选的相似度阈值(ADR 0004/0005)。
    pub dedup_threshold: f32,
    /// episodic 显著性门控:低于此值的内容不写入(ADR 0006)。
    pub episodic_salience_threshold: f32,
    /// episodic 归档窗:超过此天数且久未命中的情景记忆批量归档(非硬删,ADR 0005/0006)。
    pub episodic_archive_after_days: u32,
    /// procedural 低效淘汰:effectiveness 长期低于此阈值的经验标记 deprecated(ADR 0005)。
    pub procedural_effectiveness_floor: f32,
    /// 选择性召回:是否默认启用 RecallRouter 门控(ADR 0007;默认关,由 harness 显式开)。
    pub recall_router_enabled: bool,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            dedup_threshold: 0.92,
            episodic_salience_threshold: 0.5,
            episodic_archive_after_days: 30,
            procedural_effectiveness_floor: 0.2,
            recall_router_enabled: false,
        }
    }
}

/// Kairos 全局配置。
///
/// 记忆相关配置目前直接挂在顶层。未来出现第二个模块、配置确有交叉时,再决定是否按模块
/// 分组,不提前(YAGNI)。模型(ChatModel)配置由 model_gateway 任务落地。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KairosSettings {
    pub vector_store: VectorStoreConfig,
    pub embedding: EmbeddingConfig,
    pub rerank: RerankConfig,
    pub memory: MemoryConfig,
    pub log_level: String,
    pub trace_enabled: bool,
}

// 不派生 Default:派生会对 log_level 给出空串 ""，与期望的 "INFO" 不符。手写单一真相源,
// 避免"派生 Default 返回空串、加载路径另用一份默认"的双份默认陷阱。
impl Default for KairosSettings {
    fn default() -> Self {
        Self {
            vector_store: VectorStoreConfig::default(),
            embedding: EmbeddingConfig::default(),
            rerank: RerankConfig::default(),
            memory: MemoryConfig::default(),
            log_level: "INFO".to_string(),
            trace_enabled: false,
        }
    }
}

/// `load_settings` 的来源覆写,仅供测试注入;生产用 [`LoadOptions::default`](默认路径 + 进程环境)。
pub struct LoadOptions {
    /// 环境变量来源(键值对)。None 时读进程 `std::env::vars`。
    pub env: Option<Vec<(String, String)>>,
    pub project_config_file: PathBuf,
    pub user_config_file: PathBuf,
    pub env_file: PathBuf,
}

impl Default for LoadOptions {
    fn default() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        Self {
            env: None,
            project_config_file: PathBuf::from(".kairos/config.toml"),
            user_config_file: home.join(".kairos/config.toml"),
            env_file: PathBuf::from(".env"),
        }
    }
}

/// 加载配置:分层合并后经 serde 反序列化。优先级由高到低:
/// 环境变量 > .env > 项目 config.toml > 全局 config.toml > 代码默认值。
///
/// # Errors
/// TOML 解析失败或字段类型非法时返回 [`KairosError::Config`],fail-fast。
pub fn load_settings(opts: &LoadOptions) -> Result<KairosSettings, KairosError> {
    // 以顶层默认值作为最底层,逐层用更高优先级来源覆盖。
    let mut merged = to_value(&KairosSettings::default())?;

    // 全局 TOML → 项目 TOML(后者覆盖前者)。
    merge_into(&mut merged, load_toml_file(&opts.user_config_file)?);
    merge_into(&mut merged, load_toml_file(&opts.project_config_file)?);

    // .env → 真实环境变量(后者覆盖前者)。
    merge_into(&mut merged, env_to_value(&load_dotenv(&opts.env_file)));
    let env_pairs = match &opts.env {
        Some(pairs) => pairs.clone(),
        None => std::env::vars().collect(),
    };
    merge_into(&mut merged, env_to_value(&env_pairs));

    merged.try_into().map_err(|e: toml::de::Error| {
        KairosError::config("配置校验失败").with_detail("reason", e.to_string())
    })
}

/// 把强类型配置序列化为 `toml::Value`,作为合并基底。
fn to_value(settings: &KairosSettings) -> Result<Value, KairosError> {
    Value::try_from(settings)
        .map_err(|e| KairosError::config("默认配置序列化失败").with_detail("reason", e.to_string()))
}

/// 读取并解析一个 TOML 文件;文件缺失返回 None 对应的空表(不报错,回落默认值)。
fn load_toml_file(path: &Path) -> Result<Value, KairosError> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        // 文件不存在或不可读:视为无覆盖,交由更低优先级来源兜底。
        Err(_) => return Ok(Value::Table(Default::default())),
    };
    // toml 1.x:Value 的 FromStr 只解析单个值,文档要走 from_str 到 Table。
    let table: toml::Table = toml::from_str(&text).map_err(|e| {
        KairosError::config(format!("配置文件解析失败:{}", path.display()))
            .with_detail("path", path.display().to_string())
            .with_detail("reason", e.to_string())
    })?;
    Ok(Value::Table(table))
}

/// 解析 .env 文件为 KEY→VALUE 列表;缺失返回空。仅支持 `KEY=VALUE` 行,忽略注释与空行。
fn load_dotenv(path: &Path) -> Vec<(String, String)> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, val)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_string();
        let mut val = val.trim();
        // 去掉包裹的成对引号(单/双)。
        if val.len() >= 2 {
            let bytes = val.as_bytes();
            let (first, last) = (bytes[0], bytes[bytes.len() - 1]);
            if (first == b'"' || first == b'\'') && first == last {
                val = &val[1..val.len() - 1];
            }
        }
        out.push((key, val.to_string()));
    }
    out
}

/// 把 `KAIROS_` 前缀的扁平键值对转成嵌套 `toml::Value` 表。
/// 键格式:`KAIROS_<SECTION>__<FIELD>`,段用 `__` 分隔,各段转小写(snake_case)。
/// 无 `__` 的键作为顶层字段(如 `KAIROS_LOG_LEVEL` → log_level)。
fn env_to_value(env: &[(String, String)]) -> Value {
    let mut root = toml::value::Table::new();
    for (key, value) in env {
        let Some(rest) = key.strip_prefix(ENV_PREFIX) else {
            continue;
        };
        let path: Vec<String> = rest
            .split(ENV_NESTED_DELIMITER)
            .map(|s| s.to_lowercase())
            .collect();
        insert_nested(&mut root, &path, value);
    }
    Value::Table(root)
}

/// 按路径把标量值插入嵌套表;中间层缺失则创建。值统一以字符串写入,由 serde 反序列化时按目标类型解析。
fn insert_nested(table: &mut toml::value::Table, path: &[String], value: &str) {
    let Some((head, tail)) = path.split_first() else {
        return;
    };
    if tail.is_empty() {
        table.insert(head.clone(), parse_scalar(value));
        return;
    }
    let entry = table
        .entry(head.clone())
        .or_insert_with(|| Value::Table(toml::value::Table::new()));
    if let Value::Table(inner) = entry {
        insert_nested(inner, tail, value);
    } else {
        // 已存在非表值:覆盖为表以容纳更深路径。
        let mut inner = toml::value::Table::new();
        insert_nested(&mut inner, tail, value);
        *entry = Value::Table(inner);
    }
}

/// 把环境变量字符串解析为合适的 TOML 标量(bool / integer / float / string),
/// 使 serde 能反序列化到强类型字段(如 dim: u32)。
fn parse_scalar(s: &str) -> Value {
    if let Ok(b) = s.parse::<bool>() {
        return Value::Boolean(b);
    }
    if let Ok(i) = s.parse::<i64>() {
        return Value::Integer(i);
    }
    if let Ok(f) = s.parse::<f64>() {
        return Value::Float(f);
    }
    Value::String(s.to_string())
}

/// 深合并:`source` 覆盖 `target`,同为表则递归合并,否则直接覆盖。
fn merge_into(target: &mut Value, source: Value) {
    match (target, source) {
        (Value::Table(t), Value::Table(s)) => {
            for (k, v) in s {
                match t.get_mut(&k) {
                    Some(existing) => merge_into(existing, v),
                    None => {
                        t.insert(k, v);
                    }
                }
            }
        }
        (t, s) => *t = s,
    }
}

/// 便于测试:把 `[(k, v)]` 收成 env 覆写。生产不必用。
pub fn env_pairs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect::<Vec<_>>()
        .into_iter()
        .collect::<BTreeMap<_, _>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// 指向临时目录中不存在的文件,确保默认不读到真实机器配置。
    fn opts_in(dir: &Path) -> LoadOptions {
        LoadOptions {
            env: Some(Vec::new()),
            project_config_file: dir.join("nope-project.toml"),
            user_config_file: dir.join("nope-user.toml"),
            env_file: dir.join("nope.env"),
        }
    }

    fn write(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    fn tmpdir() -> PathBuf {
        let base = std::env::temp_dir().join(format!("kairos-cfg-{}", std::process::id()));
        // 每个测试用唯一子目录,避免并发串扰。
        let unique = base.join(format!("{:p}", &base));
        std::fs::create_dir_all(&unique).unwrap();
        unique
    }

    #[test]
    fn default_impl_uses_info_not_empty() {
        // 守护单一真相源:Default 必须给出 "INFO" 而非派生的空串。
        assert_eq!(KairosSettings::default().log_level, "INFO");
    }

    #[test]
    fn defaults_when_no_file() {
        let dir = tmpdir();
        let s = load_settings(&opts_in(&dir)).unwrap();
        assert_eq!(s.log_level, "INFO");
        assert_eq!(s.embedding.model, "BAAI/bge-m3");
        assert_eq!(s.embedding.dim, 1024);
        assert_eq!(s.vector_store.r#impl, "lancedb");
    }

    #[test]
    fn toml_overrides_defaults() {
        let dir = tmpdir();
        let proj = write(
            &dir,
            "proj.toml",
            "log_level = \"DEBUG\"\n\n[embedding]\nmodel = \"custom-model\"\nbase_url = \"http://localhost:8000/v1\"\n",
        );
        let mut opts = opts_in(&dir);
        opts.project_config_file = proj;
        let s = load_settings(&opts).unwrap();
        assert_eq!(s.log_level, "DEBUG");
        assert_eq!(s.embedding.model, "custom-model");
        assert_eq!(
            s.embedding.base_url.as_deref(),
            Some("http://localhost:8000/v1")
        );
        assert_eq!(s.embedding.dim, 1024); // 未覆盖回落默认
    }

    #[test]
    fn project_overrides_user() {
        let dir = tmpdir();
        let user = write(
            &dir,
            "user.toml",
            "log_level = \"WARNING\"\n\n[embedding]\nmodel = \"user-model\"\n",
        );
        let proj = write(&dir, "proj2.toml", "[embedding]\nmodel = \"proj-model\"\n");
        let mut opts = opts_in(&dir);
        opts.user_config_file = user;
        opts.project_config_file = proj;
        let s = load_settings(&opts).unwrap();
        assert_eq!(s.embedding.model, "proj-model"); // 项目级优先
        assert_eq!(s.log_level, "WARNING"); // 项目级未设,回落用户级
    }

    #[test]
    fn env_overrides_file() {
        let dir = tmpdir();
        let proj = write(&dir, "proj3.toml", "[embedding]\nmodel = \"file-model\"\n");
        let mut opts = opts_in(&dir);
        opts.project_config_file = proj;
        opts.env = Some(env_pairs(&[("KAIROS_EMBEDDING__MODEL", "env-model")]));
        let s = load_settings(&opts).unwrap();
        assert_eq!(s.embedding.model, "env-model"); // 环境变量最高优先级
    }

    #[test]
    fn env_nested_snake_mapping() {
        let dir = tmpdir();
        let mut opts = opts_in(&dir);
        opts.env = Some(env_pairs(&[
            ("KAIROS_VECTOR_STORE__URI", "/custom/lancedb"),
            ("KAIROS_LOG_LEVEL", "ERROR"),
        ]));
        let s = load_settings(&opts).unwrap();
        assert_eq!(s.vector_store.uri, "/custom/lancedb");
        assert_eq!(s.log_level, "ERROR");
    }

    #[test]
    fn env_typed_field_parsed() {
        let dir = tmpdir();
        let mut opts = opts_in(&dir);
        opts.env = Some(env_pairs(&[("KAIROS_EMBEDDING__DIM", "768")]));
        let s = load_settings(&opts).unwrap();
        assert_eq!(s.embedding.dim, 768); // 字符串 "768" 解析为整数并反序列化到 u32
    }

    #[test]
    fn dotenv_loaded_and_overridden_by_real_env() {
        let dir = tmpdir();
        let env_file = write(
            &dir,
            "test.env",
            "KAIROS_EMBEDDING__MODEL=\"dotenv-model\"\n# 注释\nIGNORED_NO_EQ\n",
        );
        let mut opts = opts_in(&dir);
        opts.env_file = env_file.clone();
        // .env 生效(引号被剥离)
        let s1 = load_settings(&opts).unwrap();
        assert_eq!(s1.embedding.model, "dotenv-model");
        // 真实环境变量覆盖 .env
        opts.env = Some(env_pairs(&[("KAIROS_EMBEDDING__MODEL", "real-env")]));
        let s2 = load_settings(&opts).unwrap();
        assert_eq!(s2.embedding.model, "real-env");
    }

    #[test]
    fn invalid_toml_errors() {
        let dir = tmpdir();
        let bad = write(&dir, "bad.toml", "this is = = not valid ===");
        let mut opts = opts_in(&dir);
        opts.project_config_file = bad;
        assert!(matches!(
            load_settings(&opts),
            Err(KairosError::Config { .. })
        ));
    }

    #[test]
    fn invalid_field_type_errors() {
        let dir = tmpdir();
        let mut opts = opts_in(&dir);
        opts.env = Some(env_pairs(&[("KAIROS_EMBEDDING__DIM", "not-a-number")]));
        // "not-a-number" 落为字符串,反序列化到 u32 失败 → Config 错误。
        assert!(matches!(
            load_settings(&opts),
            Err(KairosError::Config { .. })
        ));
    }
}
