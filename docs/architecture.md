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
*   バックグラウンドで `ps` コマンド等を使用し、各タスク（Tmuxウィンドウ）内で動いているプロセスを監視します。
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

### Tmux Window 命名規則

タスク（Worktree）に対応するTmuxウィンドウの命名規則は、タスクの作成方法によって異なります。

| 作成方法 | Window名の形式 | 例 |
| :--- | :--- | :--- |
| **Manual** | `ブランチ名` | `feature/my-branch` |
| **Pick from Issue** | `ブランチ名` | `issue-123` |

#### サニタイズルール

Issueタイトルはtmux window名として安全に使えるよう以下の変換が適用されます：

- `.` → `_` （ドットをアンダースコアに変換）
- `:` → ` ` （コロンをスペースに変換）
- 改行文字は除去
- 連続する空白は1つに正規化

#### 長さ制限

- Window名全体（ブランチ名 + `_` + タイトル）が **100文字** を超える場合、末尾を `...` で省略

#### Window検索

Window名にIssueタイトルが含まれるため、ブランチ名による検索には**プレフィックスマッチ**を使用します。
後方互換のため、過去に作成された window が `issue-123_Fix login bug` のようにブランチ名にサフィックスを持つ場合でも、ブランチ名 `issue-123` から prefix match で解決できます。

### 3. 操作 (Orchestrator)
ユーザーの操作を具体的なシステムコマンドに変換して実行します。
ユーザーがカスタマイズ可能な「フック」や「コマンド設定」はここで処理されます。

*   **Session/Window Manager**:
    *   `tmux new-session`, `tmux new-window`, `tmux kill-window` 等を発行し、プロジェクトセッションとタスク用ウィンドウを管理します。
*   **Git Wrapper**:
    *   `git worktree add/remove` 等を実行し、物理的な作業ディレクトリを管理します。
*   **Command Dispatcher**:
    *   ユーザー設定 (`config.toml`) の `[keybindings]` セクションに基づき、各キーに対応するコマンドを構築して実行します。
    *   キーごとに異なるアクションを設定可能（例: `Enter` でtmux切り替え、`n` でエディタを開く）。

## データフロー

1.  **起動時**: プロジェクトと既存のワークツリーをスキャンし、内部状態 (`AppState`) を構築。
2.  **ループ**:
    *   **Monitor**: 各タスクのプロセス状態とログを更新。
    *   **TUI**: `AppState` の内容を描画。
3.  **アクション**:
    *   ユーザーが「作成」→ `git worktree add` → `tmux new-session/new-window` → `AppState`更新。
    *   ユーザーが「開く」→ 設定されたキーバインディングに基づきコマンドを実行（端末切り替え）。

## カスタマイズ設計

Viveは「何を実行するか」をユーザー設定に委ねる設計になっています。

### Keybindings (キーバインディング)

タスクを開く際のコマンドは `config.toml` の `[keybindings]` セクションで定義可能です。
各キー（`enter`, `n` など）に対して、実行するシェルコマンドを設定できます。

#### プレースホルダー

コマンド内で以下のプレースホルダーを使用できます：

| プレースホルダー | 説明 |
| :--- | :--- |
| `{session_id}` | 対象タスクのTmuxターゲット (例: `project:issue-123`) |
| `{path}` | 対象タスクのワークツリーパス (例: `/home/user/src/project/.worktrees/issue-123`) |

#### 設定例

```toml
[keybindings]
# Enter: 現在のターミナル内でtmuxターゲットを切り替え
enter = "tmux switch-client -t {session_id}"

# n: VSCodeでワークツリーを開く
n = "code {path}"
```

#### デフォルト動作

`[keybindings]` が未設定でも、デフォルトで `tmux switch-client -t {session_id}`（`enter`）が有効です。

### 移行について

既存の `[terminal]` セクション（`strategy`, `command`, `args`）は**非推奨 (Deprecated)** です。
`[keybindings]` セクションを使用してください。より柔軟な設定が可能です。

これにより、以下のような柔軟な運用が可能になります：
*   OS標準のターミナルで開く
*   VSCodeで開く (`code {path}`)
*   キーごとに異なるアプリケーションで開く
*   タスク作成直後に `npm install` を走らせる
*   タスク作成直後に Claude Code を起動してプロンプトを流し込む

### Base Branch (ベースブランチ)

Worktree作成時のベースブランチを設定できます。

#### 設定例

```toml
# Worktree作成時のベースブランチ (省略時は現在のHEADから作成)
base_branch = "develop"
```

#### 動作

- **設定あり**: `git worktree add -b <branch> <path> <base_branch>` で指定されたベースブランチから分岐
- **設定なし**: `git worktree add -b <branch> <path>` で現在のHEADから分岐（デフォルト動作）

#### ユースケース

- `develop` ブランチをベースにしたい場合（例: mechanix）
- プロジェクトごとに異なる開発フローに対応
