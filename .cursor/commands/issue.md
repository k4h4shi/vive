# イシュー管理

## 概要

GitHub イシューの作成・確認を行います。

## 使用例

- `/issue` - 新規イシュー作成
- `/issue #123` - イシュー #123 の詳細確認

## 手順

### 新規作成

1. **下書き作成**

   簡潔なタイトル形式を使用してください。

   ```bash
   cd /Users/takahashikotaro/src/github/k4h4shi/vive
   cat > /tmp/issue.md << 'EOF'
   ## 概要
   <!-- 何を実現したいか -->

   ## 背景
   <!-- なぜ必要か -->

   ## 完了条件
   - [ ] 条件1
   EOF
   ```

2. **ユーザーと内容確認・修正**

3. **イシュー作成**

   タイトルは簡潔に記述してください（例: `自動補完機能の追加`）。

   ```bash
   gh issue create --title "タイトル" --body-file /tmp/issue.md
   rm /tmp/issue.md
   ```

### 詳細確認

```bash
gh issue view <番号>
```
