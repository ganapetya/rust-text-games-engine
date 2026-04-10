//! Token-based pricing for game LLM calls — same formula as
//! `AnalysisProxy.calculateTokenBasedPrice` in shakti-actors:
//! `round((prompt/1000)*input_per_1k + (completion/1000)*output_per_1k)`.

use serde::{Deserialize, Serialize};

/// Rates for one logical endpoint (coins per 1000 tokens).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointTokenRates {
    pub input_per_1k: i64,
    pub output_per_1k: i64,
}

/// Bundle passed from shakti-actors at bootstrap (optional on the engine).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameBillingRates {
    pub variant: String,
    pub prepare: EndpointTokenRates,
    pub translate: EndpointTokenRates,
}

impl GameBillingRates {
    pub fn prepare_endpoint_path() -> &'static str {
        "/game/prepare"
    }

    pub fn translate_endpoint_path() -> &'static str {
        "/game/translate"
    }
}

/// Compute total coins from token usage and per-1k rates (Scala `math.round` semantics).
pub fn coins_for_usage(
    prompt_tokens: u64,
    completion_tokens: u64,
    input_per_1k: i64,
    output_per_1k: i64,
) -> i64 {
    let input = (prompt_tokens as f64 / 1000.0) * (input_per_1k as f64);
    let output = (completion_tokens as f64 / 1000.0) * (output_per_1k as f64);
    (input + output).round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_scala_rounding_examples() {
        // 1000 in + 500 out at 100/800 per 1k => 100 + 400 = 500
        assert_eq!(coins_for_usage(1000, 500, 100, 800), 500);
        // fractional: 1500 prompt at 400/1k => 600
        assert_eq!(coins_for_usage(1500, 0, 400, 2000), 600);
    }

    #[test]
    fn serde_roundtrip() {
        let b = GameBillingRates {
            variant: "gpt-5.2".into(),
            prepare: EndpointTokenRates {
                input_per_1k: 400,
                output_per_1k: 2000,
            },
            translate: EndpointTokenRates {
                input_per_1k: 100,
                output_per_1k: 800,
            },
        };
        let j = serde_json::to_string(&b).unwrap();
        let back: GameBillingRates = serde_json::from_str(&j).unwrap();
        assert_eq!(back, b);
    }
}
