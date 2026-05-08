# HTTP ドライバーのイメージ

> ステータス：アイデアレベル（スケッチのみ）
> 最終更新：2026-05-08

HTTP はドライバー固有の I/O モデルを持つ。初期スコープ外（[`README.md`](./README.md) の「初期スコープ外ドライバー」表に「将来」として列挙されているもの）のスケッチ。

## 入力（サーバー起動型）

ブリッジ起動時に HTTP サーバーが指定ポートで立ち上がる。
アダプター の `definition` は受け付ける API エンドポイントを記述し、`binding` でリクエストボディのフィールドを ComponentState にマッピングする。

```yaml
# 入力 アダプター（driver: http）のイメージ
definition:
  components:
    - id: note_trigger
      type: pulser
    - id: expression
      type: slider
      range: [0, 1]

binding:
  input:
    driver: http
    mappings:
      - from:
          method: POST
          path: /note
          body: $.note        # JSON パス
        to:
          target: note_trigger.triggered
          set: pulse
      - from:
          method: POST
          path: /expression
          body: $.value
        to:
          target: expression.value
          set: value
```

## 出力（HTTP クライアント型）

Signal が発生するたびにプロファイルの connection で設定した URL へ JSON body をリクエスト送出する。

```yaml
# 出力 アダプター（driver: http）のイメージ
binding:
  output:
    driver: http
    mappings:
      - from:
          target: upper.{note}.pressed
        to:
          method: POST
          path: /avatar/key
          body:
            note: "{note}"
            pressed: "{value}"
```

## 未確定の論点

このスケッチは I/O モデルの輪郭を示すのみで、確定設計に進めるには以下の決定が要る:

- 認証・認可（API キー / トークン / mTLS / 無認証許容範囲）
- レート制限とエラーハンドリング（4xx / 5xx 受領時の挙動、再試行ポリシ）
- HTTPS の必須性と TLS 証明書の扱い
- サーバー停止 / クライアント切断時の再起動ポリシ
- 出力側のスロットリング・バッチング（[`../layers/05-output-driver/requirements.md`](../layers/05-output-driver/requirements.md) はドライバー内部責務としているが具体仕様は未決）

## 関連

- 入力ドライバーの I/O モデル区分 → [`../layers/01-input-driver/requirements.md`](../layers/01-input-driver/requirements.md)
- 出力ドライバーの I/O モデル区分 → [`../layers/05-output-driver/requirements.md`](../layers/05-output-driver/requirements.md)
- サーバー起動型の `physical_input_identity` 規約 → [`../layers/01-input-driver/requirements.md`](../layers/01-input-driver/requirements.md) のサーバー起動型節
