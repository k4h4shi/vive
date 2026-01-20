# Vive 仕様書

## 1. 概要 (Concept)

Viveは、複数のGit Worktree、Tmuxセッション、およびAIエージェント（Claude Code）を一元管理するための**「AI開発オーケストレーター」**です。
ユーザー（開発者）はViveを通じて、複数の並行開発タスクを俯瞰し、監視し、必要に応じて介入・指示を行うことができます。

### ターゲットユーザー
- ソロ開発者（ドッグフーディング）
- Claude Code をヘビーユースしており、同時に複数のコンテキスト（feature, fix, refactor）を切り替えながら開発するスタイル。

## 2. 機能要件 (Requirements)

### 2.1 プロジェクト管理
- [ ] **Configurable Root**: `~/src` などのルートディレクトリを指定し、Gitリポジトリを再帰的に検出・一覧表示する。
- [ ] **Favorites**: 頻繁にアクセスするプロジェクトを「お気に入り（★）」登録し、リスト最上部に固定表示する。
- [ ] **Project Dashboard**: プロジェクトを選択した際、**Tmuxネイティブなダッシュボードセッション**を起動し、配下の全タスクをグリッド表示する。

### 2.2 タスク（Worktree）管理
- [ ] **Auto-Discovery**: `git worktree list` を解析し、既存のワークツリーを「タスク」として自動検出する。
- [ ] **Create Task**: Vive上から直接 `git worktree add` を実行し、新しいタスク（ブランチ）を作成する。
- [ ] **Cleanup Task**: 不要になったタスク（Worktree + Branch + Tmux Session）をVive上から安全に削除する。
- [ ] **Safety**: `main`, `master` などのデフォルトブランチの誤削除を防止する。

### 2.3 エージェント監視 (Monitoring)
- [ ] **Status Detection**: Claude Codeのプロセス状態と出力を解析し、リアルタイムで状態を表示する。
    - ⚙ **Working**: 思考中・実行中（CPU使用、スピナー検出）。黄色で表示。
    - ✎ **Wait (Edit)**: ファイル編集の承認待ち。赤色で表示。
    - `>` **Wait (Shell)**: シェルコマンド実行の承認待ち。赤色で表示。
    - ? **Wait (Other)**: その他のユーザー入力待ち。マゼンタ色で表示。
    - ✓ **Success**: 正常完了。緑色で表示。
    - ✖ **Error**: プロセス異常終了。赤色で表示。
    - • **Idle**: セッションはあるがコマンド未実行。グレーで表示。
- [ ] **Hysteresis**: ステータスのちらつき（Working ↔ Idle）を防ぐため、短時間のアイドルは無視する。

### 2.4 オーケストレーション (Orchestration)
- [x] **Launch Strategy**:
    - **Inline**: 現在のターミナルをTmuxセッションに切り替える（デフォルト）。
    - **Spawn**: 設定された外部ターミナル（Ghostty等）で新しいウィンドウを開く。
- [ ] **Preview**: 選択中のタスクのTmuxペインの内容（最新N行）をTUI上でリアルタイム表示する。
- [ ] **Command Input**: Tmuxにアタッチせずとも、Viveから直接コマンド（`y` やプロンプト）を送信できる。
- [ ] **Native Dashboard**: プロジェクト選択時、全タスクのセッションをペインに埋め込んだ（Nested Tmux）特別なセッションを作成・表示する。

## 3. UI仕様 (Interface)

画面は3ペイン構成のTUI（Text User Interface）です。

```text
+---------------------+---------------------------------------------------------------+
| [1] Sidebar         | [2] Preview / Dashboard Area                                  |
| (Projects & Tasks)  |                                                               |
|                     | (Task Selected)                                               |
| ▼ mechanix (★)      |  Claude Code >                                                |
|   ├─ feature/ui...  |  I have analyzed the error. The issue is in main.rs.          |
|   │  ⚙ Working      |  Shall I fix it? [y/n]                                        |
|   └─ fix/bug-123    |                                                               |
|      ✎ Wait: Edit   | (Project Selected)                                            |
|                     |  Project: mechanix                                            |
| ▼ vive              |  Active Tasks: 2                                              |
|   └─ main           |                                                               |
|      • Idle         |  [ Open Dashboard ] -> Launches Tmux Grid View                |
+---------------------+---------------------------------------------------------------+
| [3] Command Input Area                                                              |
| > _                                                                                 |
+-----------------------------------------------------------------------------------+
```

### キー操作

| キー | 動作 | 備考 |
| :--- | :--- | :--- |
| `j` / `k` | カーソル移動 | プロジェクト・タスク間を移動 |
| `Enter` / `o` | **Open/Attach** | 選択中のタスクを開く。プロジェクト選択時はDashboardを開く。 |
| `n` | **New Task** | タスク作成モーダルを開く |
| `D` (Shift+d) | **Delete Task** | タスク削除確認モーダルを開く |
| `f` | **Favorite** | 選択中のプロジェクトをお気に入りトグル |
| `i` | **Input** | コマンド入力モードへ移行 |
| `Esc` | **Cancel** | 入力モード解除 / モーダルを閉じる |
| `q` | **Quit** | Viveを終了する |

#### 入力モード時のキー操作

| キー | 動作 | 備考 |
| :--- | :--- | :--- |
| `←` / `→` | カーソル移動 | 入力文字列内でカーソルを左右に移動 |
| `Backspace` | 文字削除 | カーソル位置の前の文字を削除 |
| `Enter` | 送信 | 入力内容をTmuxに送信 |
| `Esc` | キャンセル | 入力モードを解除 |

入力モードでは日本語入力（IME）にも対応しています。カーソル位置がIME変換ウィンドウの表示位置に反映されます。

## 4. 技術仕様 (Architecture)

詳細な実装方針は `docs/research/` 以下のドキュメントを参照してください。

- **状態管理**: `AppState` が静的なプロジェクト情報と動的なエージェントステータス（`HashMap`）を統合管理する。
- **監視ロジック**: [State Management & Hysteresis](research/state-management.md)
- **パース処理**: [Robust Parsing Strategy](research/robust-parsing.md)
- **UI描画**: [Rich UI Tree](research/rich-ui-tree.md)

### データモデル

```rust
struct Project {
    name: String,
    path: PathBuf,
    worktrees: Vec<Worktree>,
    is_favorite: bool,
}

struct Worktree {
    branch: String,
    path: PathBuf,
    session_id: String, // "project:branch"
}

enum AgentStatus {
    Working { activity: String },
    Waiting { reason: String }, // "Edit", "Shell", etc.
    Idle,
    Error { message: String },
}
```
