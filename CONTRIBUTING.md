## Contributing

ありがとうございます！vive への貢献は歓迎です。

### できること

- バグ報告 / 改善提案（Issue）
- ドキュメント改善（README / `docs/`）
- 実装改善（PR）

### 開発環境

- Rust (stable)
- git
- tmux

### セットアップ

```bash
git clone https://github.com/k4h4shi/vive.git
cd vive
./install.sh
```

### ローカルでの確認

PR 前に最低限、以下が通ることを確認してください。

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test
```

### PR ガイド

- **目的が伝わるタイトル/説明**にしてください（Why を重視）
- 変更が大きい場合は **Issue を先に立てる**（設計/方向性のすり合わせ）
- UI/TUI の変更は、可能であれば **スクリーンショット**を添付してください
- 仕様/使い方が変わる場合は **README / docs の更新**も含めてください

