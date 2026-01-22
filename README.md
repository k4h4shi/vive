<div align="center">
  <picture>
    <img src="./LOGO.png" height="128" style="border-radius: 50%;">
  </picture>

# vive

vive — parallel AI fixer, **alive in the shell**

[![GitHub](https://img.shields.io/badge/GitHub-k4h4shi%2Fvive-blue)](https://github.com/k4h4shi/vive)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

</div>

Vive（ヴァイヴ）は、**複数のAIエージェントによる並行開発**を管理・指揮するためのターミナルツールです。
Git Worktree、Tmux、そしてClaude Codeを一元管理し、複雑になりがちな「マルチタスク開発」をシンプルにします。

## 特徴 (Features)

Viveは以下の5つのコア機能を提供します。

1.  **Project**: 全てのリポジトリを一箇所で管理・検索。
2.  **Task**: 1クリックで「Worktree + Branch + Tmuxセッション」の作業環境を構築。
3.  **Preview**: エージェントの思考ログやステータス（思考中/入力待ち）をリアルタイム監視。
4.  **Action**: ユーザー操作（Command）に対する具体的な振る舞いを定義（カスタマイズ可能）。
5.  **Hook**: タスク作成時や終了時に、任意のスクリプトを自動実行。

## インストール

### 必須要件

- **Rust** (cargo)
- **tmux**
- **git**

### ソースコードから

```bash
git clone https://github.com/k4h4shi/vive.git
cd vive
./install.sh
```

## 設定 (Configuration)

`~/.vive/config.toml` で挙動をカスタマイズできます。
※ 旧 `[terminal]` 設定は非推奨となりました。今後は `[actions]` を使用してください。

```toml
# プロジェクトのルートディレクトリ
projects_root = "~/src/github"

# アクション設定
[actions]
# Enterキーでタスクを開く時のコマンド (Ghosttyの例)
open = "ghostty --target {session_id}"
# インラインで開く場合 (デフォルト)
# open = "tmux attach -t {session_id}"

# フック設定 (自動実行)
[hooks]
# タスク作成完了時に実行するコマンド
post_create = "tmux send-keys -t {session_id} 'claude' C-m"
```

## ドキュメント

- [コンセプト (Concept)](docs/concept.md)
- [仕様書 (Spec)](docs/spec.md)
- [アーキテクチャ (Architecture)](docs/architecture.md)
