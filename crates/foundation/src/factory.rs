//! 通用实现注册表(factory):把"实现选择是配置项"落成可复用机制。
//!
//! 设计动机(ADR 0011):各模块的 `impl` 配置项(如 vector_store.impl="lancedb"、
//! embedding.impl="openai_compat")决定实例化哪个 provider。为避免每个模块 factory
//! 各写一套 `match impl { ... }` 分支,这里提供一个类型化的 impl 名→构造器 注册表:
//!
//! ```
//! use foundation::factory::Registry;
//! // T = 能力类型(通常是 Box<dyn SomeTrait>);A = 构造参数(通常是模块配置)。
//! let mut reg: Registry<String, &str> = Registry::new("Greeter");
//! reg.register("hello", |name| format!("hello {name}")).unwrap();
//! assert_eq!(reg.create("hello", "world").unwrap(), "hello world");
//! ```
//!
//! 注册表本身不构造任何实例、不认识具体实现的构造细节——组装根(harness/server
//! 启动路径,ADR 0011)负责登记与调用。foundation 只提供这一薄机制。

use std::collections::BTreeMap;

use crate::errors::KairosError;

/// impl 名 → 构造器 的类型化注册表。
///
/// - `T`:被注册的能力类型(如 `Box<dyn VectorStore>`)。
/// - `A`:构造器入参类型(如模块自己的配置结构体引用);无参时用 `()`。
///
/// 构造器为 `Fn(A) -> T`,可多次调用(每次按配置造一个新实例)。
pub struct Registry<T, A = ()> {
    kind: String,
    constructors: BTreeMap<String, Box<dyn Fn(A) -> T + Send + Sync>>,
}

impl<T, A> Registry<T, A> {
    /// 新建注册表。`kind` 为被注册能力的名称(如 "VectorStore"),仅用于错误信息可读性。
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            constructors: BTreeMap::new(),
        }
    }

    /// 登记一个实现。重复登记同名 impl 返回 [`KairosError::Config`](禁止静默覆盖)。
    pub fn register(
        &mut self,
        name: impl Into<String>,
        constructor: impl Fn(A) -> T + Send + Sync + 'static,
    ) -> Result<(), KairosError> {
        let name = name.into();
        if self.constructors.contains_key(&name) {
            return Err(KairosError::config(format!(
                "{} 实现 '{}' 重复注册",
                self.kind, name
            )));
        }
        self.constructors.insert(name, Box::new(constructor));
        Ok(())
    }

    /// 按 impl 名构造实例。未知 impl 返回 [`KairosError::NotConfigured`],附已注册清单指引。
    pub fn create(&self, name: &str, arg: A) -> Result<T, KairosError> {
        match self.constructors.get(name) {
            Some(constructor) => Ok(constructor(arg)),
            None => {
                let available = self.available();
                let list = if available.is_empty() {
                    "(空)".to_string()
                } else {
                    available.join(", ")
                };
                Err(KairosError::not_configured(
                    format!("未知的 {} 实现 '{}'", self.kind, name),
                    Some(format!("已注册实现:{list};检查配置中对应的 impl 值")),
                ))
            }
        }
    }

    /// 返回已注册的 impl 名(BTreeMap 天然有序),供诊断与错误提示。
    pub fn available(&self) -> Vec<String> {
        self.constructors.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_create() {
        let mut reg: Registry<String, &str> = Registry::new("Greeter");
        reg.register("hello", |name| format!("hello {name}"))
            .unwrap();
        assert_eq!(reg.create("hello", "world").unwrap(), "hello world");
    }

    #[test]
    fn duplicate_registration_errors() {
        let mut reg: Registry<i32, ()> = Registry::new("Number");
        reg.register("x", |()| 1).unwrap();
        let err = reg.register("x", |()| 2).unwrap_err();
        assert!(matches!(err, KairosError::Config { .. }));
    }

    #[test]
    fn unknown_impl_errors_with_hint() {
        let mut reg: Registry<i32, ()> = Registry::new("Number");
        reg.register("known", |()| 1).unwrap();
        let err = reg.create("missing", ()).unwrap_err();
        match err {
            KairosError::NotConfigured { hint, .. } => {
                assert!(hint.unwrap().contains("known"));
            }
            _ => panic!("应为 NotConfigured 变体"),
        }
    }

    #[test]
    fn available_is_sorted() {
        let mut reg: Registry<i32, ()> = Registry::new("Number");
        reg.register("b", |()| 2).unwrap();
        reg.register("a", |()| 1).unwrap();
        assert_eq!(reg.available(), vec!["a".to_string(), "b".to_string()]);
    }
}
