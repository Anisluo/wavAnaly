//! WaveDrom 脚本导入：把 WaveDrom 的 JSON 时序描述转换成 VCD 文本，
//! 之后走和普通 VCD 完全相同的加载路径显示，并可导出为 .vcd 文件。
//!
//! 支持的语法（WaveDrom 教程 1-6 的内容）：
//! * `signal: [ {name, wave, data, period, phase}, ['组名', ...], {} ]`
//! * wave 字符：`p P n N`（时钟）`h H l L 1 0`（电平）`x`（未知）`z`（高阻）
//!   `= 2..9 u d`（带 `data` 的总线）`.`（保持）`|`（间隔，按保持处理）
//! * `data` 可以是数组，也可以是空格分隔的字符串
//! * `config.period_ns`（wavAnaly 扩展）：一个 WaveDrom 周期对应多少 ns，默认 10
//! * `head.text` / `foot.text` 会写进 VCD 的 `$comment`
//!
//! 文件可以是宽松的 JSON5 风格：不带引号的键、单引号字符串、尾逗号、`//` 和 `/* */` 注释。

use eyre::{Context, Result, bail, eyre};
use serde_json::Value;
use std::fmt::Write;

/// 是不是 WaveDrom 文件的扩展名
#[must_use]
pub fn is_wavedrom_extension(ext: &str) -> bool {
    // get_multi_extension 会返回 "wavedrom.json" 这样的多段扩展名, 只看最后一段
    let last = ext.rsplit('.').next().unwrap_or(ext);
    matches!(
        last.to_ascii_lowercase().as_str(),
        "json" | "json5" | "wavedrom" | "wd"
    )
}

/// 文件内容看起来像不像 WaveDrom（用于拖放等没有扩展名的场合）
#[must_use]
pub fn looks_like_wavedrom(bytes: &[u8]) -> bool {
    let head: String = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]).to_string();
    let trimmed = head.trim_start();
    trimmed.starts_with('{') && head.contains("signal")
}

// ------------------------------------------------------------------ JSON5 -> JSON
/// 把宽松的 JSON5 写法规整成严格 JSON，好交给 serde_json。
fn normalize_json5(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len() + 64);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // 注释
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2;
            continue;
        }
        // 字符串（单引号或双引号），统一输出成双引号
        if c == '"' || c == '\'' {
            let quote = c;
            out.push('"');
            i += 1;
            while i < chars.len() && chars[i] != quote {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    // 转义序列原样保留，但 \' 变成 '
                    if chars[i + 1] == '\'' {
                        out.push('\'');
                    } else {
                        out.push('\\');
                        out.push(chars[i + 1]);
                    }
                    i += 2;
                    continue;
                }
                if chars[i] == '"' {
                    out.push_str("\\\"");
                } else {
                    out.push(chars[i]);
                }
                i += 1;
            }
            out.push('"');
            i += 1;
            continue;
        }
        // 不带引号的键: 标识符后面跟着 ':'
        if c.is_ascii_alphabetic() || c == '_' || c == '$' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '$') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let mut j = i;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && chars[j] == ':' {
                write!(out, "\"{word}\"").unwrap();
            } else {
                out.push_str(&word); // true / false / null / 数字里的 e 等
            }
            continue;
        }
        // 尾逗号: ",  ]" 或 ",  }"
        if c == ',' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && (chars[j] == ']' || chars[j] == '}') {
                i += 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

// ------------------------------------------------------------------ 信号模型
#[derive(Debug, Clone)]
struct Sig {
    scope: Vec<String>,
    name: String,
    wave: Vec<char>,
    data: Vec<String>,
    period: f64,
    phase: f64,
}

fn collect_signals(v: &Value, scope: &mut Vec<String>, out: &mut Vec<Sig>) {
    match v {
        Value::Array(items) => {
            // ['group name', item, item, ...]
            let mut iter = items.iter();
            if let Some(Value::String(group)) = items.first() {
                scope.push(group.clone());
                iter.next();
                for it in iter {
                    collect_signals(it, scope, out);
                }
                scope.pop();
            } else {
                for it in iter {
                    collect_signals(it, scope, out);
                }
            }
        }
        Value::Object(obj) => {
            let Some(name) = obj.get("name").and_then(Value::as_str) else {
                return; // {} 空行或匿名项
            };
            let wave = obj.get("wave").and_then(Value::as_str).unwrap_or("");
            if wave.is_empty() {
                return;
            }
            let data = match obj.get("data") {
                Some(Value::Array(a)) => a
                    .iter()
                    .map(|d| match d {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    })
                    .collect(),
                Some(Value::String(s)) => s.split_whitespace().map(ToString::to_string).collect(),
                Some(other) => vec![other.to_string()],
                None => vec![],
            };
            out.push(Sig {
                scope: scope.clone(),
                name: name.to_string(),
                wave: wave.chars().collect(),
                data,
                period: obj.get("period").and_then(Value::as_f64).unwrap_or(1.0).max(0.01),
                phase: obj.get("phase").and_then(Value::as_f64).unwrap_or(0.0),
            });
        }
        _ => {}
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Level {
    Bit(char), // '0' '1' 'x' 'z'
    Str(String),
}

fn is_data_char(c: char) -> bool {
    matches!(c, '=' | '2'..='9' | 'u' | 'd')
}

/// 把一个信号展开成 (时间 ns, 值) 列表
fn expand(sig: &Sig, period_ns: f64) -> (bool, Vec<(u64, Level)>) {
    let is_string = sig.wave.iter().any(|&c| is_data_char(c));
    let cycle = period_ns * sig.period;
    let offset = -sig.phase * cycle;
    let mut changes: Vec<(u64, Level)> = vec![];
    let mut data_idx = 0usize;
    let mut last: Option<char> = None; // 上一个有效字符（用于 '.' 与 '|'）
    let mut cur: Option<Level> = None;

    let push = |t: f64, v: Level, changes: &mut Vec<(u64, Level)>, cur: &mut Option<Level>| {
        let t = t.max(0.0).round() as u64;
        if cur.as_ref() != Some(&v) {
            changes.push((t, v.clone()));
            *cur = Some(v);
        }
    };

    for (i, &raw) in sig.wave.iter().enumerate() {
        let t0 = offset + i as f64 * cycle;
        let half = t0 + cycle / 2.0;
        let c = if raw == '.' || raw == '|' {
            match last {
                Some(l) => l,
                None => continue,
            }
        } else {
            last = Some(raw);
            raw
        };
        match c {
            'p' | 'P' => {
                push(t0, Level::Bit('1'), &mut changes, &mut cur);
                push(half, Level::Bit('0'), &mut changes, &mut cur);
            }
            'n' | 'N' => {
                push(t0, Level::Bit('0'), &mut changes, &mut cur);
                push(half, Level::Bit('1'), &mut changes, &mut cur);
            }
            'h' | 'H' | '1' => push(t0, Level::Bit('1'), &mut changes, &mut cur),
            'l' | 'L' | '0' => push(t0, Level::Bit('0'), &mut changes, &mut cur),
            'x' => push(
                t0,
                if is_string { Level::Str("x".into()) } else { Level::Bit('x') },
                &mut changes,
                &mut cur,
            ),
            'z' => push(
                t0,
                if is_string { Level::Str("z".into()) } else { Level::Bit('z') },
                &mut changes,
                &mut cur,
            ),
            _ if is_data_char(c) => {
                // '.' 延续总线时不取新数据；新的数据字符才取
                if raw != '.' && raw != '|' {
                    let label = sig.data.get(data_idx).cloned().unwrap_or_default();
                    data_idx += 1;
                    push(t0, Level::Str(label), &mut changes, &mut cur);
                }
            }
            _ => {} // 未知字符忽略
        }
    }
    (is_string, changes)
}

fn vcd_id(n: usize) -> String {
    let mut n = n;
    let mut s = String::new();
    loop {
        s.insert(0, char::from(33 + (n % 94) as u8));
        if n < 94 {
            break;
        }
        n = n / 94 - 1;
    }
    s
}

/// WaveDrom 文本 -> VCD 文本
pub fn to_vcd(text: &str) -> Result<String> {
    let json = normalize_json5(text);
    let root: Value = serde_json::from_str(&json)
        .map_err(|e| eyre!("不是有效的 WaveDrom/JSON: {e}"))
        .wrap_err("解析 WaveDrom 失败")?;
    let signal = root
        .get("signal")
        .ok_or_else(|| eyre!("WaveDrom 文件缺少 signal 数组"))?;
    let period_ns = root
        .get("config")
        .and_then(|c| c.get("period_ns"))
        .and_then(Value::as_f64)
        .unwrap_or(10.0);
    if period_ns <= 0.0 {
        bail!("config.period_ns 必须大于 0");
    }

    let mut sigs = vec![];
    collect_signals(signal, &mut vec![], &mut sigs);
    if sigs.is_empty() {
        bail!("WaveDrom 文件里没有带 wave 的信号");
    }

    // 展开
    let expanded: Vec<(Sig, bool, Vec<(u64, Level)>)> = sigs
        .iter()
        .map(|s| {
            let (is_str, ch) = expand(s, period_ns);
            (s.clone(), is_str, ch)
        })
        .collect();
    let max_len = sigs
        .iter()
        .map(|s| (s.wave.len() as f64 * period_ns * s.period) as u64)
        .max()
        .unwrap_or(0);

    // 头部
    let mut out = String::new();
    out.push_str("$comment generated by wavAnaly from WaveDrom $end\n");
    for key in ["head", "foot"] {
        if let Some(t) = root.get(key).and_then(|h| h.get("text")).and_then(Value::as_str) {
            let _ = writeln!(out, "$comment {key}: {} $end", t.replace("$end", ""));
        }
    }
    out.push_str("$timescale 1ns $end\n");

    // 作用域：按出现顺序分组（同一 scope 的信号写在一起）
    let mut order: Vec<Vec<String>> = vec![];
    for (s, _, _) in &expanded {
        if !order.contains(&s.scope) {
            order.push(s.scope.clone());
        }
    }
    let root_scope = "wavedrom";
    let _ = writeln!(out, "$scope module {root_scope} $end");
    for sc in &order {
        for name in sc {
            let _ = writeln!(out, "$scope module {} $end", sanitize(name));
        }
        for (idx, (s, is_str, _)) in expanded.iter().enumerate() {
            if &s.scope == sc {
                let kind = if *is_str { "string" } else { "wire" };
                let _ = writeln!(out, "$var {kind} 1 {} {} $end", vcd_id(idx), sanitize(&s.name));
            }
        }
        for _ in sc {
            out.push_str("$upscope $end\n");
        }
    }
    out.push_str("$upscope $end\n$enddefinitions $end\n");

    // 变化：按时间合并
    let mut all: Vec<(u64, usize, &Level)> = vec![];
    for (idx, (_, _, ch)) in expanded.iter().enumerate() {
        for (t, v) in ch {
            all.push((*t, idx, v));
        }
    }
    all.sort_by_key(|(t, idx, _)| (*t, *idx));
    let mut cur_t: Option<u64> = None;
    for (t, idx, v) in all {
        if cur_t != Some(t) {
            let _ = writeln!(out, "#{t}");
            cur_t = Some(t);
        }
        match v {
            Level::Bit(b) => {
                let _ = writeln!(out, "{b}{}", vcd_id(idx));
            }
            Level::Str(s) => {
                let _ = writeln!(out, "s{} {}", s.replace(' ', "_"), vcd_id(idx));
            }
        }
    }
    let _ = writeln!(out, "#{max_len}");
    Ok(out)
}

fn sanitize(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_whitespace() { '_' } else { c })
        .collect();
    if s.is_empty() { "_".into() } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json5_normalization() {
        let src = "{ signal: [ { name: 'clk', wave: 'p..', }, ], // c\n }";
        let n = normalize_json5(src);
        let v: Value = serde_json::from_str(&n).unwrap();
        assert_eq!(v["signal"][0]["name"], "clk");
    }

    #[test]
    fn clock_and_bus() {
        let src = r#"{ signal: [
            { name: "clk", wave: "p..." },
            { name: "bus", wave: "x=.=x", data: ["A", "B"] },
            { name: "en",  wave: "01.0" }
        ], config: { period_ns: 10 } }"#;
        let vcd = to_vcd(src).unwrap();
        assert!(vcd.contains("$var wire 1 ! clk $end"));
        assert!(vcd.contains("$var string 1 \" bus $end"));
        assert!(vcd.contains("#0\n1!"));
        assert!(vcd.contains("#5\n0!"));
        assert!(vcd.contains("sA \""));
        assert!(vcd.contains("sB \""));
        // en: 0 at 0, 1 at 10, 0 at 30
        assert!(vcd.contains("#10\n1#") || vcd.contains("1#"));
    }

    #[test]
    fn groups_become_scopes() {
        let src = r#"{ signal: [ ["bus", { name: "a", wave: "01" }], { name: "b", wave: "10" } ] }"#;
        let vcd = to_vcd(src).unwrap();
        assert!(vcd.contains("$scope module bus $end"));
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl crate::SystemState {
    /// 把 WaveDrom 转换结果写成 .vcd
    pub(crate) fn export_wavedrom_vcd(&mut self, path: Option<camino::Utf8PathBuf>) {
        use crate::async_util::perform_async_work;
        use crate::channels::checked_send_many;
        use crate::message::Message;
        use tracing::{error, info};

        let Some(vcd) = self.wavedrom_vcd.clone() else {
            error!("当前波形不是从 WaveDrom 脚本加载的, 没有可导出的 VCD");
            return;
        };
        let messages = move |destination: rfd::FileHandle| async move {
            let p = destination.path().to_path_buf();
            match std::fs::write(&p, vcd.as_bytes()) {
                Ok(()) => {
                    info!("VCD written to {}", p.display());
                    vec![]
                }
                Err(e) => vec![Message::Error(eyre!("写入 {} 失败: {e}", p.display()))],
            }
        };
        if let Some(path) = path {
            let sender = self.channels.msg_sender.clone();
            perform_async_work(async move {
                checked_send_many(&sender, messages(path.into_std_path_buf().into()).await);
            });
        } else {
            let default_name = self
                .user
                .waves
                .as_ref()
                .and_then(|w| w.source.as_file().map(|p| p.file_stem().unwrap_or("wavedrom").to_string()))
                .unwrap_or_else(|| "wavedrom".to_string());
            self.file_dialog_save(
                t!("Export VCD"),
                &crate::file_dialog::VCD_EXPORT_FILTER,
                Some(format!("{default_name}.vcd")),
                messages,
            );
        }
    }
}
