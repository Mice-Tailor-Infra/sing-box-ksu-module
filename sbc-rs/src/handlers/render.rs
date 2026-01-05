use anyhow::{Context, Result};
use serde_json::{Value, Map};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

pub fn handle_render(template: PathBuf, output: PathBuf) -> Result<()> {
    // 1. Gather Environment Variables
    let env_vars: HashMap<String, String> = env::vars().collect();

    // 2. Read Template
    let template_content = fs::read_to_string(&template)
        .with_context(|| format!("Failed to read template file: {:?}", template))?;

    // 2.1 Strip Comments
    let json_content = strip_comments(&template_content);

    // 3. Parse Template as JSON
    let root: Value = serde_json::from_str(&json_content)
        .context("Failed to parse template as valid JSON. Ensure input is well-formed.")?;

    // 4. Process AST
    let processed_root = process_value(root, &env_vars)?;

    // 5. Write Output
    let output_content = serde_json::to_string_pretty(&processed_root)?;
    fs::write(&output, output_content)
        .with_context(|| format!("Failed to write output file: {:?}", output))?;
    
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
                // Check for {{VAR}} at the array item level (Magic Unwrap candidate)
                if let Value::String(ref s) = v {
                    if let Some(var_name) = extract_structural_placeholder(s) {
                        if let Some(parsed_val) = resolve_env_var(var_name, env)? {
                            // Magic Unwrap: Splice if array
                            if let Value::Array(inner_arr) = parsed_val {
                                for inner_item in inner_arr {
                                    new_arr.push(process_value(inner_item, env)?);
                                }
                            } else {
                                // Not array, just push
                                new_arr.push(process_value(parsed_val, env)?);
                            }
                        } else {
                            eprintln!("Warning: Placeholder {{{{{}}}}} in array not found/empty, skipping specific item.", var_name);
                        }
                        continue;
                    }
                }
                new_arr.push(process_value(v, env)?);
            }
            Ok(Value::Array(new_arr))
        }
        Value::String(s) => {
            // General String Handling
            // 1. Check for Structural Substitution {{VAR}} (Valid JSON Object replacement)
            if let Some(var_name) = extract_structural_placeholder(&s) {
                if let Some(parsed_val) = resolve_env_var(var_name, env)? {
                    return process_value(parsed_val, env);
                } else {
                    eprintln!("Warning: Placeholder {{{{{}}}}} in value not found/empty, keeping original.", var_name);
                    return Ok(Value::String(s));
                }
            }
            
            // 2. String Interpolation ${VAR}
            Ok(Value::String(interpolate_string(&s, env)))
        }
        _ => Ok(v),
    }
}

// Helper to look up and parse env var as JSON
fn resolve_env_var(var_name: &str, env: &HashMap<String, String>) -> Result<Option<Value>> {
    if let Some(env_val) = env.get(var_name) {
        let env_val = env_val.trim();
        if env_val.is_empty() {
            return Ok(None);
        }
        let parsed: Value = serde_json::from_str(env_val)
            .with_context(|| format!("Failed to parse env var '{}' as JSON: {}", var_name, env_val))?;
        Ok(Some(parsed))
    } else {
        Ok(None)
    }
}

// Check for exact "{{VAR}}" pattern
fn extract_structural_placeholder(s: &str) -> Option<&str> {
    if s.starts_with("{{") && s.ends_with("}}") {
        // Extract content
        let content = &s[2..s.len()-2];
        // Ensure strictly alphanumeric/underscore to avoid false positives?
        // Actually, just checking brackets is a strong enough signal for now in this context.
        Some(content.trim())
    } else {
        None
    }
}

// Simple interpolation of ${VAR}
fn interpolate_string(s: &str, env: &HashMap<String, String>) -> String {
    let mut result = s.to_string();
    // Logic: find ${...} blocks and replace.
    // Iterative replacement.
    // NOTE: This simple implementation doesn't handle escaping. 
    // Assuming config doesn't use ${} for anything else.
    
    let mut search_start = 0;
    while let Some(start_idx) = result[search_start..].find("${") {
        let abs_start = search_start + start_idx;
        if let Some(end_offset) = result[abs_start..].find('}') {
            let abs_end = abs_start + end_offset;
            let var_name = &result[abs_start+2..abs_end];
            
            // Check if alphanumeric mostly
            if var_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                if let Some(val) = env.get(var_name) {
                     result.replace_range(abs_start..=abs_end, val);
                     // Adjust search_start to avoid infinite loops if val contains ${...} (we don't recursive interpolate env vals generally)
                     search_start = abs_start + val.len();
                } else {
                    // Var not found. Keep strict or leave as is?
                    // Usually leaving as is might break config if it expects value.
                    // But shell behavior is empty string.
                    // Let's replace with empty string? Or keep raw literal?
                    // User said "Legacy shell constructs", usually envsubst replaces with empty.
                    // Let's replace with empty for robust cleanup.
                    // BUT: Maybe warn?
                    eprintln!("Warning: Variable ${{{}}} not found, replacing with empty string.", var_name);
                    result.replace_range(abs_start..=abs_end, "");
                    search_start = abs_start;
                }
            } else {
                // Not a valid var name, skip
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
            // Check for comment start
            if c == '/' {
                if let Some(&next_c) = chars.peek() {
                    if next_c == '/' {
                        // Line comment: skip until newline
                        chars.next(); // consume second /
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
