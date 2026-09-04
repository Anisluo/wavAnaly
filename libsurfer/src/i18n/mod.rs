//! 极简的界面文案本地化。
//!
//! 用法：`t!("Scopes")`。参数是英文原文（同时也是查找键）；当前语言为中文且
//! 词典里有对应条目时返回中文，否则原样返回英文。这样即使漏翻也不会出错。
//!
//! 词典按界面区域拆成多个文件（`zh_*.rs`），每个文件导出一个
//! `ENTRIES: &[(&str, &str)]`，在 [`dictionary`] 里合并。
//!
//! 语言选择优先级：环境变量 `WAVANALY_LANG`（`zh` / `en`）> 配置文件 `language` > 默认中文。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::LazyLock;

mod zh_menus;
mod zh_misc;
mod zh_panels;
mod zh_help;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Language {
    English = 0,
    Chinese = 1,
}

static LANG: AtomicU8 = AtomicU8::new(Language::Chinese as u8);

/// 设置当前语言（启动时由配置调用一次）。
pub fn set_language(lang: Language) {
    LANG.store(lang as u8, Ordering::Relaxed);
}

/// 解析 `"zh"` / `"en"` 之类的字符串。
pub fn parse_language(s: &str) -> Option<Language> {
    match s.trim().to_ascii_lowercase().as_str() {
        "zh" | "zh-cn" | "zh_cn" | "cn" | "chinese" => Some(Language::Chinese),
        "en" | "en-us" | "en_us" | "english" => Some(Language::English),
        _ => None,
    }
}

/// 启动时调用：应用环境变量和配置里的语言设置。
pub fn init_from_config(config_language: Option<&str>) {
    let from_env = std::env::var("WAVANALY_LANG")
        .ok()
        .and_then(|s| parse_language(&s));
    let from_cfg = config_language.and_then(parse_language);
    set_language(from_env.or(from_cfg).unwrap_or(Language::Chinese));
}

#[must_use]
pub fn language() -> Language {
    match LANG.load(Ordering::Relaxed) {
        1 => Language::Chinese,
        _ => Language::English,
    }
}

fn dictionary() -> &'static HashMap<&'static str, &'static str> {
    static DICT: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
        let mut m = HashMap::new();
        for entries in [
            zh_menus::ENTRIES,
            zh_misc::ENTRIES,
            zh_panels::ENTRIES,
            zh_help::ENTRIES,
        ] {
            for (k, v) in entries {
                m.insert(*k, *v);
            }
        }
        m
    });
    &DICT
}

/// 翻译一段界面文案。找不到时返回原文。
#[must_use]
pub fn translate(key: &'static str) -> &'static str {
    if language() == Language::English {
        return key;
    }
    dictionary().get(key).copied().unwrap_or(key)
}

/// `t!("English text")` -> `&'static str`
#[macro_export]
macro_rules! t {
    ($key:literal) => {
        $crate::i18n::translate($key)
    };
}
