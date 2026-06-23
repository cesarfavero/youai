//! Instruct chat templates and response cleanup (SmolLM2 / Qwen2.5).

/// Bump when template text changes (invalidates response cache keys).
pub const CHAT_TEMPLATE_VERSION: u32 = 1;

pub const SMOLLM2_SYSTEM: &str =
    "You are a helpful AI assistant named SmolLM, trained by Hugging Face.";

/// Full prompt for pipeline prefill (includes assistant generation header).
pub fn format_smollm2_instruct(user_message: &str) -> String {
    let user = user_message.trim();
    format!(
        "<|im_start|>system\n{SMOLLM2_SYSTEM}<|im_end|>\n<|im_start|>user\n{user}<|im_end|>\n<|im_start|>assistant\n"
    )
}

/// Pick instruct formatter from model name (default SmolLM2 ChatML).
pub fn format_instruct_prompt(model: &str, user_message: &str) -> String {
    let m = model.to_lowercase();
    if m.contains("qwen") {
        format_qwen25_instruct(user_message)
    } else {
        format_smollm2_instruct(user_message)
    }
}

pub fn format_qwen25_instruct(user_message: &str) -> String {
    let user = user_message.trim();
    format!(
        "<|im_start|>system\nYou are a helpful assistant.<|im_end|>\n<|im_start|>user\n{user}<|im_end|>\n<|im_start|>assistant\n"
    )
}

pub fn is_eos_piece(piece: &str) -> bool {
    piece.contains("<|im_end|>") || piece.contains("<|endoftext|>")
}

/// Strip special tokens and degeneration tails from model output.
pub fn clean_assistant_response(raw: &str) -> String {
    let had_eos = raw.contains("<|im_end|>") || raw.contains("<|endoftext|>");
    let mut text = raw.replace("<|im_end|>", "").replace("<|endoftext|>", "");
    text = text.replace("<|im_start|>", "");
    if let Some(idx) = text.find("<|im_start|>") {
        text.truncate(idx);
    }
    if had_eos {
        text = text.lines().next().unwrap_or("").to_string();
    }

    // Stop at common degeneration markers (parenthesis spam, etc.)
    let mut out = String::new();
    let mut paren_run = 0u32;
    for ch in text.chars() {
        if ch == '(' || ch == ')' {
            paren_run += 1;
            if paren_run > 12 {
                break;
            }
            out.push(ch);
            continue;
        }
        if !ch.is_whitespace() && ch != '(' && ch != ')' {
            paren_run = 0;
        }
        out.push(ch);
    }

    out.split_whitespace().collect::<Vec<_>>().join(" ").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smollm2_template_has_assistant_header() {
        let p = format_smollm2_instruct("Hello");
        assert!(p.contains("<|im_start|>user\nHello<|im_end|>"));
        assert!(p.ends_with("<|im_start|>assistant\n"));
    }

    #[test]
    fn cleans_im_end() {
        assert_eq!(
            clean_assistant_response("Hi there!<|im_end|>\nmore junk"),
            "Hi there!"
        );
    }
}