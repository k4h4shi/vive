# アーキテクチャ (Architecture)

## 技術スタック

- **言語**: Rust
- **UI**: [Ratatui](https://github.com/ratatui-org/ratatui) (TUIフレームワーク)
- **バックエンド**: Tmux (CLI経由で制御)
- **設定**: TOML形式

## コンポーネント構成

Viveは大きく分けて「情報の収集(Input)」「表示(View)」「操作(Action)」の3つのパートで構成されています。

### 1. 情報収集 (Discovery & Monitor)
ファイルシステムとプロセスを監視し、現在の状況をメモリ上に構築します。

*   **Project Discovery**:
    *   設定されたルートディレクトリ（`~/src`など）をスキャンし、Gitリポジトリを探します。
*   **Task Discovery**:
    *   各リポジトリで `git worktree list` を実行し、存在するワークツリー（タスク）を洗い出します。
*   **Process Monitor**:
    *   バックグラウンドで `ps` コマンド等を使用し、各タスク（Tmuxセッション）内で動いているプロセスを監視します。
    *   エージェントの状態（実行中、入力待ち、終了）を判定し、UIに通知します。

### 2. 表示 (TUI Layer)
収集した情報をユーザーに分かりやすく提示します。

*   **Dashboard View**:
    *   左ペイン: プロジェクトとタスクのツリーリスト。
    *   右ペイン: 選択中のタスクの詳細や、ターミナル出力のプレビュー。
*   **Focus Pane Management**:
    *   **FocusPane Enum**: `Sidebar` (default) or `Preview` - キー操作の対象ペインを管理します。
    *   **Sidebar Focus**: `j`/`k` でプロジェクトリストを移動。
    *   **Preview Focus**: `j`/`k` でプレビュー内容をスクロール。
    *   **Visual Feedback**: アクティブなペインは黄色の枠線で強調表示。
*   **Preview Window**:
    *   `tmux capture-pane` を定期的に実行し、タスクの最新の実行ログを表示します。

### 3. 操作 (Orchestrator)
ユーザーの操作を具体的なシステムコマンドに変換して実行します。
ユーザーがカスタマイズ可能な「フック」や「コマンド設定」はここで処理されます。

*   **Session Manager**:
    *   `tmux new-session`, `tmux kill-session` 等を発行し、タスクに対応するセッションを管理します。
*   **Git Wrapper**:
    *   `git worktree add/remove` 等を実行し、物理的な作業ディレクトリを管理します。
*   **Command Dispatcher**:
    *   ユーザー設定 (`config.toml`) に基づき、タスクを開く際のコマンド（例: `ghostty --target {session_id}`）や、作成後の自動実行コマンドを構築して実行します。

## データフロー

1.  **起動時**: プロジェクトと既存のワークツリーをスキャンし、内部状態 (`AppState`) を構築。
2.  **ループ**:
    *   **Monitor**: 各タスクのプロセス状態とログを更新。
    *   **TUI**: `AppState` の内容を描画。
3.  **アクション**:
    *   ユーザーが「作成」→ `git worktree add` → `tmux new-session` → `AppState`更新。
    *   ユーザーが「開く」→ 設定されたコマンドを実行（端末切り替え）。

## カスタマイズ設計

Viveは「何を実行するか」をユーザー設定に委ねる設計になっています。

### コマンド実行 (Exec)
タスクを開く際や、作成時に実行されるコマンドは `config.toml` で定義可能です。
プレースホルダー（`{session_id}`, `{path}`など）を使うことで、動的なコマンド生成が可能です。

これにより、以下のような柔軟な運用が可能になります：
*   OS標準のターミナルで開く
*   VSCodeで開く (`code {path}`)
*   タスク作成直後に `npm install` を走らせる
*   タスク作成直後に Claude Code を起動してプロンプトを流し込む
