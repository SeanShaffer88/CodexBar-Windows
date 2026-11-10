//! Public list prices used for cost **estimates** on the local-log
//! providers (Codex, Claude Code), whose logs carry tokens but no money.
//!
//! Prices are USD per 1M tokens, as published by OpenAI and Anthropic
//! (checked 2026-11-09). They are list prices: subscription plans,
//! discounts, and tiered pricing mean a real bill can differ, and the UI
//! always labels these numbers as estimates. A model with no table entry
//! produces no estimate at all — never a guess.

/// Per-1M-token prices for one model family.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelPrice {
    pub input: f64,
    pub cached_input: f64,
    pub cache_write: f64,
    pub output: f64,
}

const fn p(input: f64, cached_input: f64, cache_write: f64, output: f64) -> ModelPrice {
    ModelPrice {
        input,
        cached_input,
        cache_write,
        output,
    }
}

/// The list price for a model id, matched by family so dated variants
/// (`claude-sonnet-4-5-20250929`) resolve to their family's price.
pub fn price_for(model: &str) -> Option<ModelPrice> {
    let m = model.to_ascii_lowercase();
    // Anthropic families.
    if m.contains("opus") {
        return Some(p(15.0, 1.50, 18.75, 75.0));
    }
    if m.contains("sonnet") {
        return Some(p(3.0, 0.30, 3.75, 15.0));
    }
    if m.contains("haiku") {
        if m.contains("3-5") || m.contains("3.5") {
            return Some(p(0.80, 0.08, 1.00, 4.0));
        }
        return Some(p(1.0, 0.10, 1.25, 5.0));
    }
    // OpenAI families (cache writes are not charged separately; the
    // cache-write column mirrors input for those call sites).
    if m.contains("gpt-5") {
        if m.contains("nano") {
            return Some(p(0.05, 0.005, 0.05, 0.40));
        }
        if m.contains("mini") {
            return Some(p(0.25, 0.025, 0.25, 2.0));
        }
        return Some(p(1.25, 0.125, 1.25, 10.0));
    }
    if m.contains("gpt-4.1") {
        if m.contains("mini") {
            return Some(p(0.40, 0.10, 0.40, 1.60));
        }
        if m.contains("nano") {
            return Some(p(0.10, 0.025, 0.10, 0.40));
        }
        return Some(p(2.0, 0.50, 2.0, 8.0));
    }
    if m.contains("gpt-4o") {
        if m.contains("mini") {
            return Some(p(0.15, 0.075, 0.15, 0.60));
        }
        return Some(p(2.50, 1.25, 2.50, 10.0));
    }
    None
}

/// Estimated USD for a token mix at `price`. All counts are raw tokens.
pub fn estimate_usd(
    uncached_input: i64,
    cache_read: i64,
    cache_write: i64,
    output: i64,
    price: &ModelPrice,
) -> f64 {
    (uncached_input as f64 * price.input
        + cache_read as f64 * price.cached_input
        + cache_write as f64 * price.cache_write
        + output as f64 * price.output)
        / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn families_resolve() {
        assert_eq!(price_for("claude-sonnet-4-5-20250929").unwrap().input, 3.0);
        assert_eq!(price_for("claude-opus-4-1").unwrap().output, 75.0);
        assert_eq!(price_for("claude-haiku-4-5").unwrap().input, 1.0);
        assert_eq!(price_for("claude-3-5-haiku-latest").unwrap().input, 0.80);
        assert_eq!(price_for("gpt-5-codex").unwrap().input, 1.25);
        assert_eq!(price_for("gpt-5-mini").unwrap().output, 2.0);
        assert!(price_for("some-unknown-model").is_none());
    }

    #[test]
    fn estimate_math() {
        let pr = price_for("claude-sonnet-4-5").unwrap();
        // 1M fresh input + 1M output = $3 + $15.
        assert_eq!(estimate_usd(1_000_000, 0, 0, 1_000_000, &pr), 18.0);
        // Cache reads are cheap: 1M reads = $0.30.
        assert_eq!(estimate_usd(0, 1_000_000, 0, 0, &pr), 0.30);
    }
}
