//! Ring poll thread が [`RingConsumer`] から 1 slot ずつ pop して
//! [`process_inline_payload`] へ流す配線層。
//!
//! 役割:
//!
//! - 専用 thread で [`RingConsumer::read`] を spin / sleep しながら呼ぶ
//! - 取得した payload を [`process_inline_payload`] に渡し、Layer 2 stub
//!   sink まで運ぶ
//! - 落ちた件数（producer 側 ring 満杯による drop と、Bridge 側で
//!   `LoggingSink` 等の sink まで届かなかった件数）を観測カウンタに
//!   集約し、warn ログとして出す
//! - `IngestHandle::shutdown()` で thread を join し、`RingConsumer` を
//!   drop して mmap を unmap する
//!
//! 設計参照: `design/17-driver-comm/01-inline-ring.md`「メモリ順序」。
//! ring 空時の spin / sleep 間隔は本ファイル内の `RING_POLL_SPIN_INTERVAL`
//! と `RING_POLL_SLEEP_INTERVAL_MAX` の二定数で min / max を制御する。

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::events_pipeline::{process_inline_payload, EventSink};
use crate::events_schema::EventsSchema;
use crate::ring_consumer::RingConsumer;

/// ring が空のときに試行する初期 poll 間隔。100 µs は active stream
/// 中（直前の poll で event を取れていて `backoff_step` が 0 にリセット
/// されている状態）に MIDI の sub-1 ms latency を保つための初動値。
/// 連続して event が流れている限りこの間隔が維持される。
pub const RING_POLL_SPIN_INTERVAL: Duration = Duration::from_micros(100);

/// 連続して空読みが続いたときに `RING_POLL_SPIN_INTERVAL` を倍化させる
/// 上限。アイドル driver の CPU 占有を抑えるための cap で、ここには
/// MIDI realtime 要件は適用しない。アイドル後に最初に到着した event は
/// 最大 10 ms 遅延し得るが、その時点で `backoff_step` は 0 にリセット
/// され、以降は再び `RING_POLL_SPIN_INTERVAL` に戻る。
pub const RING_POLL_SLEEP_INTERVAL_MAX: Duration = Duration::from_millis(10);

/// `RING_POLL_SPIN_INTERVAL` から `RING_POLL_SLEEP_INTERVAL_MAX` へ
/// 倍率 2 で escalate するときの最大段数（初期 100 µs → 200 µs → … →
/// 6.4 ms → 10 ms にキャップ）。
const POLL_BACKOFF_STEPS_MAX: u32 = 7;

/// drop 件数を warn ログに出すしきい値。1 件出るたびにログを出すと
/// 流量が爆発するため、まとまった単位で出す。
const DROP_LOG_BATCH: u64 = 100;

/// 観測カウンタ。`IngestHandle::shutdown` 後にも参照したいので Arc で
/// 共有する。
#[derive(Debug, Default)]
pub struct IngestStats {
    /// `process_inline_payload` 内部で error log + drop された件数。
    /// 実 stat は `process_inline_payload` 自身の eprintln 経由なので、
    /// ここで二重計上しない。consumer thread が観測する **「sink まで
    /// 通らなかった」件数の総和** ではなく、「pop した件数の総和」を
    /// 入れる（drop 件数は logger 側で集計）。
    pub events_dispatched: AtomicU64,
    /// producer 側 ring 満杯ケース（`emit_event` が `0` を返した状況）が
    /// driver から control channel 経由で報告された件数。本値は driver
    /// stub から差し込むためのスロットで、ring poll thread からは増加
    /// させない（外部からのフィード可）。
    pub producer_drops_reported: AtomicU64,
}

impl IngestStats {
    fn record_dispatched(&self) {
        self.events_dispatched.fetch_add(1, Ordering::Relaxed);
    }

    /// driver 側から「`emit_event` が 0 を返して 1 件 drop した」報告を
    /// 受けたときに caller が呼ぶ。閾値を超えるたびに warn ログを 1 行出す。
    pub fn record_producer_drop(&self, driver_name: &str) {
        let prev = self.producer_drops_reported.fetch_add(1, Ordering::Relaxed);
        let current = prev + 1;
        if current.is_multiple_of(DROP_LOG_BATCH) {
            crate::logging::warn(
                "bridge",
                Some(driver_name),
                format_args!(
                    "driver `{driver_name}` reported {current} cumulative ring-full drops"
                ),
            );
        }
    }
}

/// poll thread を起動した結果として返るハンドル。
///
/// `Drop` ではなく明示的な [`shutdown`](Self::shutdown) を要求する: ingest
/// thread が止まる前に sink を drop すると racey に dispatch が走るため、
/// caller は必ず `shutdown()` を呼んでから sink を解放する。
pub struct IngestHandle {
    /// thread に「終了して」を伝える共有フラグ。
    shutdown_flag: Arc<AtomicBool>,
    /// poll thread の `JoinHandle`。`shutdown()` で取り出し join する。
    join: Option<JoinHandle<()>>,
    /// 観測カウンタ。`stats()` で参照可能。
    stats: Arc<IngestStats>,
}

impl IngestHandle {
    /// 観測カウンタ参照。テストや外部観測から使う。
    #[must_use]
    pub fn stats(&self) -> &IngestStats {
        &self.stats
    }

    /// poll thread に shutdown を伝え、join まで待つ。
    ///
    /// 必ず呼び出すこと。呼び忘れると thread leak。
    pub fn shutdown(mut self) {
        self.shutdown_flag.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            // join 失敗（thread が panic した）は logger に流すだけで
            // 上位伝播はしない。bridge shutdown を止めない方針。
            if join.join().is_err() {
                crate::logging::error(
                    "bridge",
                    None,
                    format_args!("ring ingest thread panicked during shutdown"),
                );
            }
        }
    }
}

impl Drop for IngestHandle {
    fn drop(&mut self) {
        // shutdown() を呼び忘れた場合の保険。stop signal だけ送り、join
        // できないので thread leak になり得る。warn を 1 行出す。
        if self.join.is_some() {
            self.shutdown_flag.store(true, Ordering::Release);
            crate::logging::warn(
                "bridge",
                None,
                format_args!("IngestHandle dropped without explicit shutdown(); join skipped"),
            );
        }
    }
}

/// `consumer` をサブスレッドに送り、Layer 2 stub sink へ流す。
///
/// `schema` は driver の events.yaml ロード結果。`None` の場合、
/// [`process_inline_payload`] は Error ログを出して全件 drop する。
///
/// `sink` の所有権は thread に移る。caller が dispatch 結果を観測したい
/// 場合は `Arc<Mutex<RecordingSink>>` のような共有型を使うこと（テスト
/// では現にそうしている）。
pub fn spawn_ingest<S>(
    driver_name: String,
    consumer: RingConsumer,
    schema: Option<EventsSchema>,
    mut sink: S,
) -> IngestHandle
where
    S: EventSink + Send + 'static,
{
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let stats = Arc::new(IngestStats::default());

    let stop = Arc::clone(&shutdown_flag);
    let stats_for_thread = Arc::clone(&stats);
    let join = thread::spawn(move || {
        ingest_loop(
            &driver_name,
            &consumer,
            schema.as_ref(),
            &mut sink,
            &stop,
            &stats_for_thread,
        );
    });

    IngestHandle {
        shutdown_flag,
        join: Some(join),
        stats,
    }
}

/// thread 本体。`stop` が立つまで poll を続ける。
fn ingest_loop(
    driver_name: &str,
    consumer: &RingConsumer,
    schema: Option<&EventsSchema>,
    sink: &mut dyn EventSink,
    stop: &AtomicBool,
    stats: &IngestStats,
) {
    let mut backoff_step: u32 = 0;
    while !stop.load(Ordering::Acquire) {
        if let Some(payload) = consumer.read() {
            process_inline_payload(driver_name, schema, &payload, sink);
            stats.record_dispatched();
            backoff_step = 0;
        } else {
            let nap = backoff_interval(backoff_step);
            thread::sleep(nap);
            backoff_step = backoff_step.saturating_add(1).min(POLL_BACKOFF_STEPS_MAX);
        }
    }
}

/// `step` 段目の sleep 間隔を返す。`step = 0` で `RING_POLL_SPIN_INTERVAL`
/// (= 100 µs)、step ごとに倍化し、`RING_POLL_SLEEP_INTERVAL_MAX` で頭打ち。
fn backoff_interval(step: u32) -> Duration {
    let base = RING_POLL_SPIN_INTERVAL;
    let cap = RING_POLL_SLEEP_INTERVAL_MAX;
    let scaled = base.saturating_mul(1u32 << step.min(31));
    if scaled > cap {
        cap
    } else {
        scaled
    }
}

/// テスト専用: in-process で push → ingest が dispatch するまで待つ簡易
/// 待機。プロダクション経路では使わない。
#[cfg(test)]
pub(crate) fn wait_for_dispatch(stats: &IngestStats, expected: u64, timeout: Duration) -> bool {
    use std::time::Instant;
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if stats.events_dispatched.load(Ordering::Relaxed) >= expected {
            return true;
        }
        thread::sleep(Duration::from_millis(1));
    }
    false
}

/// テスト都合: `RecordingSink` を `Arc<Mutex<_>>` 越しに dispatch するための
/// アダプタ。本ファイルのテストで使う。
#[cfg(test)]
pub(crate) struct SharedSink<S: EventSink + Send> {
    inner: Arc<std::sync::Mutex<S>>,
}

#[cfg(test)]
impl<S: EventSink + Send> SharedSink<S> {
    pub(crate) fn new(inner: Arc<std::sync::Mutex<S>>) -> Self {
        Self { inner }
    }
}

#[cfg(test)]
impl<S: EventSink + Send> EventSink for SharedSink<S> {
    fn dispatch(&mut self, event: crate::events_pipeline::ValidatedEvent) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.dispatch(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::events_pipeline::{EventSink, ValidatedEvent};
    use rmpv::Value;

    fn midi_schema() -> EventsSchema {
        let yaml = r"
schema_version: 1
events:
  noteOn:
    fields:
      channel:  { type: uint8, range: [1, 16] }
      note:     { type: uint8, range: [0, 127] }
      velocity: { type: uint8, range: [0, 127] }
    binding_filter: [type, channel]
    note_field: note
";
        serde_yaml_ng::from_str(yaml).expect("fixture parses")
    }

    fn encode_note_on() -> Vec<u8> {
        let value = Value::Map(vec![
            (Value::String("type".into()), Value::String("noteOn".into())),
            (Value::String("channel".into()), Value::from(1u8)),
            (Value::String("note".into()), Value::from(60u8)),
            (Value::String("velocity".into()), Value::from(100u8)),
        ]);
        let mut buf = Vec::new();
        rmpv::encode::write_value(&mut buf, &value).expect("encode");
        buf
    }

    #[derive(Default)]
    struct RecordingSink {
        events: Vec<ValidatedEvent>,
    }

    impl EventSink for RecordingSink {
        fn dispatch(&mut self, event: ValidatedEvent) {
            self.events.push(event);
        }
    }

    #[test]
    fn it_should_compute_backoff_interval_within_design_min_and_max() {
        // step 0 = 100 µs, それ以降倍化、最大 10 ms cap
        assert_eq!(backoff_interval(0), RING_POLL_SPIN_INTERVAL);
        assert!(backoff_interval(1) > RING_POLL_SPIN_INTERVAL);
        assert!(backoff_interval(POLL_BACKOFF_STEPS_MAX) <= RING_POLL_SLEEP_INTERVAL_MAX);
        // 上限飽和の確認
        assert_eq!(backoff_interval(31), RING_POLL_SLEEP_INTERVAL_MAX);
    }

    #[test]
    fn it_should_pump_a_pushed_payload_into_the_layer2_sink() {
        let schema = midi_schema();
        let (mut consumer, _fd) = RingConsumer::create(0).expect("default sentinel must succeed");
        consumer.test_push(&encode_note_on());

        let recording: Arc<Mutex<RecordingSink>> = Arc::new(Mutex::new(RecordingSink::default()));
        let sink_for_thread = SharedSink::new(Arc::clone(&recording));
        let handle = spawn_ingest("midi".into(), consumer, Some(schema), sink_for_thread);

        assert!(
            wait_for_dispatch(handle.stats(), 1, Duration::from_secs(2)),
            "pushed event must reach the sink within 2s"
        );
        handle.shutdown();

        let recorded = recording.lock().expect("recording lock");
        assert_eq!(recorded.events.len(), 1);
        assert_eq!(recorded.events[0].event_type, "noteOn");
        assert_eq!(recorded.events[0].driver_name, "midi");
    }

    #[test]
    fn it_should_record_producer_drops_and_log_at_batch_threshold() {
        // record_producer_drop は driver stub から呼ぶ想定。100 件刻みで
        // warn を出すが、本テストはカウンタ加算のみを観測する（logger
        // 出力の subscribe は init 順序が複雑なので unit test scope 外）。
        let stats = IngestStats::default();
        for _ in 0..250 {
            stats.record_producer_drop("midi-stub");
        }
        assert_eq!(
            stats.producer_drops_reported.load(Ordering::Relaxed),
            250,
            "全 record が加算される"
        );
    }
}
