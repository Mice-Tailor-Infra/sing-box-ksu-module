use anyhow::{Context, Result};
use serde_json::{Value, Map};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use log::{warn, info}; // Added info for new log messages

pub fn handle_render(template: PathBuf, output: PathBuf) -> Result<()> {
    // 1. 收集环境变量
    let env_vars: HashMap<String, String> = env::vars().collect();

    // 2. 读取模板
    info!("正在读取模板文件: {:?}", template); // Added info log
    let template_content = fs::read_to_string(&template)
        .with_context(|| format!("读取模板文件失败: {:?}", template))?;

    // 2.1 移除注释
    let json_content = strip_comments(&template_content);

    // 3. 解析模板为 JSON
    let root: Value = serde_json::from_str(&json_content)
        .context("无法将模板解析为有效的 JSON。请确保输入格式正确。")?;

    // 4. 处理抽象语法树 (AST)
    let processed_root = process_value(root, &env_vars)?;

    // 5. 写入输出
    let output_content = serde_json::to_string_pretty(&processed_root)?;
    fs::write(&output, output_content)
        .with_context(|| format!("写入输出文件失败: {:?}", output))?;
    
    info!("渲染完成，输出文件已写入: {:?}", output); // Added info log
    Ok(())
}

fn process_value(v: Value, env: &HashMap<String, String>) -> Result<Value> {
    match v {
        Value::Object(map) => {
            let mut new_map = Map::new();
            for (k, v) in map {
                let processed_v = process_value(v, env)?;
                new_map.insert(k, processed_v);
            }
            Ok(Value::Object(new_map))
        }
        Value::Array(arr) => {
            let mut new_arr = Vec::new();
            for v in arr {
                // 检查数组项级别的 {{VAR}} (Magic Unwrap 候选)
                if let Value::String(ref s) = v {
                    if let Some(var_name) = extract_structural_placeholder(s) {
                        if let Some(parsed_val) = resolve_env_var(var_name, env)? {
                            // Magic Unwrap: 如果是数组则展开
                            if let Value::Array(inner_arr) = parsed_val {
                                info!("发现数组占位符 {{{{{}}}}}，正在展开数组。", var_name); // Added info log
                                for inner_item in inner_arr {
                                    new_arr.push(process_value(inner_item, env)?);
                                }
                            } else {
                                // 不是数组，直接添加
                                new_arr.push(process_value(parsed_val, env)?);
                            }
                        } else {
                            warn!("数组中的占位符 {{{{{}}}}} 未找到或为空，跳过该项。", var_name);
                        }
                        continue;
                    }
                }
                new_arr.push(process_value(v, env)?);
            }
            Ok(Value::Array(new_arr))
        }
        Value::String(s) => {
            // 通用字符串处理
            // 1. 检查结构化替换 {{VAR}} (有效的 JSON 对象替换)
            if let Some(var_name) = extract_structural_placeholder(&s) {
                if let Some(parsed_val) = resolve_env_var(var_name, env)? {
                    info!("发现结构化占位符 {{{{{}}}}}，正在替换为解析后的值。", var_name); // Added info log
                    return process_value(parsed_val, env);
                } else {
                    warn!("值中的占位符 {{{{{}}}}} 未找到或为空，保留原样。", var_name);
                    return Ok(Value::String(s));
                }
            }
            
            // 2. 字符串插值 ${VAR}
            Ok(Value::String(interpolate_string(&s, env)))
        }
        _ => Ok(v),
    }
}

// 辅助函数：查找并解析环境变量为 JSON
fn resolve_env_var(var_name: &str, env: &HashMap<String, String>) -> Result<Option<Value>> {
    if let Some(env_val) = env.get(var_name) {
        let env_val = env_val.trim();
        if env_val.is_empty() {
            return Ok(None);
        }
        let parsed: Value = serde_json::from_str(env_val)
            .with_context(|| format!("无法将环境变量 '{}' 解析为 JSON: {}", var_name, env_val))?;
        Ok(Some(parsed))
    } else {
        Ok(None)
    }
}

// 检查精确的 "{{VAR}}" 模式
fn extract_structural_placeholder(s: &str) -> Option<&str> {
    if s.starts_with("{{") && s.ends_with("}}") {
        // 提取内容
        let content = &s[2..s.len()-2];
        // 确保严格的字母数字/下划线以避免误报？
        // 实际上，在这种情况下，仅检查括号就足以作为强信号。
        Some(content.trim())
    } else {
        None
    }
}

// 简单地插值 ${VAR}
fn interpolate_string(s: &str, env: &HashMap<String, String>) -> String {
    let mut result = s.to_string();
    // 逻辑：查找 ${...} 块并替换。
    // 迭代替换。
    // 注意：这个简单的实现不处理转义。
    // 假设配置不会将 ${} 用于其他目的。
    
    let mut search_start = 0;
    while let Some(start_idx) = result[search_start..].find("${") {
        let abs_start = search_start + start_idx;
        if let Some(end_offset) = result[abs_start..].find('}') {
            let abs_end = abs_start + end_offset;
            let var_name = &result[abs_start+2..abs_end];
            
            // 检查是否主要是字母数字
            if var_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                if let Some(val) = env.get(var_name) {
                     result.replace_range(abs_start..=abs_end, val);
                     // 调整 search_start 以避免如果 val 包含 ${...} 时的无限循环（我们通常不递归插值环境变量值）
                     search_start = abs_start + val.len();
                } else {
                    // 变量未找到。是保持严格还是原样？
                    // 通常，如果期望值，保持原样可能会破坏配置。
                    // 但 shell 行为是空字符串。
                    // 让我们替换为空字符串？或者保留原始字面量？
                    // 用户说“传统 shell 构造”，通常 envsubst 替换为空。
                    // 为了健壮的清理，让我们替换为空。
                    // 但是：也许警告？
                    warn!("变量 ${{{}}} 未找到，替换为空字符串。", var_name);
                    result.replace_range(abs_start..=abs_end, "");
                    search_start = abs_start;
                }
            } else {
                // 不是有效的变量名，跳过
                search_start = abs_end + 1;
            }
        } else {
            break;
        }
    }
    result
}

fn strip_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_quote = false;
    let mut escaped = false;

    while let Some(c) = chars.next() {
        if in_quote {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_quote = false;
            }
        } else {
            // 检查注释开始
            if c == '/' {
                if let Some(&next_c) = chars.peek() {
                    if next_c == '/' {
                        // 行注释: 跳过直到换行符
                        chars.next(); // 消耗第二个 /
                        while let Some(&nc) = chars.peek() {
                            if nc == '\n' {
                                break;
                            }
                            chars.next();
                        }
                        continue;
                    } else if next_c == '*' {
                        // Block comment: skip until */
                        chars.next(); // consume *
                        while let Some(nc) = chars.next() {
                            if nc == '*' {
                                if let Some(&nnc) = chars.peek() {
                                    if nnc == '/' {
                                        chars.next(); // consume /
                                        break;
                                    }
                                }
                            }
                        }
                        continue;
                    }
                }
            }
            if c == '"' {
                in_quote = true;
            }
            out.push(c);
        }
    }
    out
}
