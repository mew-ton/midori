//! `request_ring` JSON Lines wire format。
//!
//! driver は handshake 中に `{"type":"request_ring","slot_size":<u32>}` を
//! stdout に書き、Bridge は本構造体で parse する。Bridge 側は受領した
//! `slot_size` を [`crate::ring_consumer::RingConsumer::create`] に渡す。
//!
//! `slot_size` の sentinel `0` は「default で確保してくれ」を意味する
//! （詳細: `crates/midori-runtime/src/ring_handshake.rs` の
//! `REQUEST_DEFAULT_SLOT_SIZE`）。
//!
//! Bridge → driver の応答は 2 種類:
//!
//! - 受理: `{"type":"ring_ready"}`（fd は別経路 = `SCM_RIGHTS` で渡る）
//! - 拒否: `{"type":"ring_rejected","reason":"…"}`（driver は速やかに exit
//!   する想定。Bridge は `SIGTERM` を発行して念押しする）

use serde::{Deserialize, Serialize};

/// driver → Bridge の `request_ring` メッセージ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestRingMessage {
    /// 識別子。常に `"request_ring"`。
    #[serde(rename = "type")]
    pub message_type: String,
    /// 要求 `slot_size`。`0` は sentinel（`DEFAULT_SLOT_SIZE` 採用）。
    pub slot_size: u32,
}

/// Bridge → driver の `ring_ready` 受理応答。
#[derive(Debug, Clone, Serialize)]
pub struct RingReadyMessage<'a> {
    /// 識別子。常に `"ring_ready"`。
    #[serde(rename = "type")]
    pub message_type: &'a str,
}

/// Bridge → driver の `ring_rejected` 拒否応答。
#[derive(Debug, Clone, Serialize)]
pub struct RingRejectedMessage<'a> {
    /// 識別子。常に `"ring_rejected"`。
    #[serde(rename = "type")]
    pub message_type: &'a str,
    /// 人間可読な拒否理由。alignment / 上限などの判定文。
    pub reason: &'a str,
}

/// `request_ring` メッセージのフィールド `type` 定数。
pub const REQUEST_RING_TYPE: &str = "request_ring";

/// `ring_ready` メッセージのフィールド `type` 定数。
pub const RING_READY_TYPE: &str = "ring_ready";

/// `ring_rejected` メッセージのフィールド `type` 定数。
pub const RING_REJECTED_TYPE: &str = "ring_rejected";

/// driver の stdout 行が `request_ring` メッセージとしてパースできるなら
/// `Some(slot_size)` を返す。`type` フィールドが `"request_ring"` 以外、
/// JSON parse 失敗、必須フィールド欠損のいずれでも `None`。
///
/// caller は `None` が返ったら次の行を待つ（driver は非 JSON ログ行と
/// `request_ring` を混在させ得る、`design/10-driver-plugin.md` 参照）。
#[must_use]
pub fn try_parse_request_ring(line: &str) -> Option<u32> {
    let parsed: RequestRingMessage = serde_json::from_str(line.trim_end()).ok()?;
    if parsed.message_type != REQUEST_RING_TYPE {
        return None;
    }
    Some(parsed.slot_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_should_parse_default_sentinel_slot_size_zero() {
        let parsed = try_parse_request_ring(r#"{"type":"request_ring","slot_size":0}"#);
        assert_eq!(parsed, Some(0));
    }

    #[test]
    fn it_should_parse_custom_slot_size() {
        let parsed = try_parse_request_ring(r#"{"type":"request_ring","slot_size":4104}"#);
        assert_eq!(parsed, Some(4_104));
    }

    #[test]
    fn it_should_ignore_messages_with_other_type_tags() {
        // hello 行や非 JSON 行が混じるケースで誤検出しない。
        assert!(try_parse_request_ring(r#"{"type":"hello","sdk_version":"0.1.0"}"#).is_none());
        assert!(try_parse_request_ring("midori-driver-dummy: noisy debug").is_none());
        assert!(try_parse_request_ring("").is_none());
    }

    #[test]
    fn it_should_reject_messages_missing_slot_size_field() {
        assert!(try_parse_request_ring(r#"{"type":"request_ring"}"#).is_none());
    }

    #[test]
    fn it_should_round_trip_ring_ready_to_json() {
        let msg = RingReadyMessage {
            message_type: RING_READY_TYPE,
        };
        let encoded = serde_json::to_string(&msg).expect("encode");
        assert_eq!(encoded, r#"{"type":"ring_ready"}"#);
    }

    #[test]
    fn it_should_round_trip_ring_rejected_to_json_with_reason() {
        let msg = RingRejectedMessage {
            message_type: RING_REJECTED_TYPE,
            reason: "slot_size 65540 は HARD_SLOT_SIZE 65536 を超えています",
        };
        let encoded = serde_json::to_string(&msg).expect("encode");
        assert!(encoded.starts_with(r#"{"type":"ring_rejected","reason":"slot_size 65540"#));
    }
}
