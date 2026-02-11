# Vive 仕様書

## 1. 概要 (Concept)

Viveは、複数のGit Worktree、Tmuxウィンドウ（プロジェクトセッション内）、およびAIエージェント（Claude Code）を一元管理するための**「AI開発オーケストレーター」**です。

### ターゲットユーザー
- ソロ開発者（ドッグフーディング）
- Claude Code をヘビーユースしており、同時に複数のコンテキスト（feature, fix, refactor）を切り替えながら開発するスタイル。

## 2. 機能要件 (Requirements)

### 2.1 プロジェクト管理
- [ ] **Configurable Root**: `~/src` などのルートディレクトリを指定し、Gitリポジトリを再帰的に検出・一覧表示する。
- [ ] **Favorites**: 頻繁にアクセスするプロジェクトを「お気に入り（★）」登録し、リスト最上部に固定表示する。

### 2.2 タスク（Worktree）管理
- [ ] **Auto-Discovery**: `git worktree list` を解析し、既存のワークツリーを「タスク」として自動検出する。
- [x] **Create Task**: Vive上から直接 `git worktree add` を実行し、新しいタスク（ブランチ）を作成する。
    - **Manual**: ブランチ名を手動入力する。
    - **Pick from Issue**: GitHub Issueリストからタスクを選択し、`issue-{番号}` 形式でブランチを自動生成する。Tmux window名はデフォルトではブランチ名と同じ `issue-{番号}` になる（旧バージョンの `issue-{番号}_{Issueタイトル}` 形式も後方互換で扱える）。
        - **Batch Creation**: Spaceキーで複数Issueを選択し、Enterキーで一括作成が可能。
        - 選択されたIssueは `[x]` チェックボックスで表示され、選択件数がモーダル上部に表示される。
        - 一部のタスク作成に失敗しても処理は継続し、完了後にサマリー（成功/失敗件数）を表示する。
    - **Auto-Kickstart**: タスク作成後、設定されたコマンドを自動実行する（デフォルト有効、Tabキーでトグル可能）。
        - Manual: `manual_command` で設定されたコマンドを実行（例: `claude`）。
        - Issue Picker: `issue_command` で設定されたワンライナーコマンドを実行（例: `claude "/fix {issue_number}"`）。
        - Batch Creation時は、作成された全タスクに対してAuto-Kickstartを実行する。
        - 利用可能なプレースホルダー: `{issue_number}`, `{session_id}`, `{branch_name}`, `{project_name}`, `{worktree_path}`
- [x] **Cleanup Task**: 不要になったタスク（Worktree + Branch + Tmux Window）をVive上から安全に削除する。
    - Worktreeが存在しない場合でも、SessionやBranchのみが残っている場合は削除可能（Issue #90）。
    - Worktree削除に失敗しても、BranchとSessionの削除を試行する。
- [x] **Safety**: `main`, `master` などのデフォルトブランチの誤削除を防止する.

### 2.3 エージェント監視 (Monitoring)
- [ ] **Status Detection**: Claude Codeのプロセス状態と出力を解析し、リアルタイムで状態を表示する。
    - ⚙ **Working**: 思考中・実行中（CPU使用、スピナー検出）。黄色で表示。
    - ✎ **Wait (Edit)**: ファイル編集の承認待ち。赤色で表示。
    - `>` **Wait (Shell)**: シェルコマンド実行の承認待ち。赤色で表示。
    - ? **Wait (Other)**: その他のユーザー入力待ち。マゼンタ色で表示。
    - ✓ **Success**: 正常完了。緑色で表示。
    - ✖ **Error**: プロセス異常終了。赤色で表示。
    - • **Idle**: ウィンドウはあるがコマンド未実行。グレーで表示。
- [ ] **Hysteresis**: ステータスのちらつき（Working ↔ Idle）を防ぐため、短時間のアイドルは無視する。

### 2.4 オーケストレーション (Actions & Hooks)
ユーザー操作（Command）やイベント（Hook）に対する具体的な振る舞い（Action）を定義する。

- [ ] **User-Defined Actions**: `config.toml` に定義された任意のシェルコマンドを実行可能にする。
    - プレースホルダー（`{session_id}`, `{path}`, `{project_name}`）の置換をサポート。
- [ ] **Command Bindings**: UI上の操作（Enterキーなど）に対して、実行するアクションを割り当てる。
    - 例: Open Command (`Enter`) -> `actions.open` ("tmux switch-client -t {session_id}")
- [ ] **Lifecycle Hooks**: 特定のイベント発生時に自動実行されるアクションを定義する。
    - `post_create_task`: タスク作成完了時に実行（例: `npm install`, `claude` 起動）。

### 2.5 プレビュー (Preview)
- [x] **Live Preview**: 選択中のタスクのTmuxペインの内容（最新N行）をTUI上でリアルタイム表示する。
    - 非アクティブなタスクの進行状況や、エージェントの待機理由を確認可能にする。
- [x] **Preview Scroll**: プレビューにフォーカスして過去のログをスクロール閲覧可能。

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

#### 共通キー操作

| キー | 動作 | 備考 |
| :--- | :--- | :--- |
| `Tab` | **フォーカス切り替え** | サイドバーとプレビュー間でフォーカスを切り替え |
| `h` / `←` | **サイドバーへフォーカス** | 左ペイン（サイドバー）にフォーカスを移動 |
| `l` / `→` | **プレビューへフォーカス** | 右ペイン（プレビュー）にフォーカスを移動 |
| `Enter` | **Open/Attach** | 選択中のタスクを開く（設定されたコマンドを実行）。 |
| `q` | **Quit** | Viveを終了する |

#### サイドバーフォーカス時

| キー | 動作 | 備考 |
| :--- | :--- | :--- |
| `j` / `k` | カーソル移動 | プロジェクト・タスク間を移動 |
| `n` | **New Task** | タスク作成方法選択モーダルを開く（Manual / Pick from Issue） |
| `d` or `D` | **Delete Task** | タスク削除確認モーダルを開く（Session/Branch/Worktreeのいずれかが存在する場合に削除可能） |
| `f` | **Favorite** | 選択中のプロジェクトをお気に入りトグル |
| `i` | **Input** | コマンド入力モードへ移行 |
| `Space` | **Expand/Collapse** | プロジェクトの展開/折りたたみ |

#### プレビューフォーカス時

| キー | 動作 | 備考 |
| :--- | :--- | :--- |
| `j` / `k` | スクロール | プレビュー内容を1行ずつスクロール |
| `Ctrl-d` | ページダウン | 半ページ分下にスクロール |
| `Ctrl-u` | ページアップ | 半ページ分上にスクロール |
| `g` | 先頭へ移動 | プレビューの先頭にスクロール |
| `G` (Shift+g) | 末尾へ移動 | プレビューの末尾にスクロール |

#### モーダル表示時（タスク作成）

| キー | 動作 | 備考 |
| :--- | :--- | :--- |
| `Tab` | **Auto-Kickstartトグル** | Auto-Kickstartの有効/無効を切り替え |
| `Enter` | **決定** | 選択を確定（Issue Pickerでは選択中の全Issueでタスク作成） |
| `Esc` | **キャンセル** | モーダルを閉じる |
| `j` / `k` または `↓` / `↑` | **選択移動** | リスト内のカーソル移動 |
| `Space` | **Issue選択トグル** | Issue Pickerでのみ有効。複数Issueを選択可能 |

#### Issue Picker バッチ作成機能

Issue Pickerモーダルでは、複数のIssueを選択して一括でタスクを作成できます。

- **選択方法**: `Space` キーでIssueの選択/解除をトグル
- **選択状態の表示**: 選択されたIssueは `[x]` チェックボックスで表示
- **選択数の表示**: 1件以上選択時、モーダル上部に選択件数を表示
- **一括作成**: `Enter` キーで選択中の全Issueのタスクを作成
  - 選択なしの場合は、カーソル位置の1件のみ作成
- **エラーハンドリング**: 一部のタスク作成に失敗しても処理は継続し、完了後にサマリーを表示
- **Auto-Kickstart**: 有効な場合、作成された全タスクに対して自動キックスタートを実行

#### マウス操作

| 操作 | 動作 | 備考 |
| :--- | :--- | :--- |
| サイドバーをクリック | サイドバーへフォーカス | サイドバーにフォーカスを移動 |
| プレビューをクリック | プレビューへフォーカス | プレビューにフォーカスを移動 |
| マウスホイール | スクロール | フォーカス中のペインをスクロール |

#### UIフィードバック

フォーカス中のペインは**黄色の太線枠**で強調表示されます。非フォーカスのペインは**グレーの枠線**で表示されます。
これにより、ユーザーは常に「今どちらのペインを操作しているか」を視覚的に確認できます。

### 3.1 起動モード

#### 通常モード（デフォルト）
3ペイン構成（サイドバー + プレビュー + コマンド入力）で表示する。

#### リスト専用モード (`--list-only` / `-l`)
一覧（サイドバー）+ ヘッダー + フッターのみを表示する簡易モード。プレビューペインは非表示。
- CLI引数: `--list-only` または `-l`
- 表示範囲: 一覧 + ヘッダー + フッター。プレビューペインは非表示。
- ステータス更新: プレビュー非表示でも、エージェントステータスは定期的に更新される。
- 操作: 通常操作はそのまま有効（Enter / o 等）。
- 設定: CLIのみ（config.tomlには追加しない）。

## 4. 技術仕様 (Architecture)

詳細な実装方針は `docs/research/` 以下のドキュメントを参照してください。

- **状態管理**: `AppState` が静的なプロジェクト情報と動的なエージェントステータス（`HashMap`）を統合管理する。
- **設定ファイル**: `~/.vive/config.toml` にTOML形式で保存。

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

// Focus pane for key navigation
enum FocusPane {
    Sidebar,  // j/k navigate project list
    Preview,  // j/k scroll preview content
}

// GitHub Issue (for Issue Picker)
struct GitHubIssue {
    number: u32,
    title: String,
}

// Modal types
enum ModalType {
    CreateTaskMethod { selected: CreateTaskMethod },  // Manual or Pick from Issue
    CreateTask { input: String },                     // Manual branch name input
    IssuePicker {
        issues: Vec<GitHubIssue>,
        selected_indices: HashSet<usize>,  // Multi-select support
        ...
    },                                                // Issue selection (supports batch creation)
    ConfirmDeletion { branch_name: String },          // Delete confirmation
}

// ユーザー設定可能なアクション定義
struct ActionsConfig {
    open: String,    // "tmux attach -t {session_id}" etc.
    hooks: HashMap<String, String>, // "post_create" -> "..."
}

### 移行計画 (Migration)
- 既存の `[terminal]` セクション（`strategy`, `command`, `args`）は **非推奨 (Deprecated)** とする。
- `[actions]` セクションが定義されている場合はそちらを優先する。
- 将来的に `[terminal]` セクションのサポートを削除する。
```
