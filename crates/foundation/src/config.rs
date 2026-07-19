//! 配置管理:分层加载机制 + 底座自足配置。
//!
//! 本模块只提供两样东西:
//! 1. **分层加载机制**([`load_settings`]):多来源合并成强类型配置。
//! 2. **底座自足配置**([`KairosSettings`]):当前仅 `log_level`——不依赖任何外部资源、
//!    底座自己就能用的横切配置。
//!
//! **模块业务配置不住这里**(YAGNI + foundation 零业务语义):embedding/rerank/vector_store/
//! memory 等配置各自命名部署环境里的外部资源(哪个模型、哪个端点、哪个 key),其正确值
//! 只有部署方知道,底座无从给出合理默认。这类配置随对应模块落地、归各自 crate;缺失时由
//! 模块 factory fail-closed(见 [`crate::errors::KairosError::NotConfigured`])。
//!
//! 分层合并优先级由高到低:
//!
//! ```text
//! 环境变量  >  .env  >  项目 ./.kairos/config.toml  >  全局 ~/.kairos/config.toml  >  代码默认值
//! ```
//!
//! 用 TOML(ADR 0018,Rust 下为一等公民):支持注释、适合手改。各作用域共用同一强类型
//! 结构——文件只需写要覆盖的字段。密钥永不进配置值,只存环境变量名,运行时按名读取。
//!
//! 详见 docs/foundation/foundation.md。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use toml::Value;

use crate::errors::KairosError;

/// 环境变量前缀与嵌套分隔符(如 `KAIROS_LOG_LEVEL` → log_level)。
const ENV_PREFIX: &str = "KAIROS_";
const ENV_NESTED_DELIMITER: &str = "__";

/// 底座自足配置。
///
/// 只放不依赖部署环境、底座自己就能用的横切配置。目前仅 `log_level`(由 logging 消费)。
/// 模块业务配置随模块落地、归各自 crate,不在此堆积(见模块级文档)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KairosSettings {
    /// 日志级别名(如 "INFO" / "DEBUG"),由 logging 消费。
    pub log_level: String,
}

// 手写单一真相源的 Default:派生会给 log_level 空串 "",与期望的 "INFO" 不符。
impl Default for KairosSettings {
    fn default() -> Self {
        Self {
            log_level: "INFO".to_string(),
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

/// 加载任意强类型配置:分层合并后经 serde 反序列化。优先级由高到低:
/// 环境变量 > .env > 项目 config.toml > 全局 config.toml > 代码默认值。
///
/// 模块把自己的配置结构作为 `T` 传入即可复用该机制;业务字段仍归模块自身,
/// 不得回填到底座配置结构。
///
/// # Errors
/// TOML 解析失败或字段类型非法时返回 [`KairosError::Config`],fail-fast。
pub fn load_settings<T>(opts: &LoadOptions) -> Result<T, KairosError>
where
    T: Default + Serialize + DeserializeOwned,
{
    let merged = merge_layers::<T>(opts)?;
    merged.try_into().map_err(|e: toml::de::Error| {
        KairosError::config("配置校验失败").with_detail("reason", e.to_string())
    })
}

/// 分层合并出 `toml::Value`(未反序列化)。以 `T::default()` 为最底层基底,逐层用更高
/// 优先级来源覆盖。泛型于目标结构:环境变量的类型强制按基底同位置字段的既有类型进行。
fn merge_layers<T>(opts: &LoadOptions) -> Result<Value, KairosError>
where
    T: Default + Serialize,
{
    let mut merged = to_value(&T::default())?;

    // 全局 TOML → 项目 TOML(后者覆盖前者)。
    merge_into(&mut merged, load_toml_file(&opts.user_config_file)?);
    merge_into(&mut merged, load_toml_file(&opts.project_config_file)?);

    // .env → 真实环境变量(后者覆盖前者)。环境变量值天生是字符串,
    // 按默认值基底的既有字段类型做强制转换(见 merge_env_into),避免"像数字的字符串"被误判。
    merge_env_into(&mut merged, env_to_value(&load_dotenv(&opts.env_file)));
    let env_pairs = match &opts.env {
        Some(pairs) => pairs.clone(),
        None => std::env::vars().collect(),
    };
    merge_env_into(&mut merged, env_to_value(&env_pairs));

    Ok(merged)
}

/// 把强类型配置序列化为 `toml::Value`,作为合并基底。
fn to_value<T: Serialize>(settings: &T) -> Result<Value, KairosError> {
    Value::try_from(settings)
        .map_err(|e| KairosError::config("默认配置序列化失败").with_detail("reason", e.to_string()))
}

/// 读取并解析一个 TOML 文件;文件缺失返回空表(不报错,回落默认值)。
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

/// 按路径把标量值插入嵌套表;中间层缺失则创建。值统一以**字符串**写入;真实类型转换
/// 推迟到 [`merge_env_into`]——那里能对照默认值基底的既有类型,避免在不知目标 schema 时盲猜。
fn insert_nested(table: &mut toml::value::Table, path: &[String], value: &str) {
    let Some((head, tail)) = path.split_first() else {
        return;
    };
    if tail.is_empty() {
        table.insert(head.clone(), Value::String(value.to_string()));
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

/// 环境变量专用的深合并:`source` 的标量值均为字符串(见 [`insert_nested`]),
/// 合并时按 `target` 同位置的**既有类型**做强制转换,使 `dim=768` 解析为整数、
/// 而 `model=123` 这类字符串字段原样保留字符串——修掉"像数字的字符串被误判"的陷阱。
///
/// `target` 来自默认值基底([`KairosSettings::default`] 序列化),因此每个已知字段
/// 在此都有正确类型可依据;未知字段(基底不存在)回退为原字符串,交由 serde 兜底报错。
fn merge_env_into(target: &mut Value, source: Value) {
    match (target, source) {
        (Value::Table(t), Value::Table(s)) => {
            for (k, v) in s {
                match t.get_mut(&k) {
                    Some(existing) => merge_env_into(existing, v),
                    None => {
                        t.insert(k, v);
                    }
                }
            }
        }
        // source 恒为字符串标量:按 target 既有类型强制转换。
        (t, Value::String(s)) => *t = coerce_str_to(t, &s),
        (t, s) => *t = s,
    }
}

/// 按 `target` 的既有 TOML 类型把字符串 `s` 强制为对应标量;类型不匹配或无法解析时
/// 保留字符串原样(交由 serde 反序列化按目标字段类型给出精确错误)。
fn coerce_str_to(target: &Value, s: &str) -> Value {
    match target {
        Value::Boolean(_) => s
            .parse::<bool>()
            .map(Value::Boolean)
            .unwrap_or_else(|_| Value::String(s.to_string())),
        Value::Integer(_) => s
            .parse::<i64>()
            .map(Value::Integer)
            .unwrap_or_else(|_| Value::String(s.to_string())),
        Value::Float(_) => s
            .parse::<f64>()
            .map(Value::Float)
            .unwrap_or_else(|_| Value::String(s.to_string())),
        // String 及其他类型:原样保留字符串。
        _ => Value::String(s.to_string()),
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

    // ---- KairosSettings(生产 schema):只验证底座自足配置的行为 ----

    #[test]
    fn default_impl_uses_info_not_empty() {
        // 守护单一真相源:Default 必须给出 "INFO" 而非派生的空串。
        assert_eq!(KairosSettings::default().log_level, "INFO");
    }

    #[test]
    fn settings_defaults_when_no_source() {
        let dir = tmpdir();
        let s: KairosSettings = load_settings(&opts_in(&dir)).unwrap();
        assert_eq!(s.log_level, "INFO");
    }

    #[test]
    fn settings_env_overrides_default() {
        let dir = tmpdir();
        let mut opts = opts_in(&dir);
        opts.env = Some(env_pairs(&[("KAIROS_LOG_LEVEL", "ERROR")]));
        let s: KairosSettings = load_settings(&opts).unwrap();
        assert_eq!(s.log_level, "ERROR");
    }

    // ---- 分层加载机制:用 fixture 结构覆盖嵌套 / 合并 / 类型强制 ----
    //
    // 生产 schema(KairosSettings)当前只有单个标量字段,不足以行使嵌套合并与类型强制。
    // 机制服务的是后续模块配置(model_gateway 等,含 base_url/dim/重试数等嵌套强类型字段),
    // 故此处用一个带嵌套 + 多类型字段的测试专用结构充分验证机制本身,而不为"有东西可测"
    // 在生产 schema 里养字段。

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    #[serde(default)]
    struct FixtureSection {
        name: String,  // 字符串字段:值像数字时不得被误判
        count: u32,    // 整数字段:env 字符串应解析为整数
        ratio: f64,    // 浮点字段
        enabled: bool, // 布尔字段
        endpoint: Option<String>,
    }

    impl Default for FixtureSection {
        fn default() -> Self {
            Self {
                name: "default-name".to_string(),
                count: 10,
                ratio: 0.5,
                enabled: false,
                endpoint: None,
            }
        }
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
    #[serde(default)]
    struct Fixture {
        section: FixtureSection,
        top_level: String,
    }

    /// 用 fixture 走一遍分层合并 + 反序列化(复用生产 merge_layers 机制)。
    fn load_fixture(opts: &LoadOptions) -> Result<Fixture, KairosError> {
        load_settings(opts)
    }

    #[test]
    fn fixture_defaults_when_no_source() {
        let dir = tmpdir();
        let f = load_fixture(&opts_in(&dir)).unwrap();
        assert_eq!(f.section.name, "default-name");
        assert_eq!(f.section.count, 10);
        assert_eq!(f.section.endpoint, None);
    }

    #[test]
    fn toml_overrides_defaults() {
        let dir = tmpdir();
        let proj = write(
            &dir,
            "proj.toml",
            "top_level = \"x\"\n\n[section]\nname = \"custom\"\nendpoint = \"http://localhost:8000/v1\"\n",
        );
        let mut opts = opts_in(&dir);
        opts.project_config_file = proj;
        let f = load_fixture(&opts).unwrap();
        assert_eq!(f.section.name, "custom");
        assert_eq!(
            f.section.endpoint.as_deref(),
            Some("http://localhost:8000/v1")
        );
        assert_eq!(f.section.count, 10); // 未覆盖回落默认
    }

    #[test]
    fn project_overrides_user() {
        let dir = tmpdir();
        let user = write(
            &dir,
            "user.toml",
            "[section]\nname = \"user-name\"\ncount = 1\n",
        );
        let proj = write(&dir, "proj2.toml", "[section]\nname = \"proj-name\"\n");
        let mut opts = opts_in(&dir);
        opts.user_config_file = user;
        opts.project_config_file = proj;
        let f = load_fixture(&opts).unwrap();
        assert_eq!(f.section.name, "proj-name"); // 项目级优先
        assert_eq!(f.section.count, 1); // 项目级未设,回落用户级
    }

    #[test]
    fn env_overrides_file() {
        let dir = tmpdir();
        let proj = write(&dir, "proj3.toml", "[section]\nname = \"file-name\"\n");
        let mut opts = opts_in(&dir);
        opts.project_config_file = proj;
        opts.env = Some(env_pairs(&[("KAIROS_SECTION__NAME", "env-name")]));
        let f = load_fixture(&opts).unwrap();
        assert_eq!(f.section.name, "env-name"); // 环境变量最高优先级
    }

    #[test]
    fn env_nested_snake_mapping() {
        let dir = tmpdir();
        let mut opts = opts_in(&dir);
        opts.env = Some(env_pairs(&[
            ("KAIROS_SECTION__ENDPOINT", "/custom/ep"),
            ("KAIROS_TOP_LEVEL", "tl"),
        ]));
        let f = load_fixture(&opts).unwrap();
        assert_eq!(f.section.endpoint.as_deref(), Some("/custom/ep"));
        assert_eq!(f.top_level, "tl");
    }

    #[test]
    fn env_typed_fields_parsed() {
        // 数字/浮点/布尔字段:env 字符串按基底类型强制解析。
        let dir = tmpdir();
        let mut opts = opts_in(&dir);
        opts.env = Some(env_pairs(&[
            ("KAIROS_SECTION__COUNT", "768"),
            ("KAIROS_SECTION__RATIO", "0.92"),
            ("KAIROS_SECTION__ENABLED", "true"),
        ]));
        let f = load_fixture(&opts).unwrap();
        assert_eq!(f.section.count, 768);
        assert_eq!(f.section.ratio, 0.92);
        assert!(f.section.enabled);
    }

    #[test]
    fn string_field_keeps_numeric_looking_value() {
        // 回归:字符串字段的值恰好像数字/布尔字面量时,不得被误判类型。
        // 类型强制按默认值基底的既有类型进行——String 字段原样保留字符串。
        let dir = tmpdir();
        let mut opts = opts_in(&dir);
        opts.env = Some(env_pairs(&[("KAIROS_SECTION__NAME", "123")]));
        let f = load_fixture(&opts).unwrap();
        assert_eq!(f.section.name, "123"); // 不被转成 Integer 而致反序列化失败
    }

    #[test]
    fn dotenv_loaded_and_overridden_by_real_env() {
        let dir = tmpdir();
        let env_file = write(
            &dir,
            "test.env",
            "KAIROS_SECTION__NAME=\"dotenv-name\"\n# 注释\nIGNORED_NO_EQ\n",
        );
        let mut opts = opts_in(&dir);
        opts.env_file = env_file.clone();
        // .env 生效(引号被剥离)
        let f1 = load_fixture(&opts).unwrap();
        assert_eq!(f1.section.name, "dotenv-name");
        // 真实环境变量覆盖 .env
        opts.env = Some(env_pairs(&[("KAIROS_SECTION__NAME", "real-env")]));
        let f2 = load_fixture(&opts).unwrap();
        assert_eq!(f2.section.name, "real-env");
    }

    #[test]
    fn invalid_toml_errors() {
        let dir = tmpdir();
        let bad = write(&dir, "bad.toml", "this is = = not valid ===");
        let mut opts = opts_in(&dir);
        opts.project_config_file = bad;
        assert!(matches!(
            load_settings::<KairosSettings>(&opts),
            Err(KairosError::Config { .. })
        ));
    }

    #[test]
    fn invalid_field_type_errors() {
        let dir = tmpdir();
        let mut opts = opts_in(&dir);
        opts.env = Some(env_pairs(&[("KAIROS_SECTION__COUNT", "not-a-number")]));
        // "not-a-number" 落为字符串,强制到整数基底失败保留字符串,反序列化到 u32 失败 → Config 错误。
        assert!(matches!(
            load_fixture(&opts),
            Err(KairosError::Config { .. })
        ));
    }
}
